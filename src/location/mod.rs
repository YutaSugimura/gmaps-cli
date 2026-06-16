use crate::api::geocoding;
use crate::config::Config;
use crate::http::{MapsApiError, MapsClient};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod gps;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

pub fn parse_latlng(s: &str) -> Option<LatLng> {
    let s = s.trim();
    let (lat_s, lng_s) = s.split_once(',')?;
    let lat: f64 = lat_s.trim().parse().ok()?;
    let lng: f64 = lng_s.trim().parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return None;
    }
    Some(LatLng { lat, lng })
}

/// Pass through "lat,lng" inputs; otherwise resolve via the Geocoding API.
pub async fn resolve_address_or_latlng(
    client: &MapsClient,
    input: &str,
    language: &str,
    region: &str,
) -> Result<LatLng, MapsApiError> {
    if let Some(latlng) = parse_latlng(input) {
        return Ok(latlng);
    }
    let results = geocoding::geocode(client, input, language, region).await?;
    if results.is_empty() {
        return Err(MapsApiError::Logical {
            message: format!("Address not found: {input}"),
            code: Some("ZERO_RESULTS".into()),
        });
    }
    let g = &results[0].geometry.location;
    Ok(LatLng {
        lat: g.lat,
        lng: g.lng,
    })
}

/// `@name` references look up places.yaml, otherwise fall back to lat/lng/address resolution.
pub async fn resolve_input(
    client: &MapsClient,
    input: &str,
    language: &str,
    region: &str,
) -> Result<LatLng> {
    if let Some(name) = input.strip_prefix('@') {
        let places = crate::places::load().context("Failed to load places.yaml")?;
        let place = places.find(name).with_context(|| {
            format!(
                "Unknown place: @{name}\n  → run 'gmaps places list' to inspect, or 'gmaps places add {name} ...' to add it"
            )
        })?;
        return Ok(LatLng {
            lat: place.lat,
            lng: place.lng,
        });
    }
    resolve_address_or_latlng(client, input, language, region)
        .await
        .map_err(anyhow::Error::from)
}

pub struct LocationResolveOptions {
    pub cli_location: Option<String>,
    pub use_here: bool,
}

/// Resolve the search center.
/// Priority:
///   1. --location argument
///   2. --here / -H flag, or location_provider=gps → GPS
///   3. default_place (a place registered in places.yaml)
pub async fn resolve_center(
    client: &MapsClient,
    config: &Config,
    opts: &LocationResolveOptions,
) -> Result<(LatLng, String)> {
    use crate::config::LocationProvider;

    if let Some(cli) = &opts.cli_location {
        let center = resolve_input(client, cli, &config.language, &config.region).await?;
        let source = if cli.starts_with('@') {
            format!("--location {cli}")
        } else {
            "--location argument".into()
        };
        return Ok((center, source));
    }

    if matches!(config.location_provider, LocationProvider::Manual) && !opts.use_here {
        bail!(
            "location_provider is set to 'manual'; --location is required.\n  → e.g., gmaps nearby cafe --location @home\n  → or run 'gmaps init' to switch location_provider to 'default'"
        );
    }

    if opts.use_here || matches!(config.location_provider, LocationProvider::Gps) {
        if !is_app_bundle() {
            // CoreLocation authorization does not work outside an .app bundle and will time out.
            if opts.use_here {
                bail!(
                    "--here requires running through the .app bundle.\n  → run './scripts/build.sh' to produce gmaps.app and invoke via 'gmaps' (nix shell wrapper)"
                );
            }
            // provider=gps but running as a bare binary → silently fall back to default_place.
            if std::env::var("DEBUG").as_deref() == Ok("1") {
                eprintln!(
                    "[debug] not running inside an .app bundle; skipping GPS and falling back to default_place"
                );
            }
        } else {
            match get_current_location_via_gps() {
                Ok(center) => return Ok((center, "GPS (CoreLocation)".into())),
                Err(e) => {
                    if opts.use_here {
                        return Err(e);
                    }
                    eprintln!("[!] GPS lookup failed; falling back to default_place: {e:#}");
                }
            }
        }
    }

    if let Some(name) = &config.default_place {
        let places = crate::places::load().context("Failed to load places.yaml")?;
        let place = places.find(name).with_context(|| {
            format!(
                "default_place '{name}' is not registered in places.\n  → run 'gmaps places list' to inspect, or 'gmaps places add {name} <lat,lng>' to add it"
            )
        })?;
        return Ok((
            LatLng {
                lat: place.lat,
                lng: place.lng,
            },
            format!("default_place ({name})"),
        ));
    }

    bail!(
        "Could not determine a center location.\n  → pass --location\n  → or run 'gmaps places add home <lat,lng>' followed by 'gmaps init' to set a default place"
    );
}

#[cfg(target_os = "macos")]
pub fn get_current_location_via_gps() -> Result<LatLng> {
    gps::run().context("GPS lookup failed")
}

#[cfg(not(target_os = "macos"))]
pub fn get_current_location_via_gps() -> Result<LatLng> {
    bail!("GPS lookup is supported only on macOS");
}

/// Detect whether the running binary is inside an .app bundle.
///
/// The executable is almost always invoked through a symlink: Homebrew links
/// it into `$(brew --prefix)/bin`, and the manual install symlinks it into
/// `~/.local/bin`. `current_exe()` returns that symlink path verbatim (it does
/// not resolve symlinks), so we canonicalize first — otherwise a correctly
/// bundled install looks like a bare binary and GPS gets disabled. CoreLocation
/// itself resolves the real path via CFBundle, so GPS works once we agree the
/// process is bundled.
pub fn is_app_bundle() -> bool {
    match std::env::current_exe() {
        Ok(path) => exe_is_in_app_bundle(&path),
        Err(_) => false,
    }
}

fn exe_is_in_app_bundle(exe: &std::path::Path) -> bool {
    // Resolve symlinks so the real `.app/Contents/MacOS/<bin>` layout shows
    // through; fall back to the raw path if canonicalization fails.
    let resolved = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    resolved.to_string_lossy().contains(".app/Contents/MacOS/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
        assert!(matches!(
            parse_latlng("40.7580,-73.9855"),
            Some(LatLng { lat, lng }) if (lat - 40.7580).abs() < 1e-6 && (lng - (-73.9855)).abs() < 1e-6
        ));
        assert!(parse_latlng(" 40.7580 , -73.9855 ").is_some());
        assert!(parse_latlng("-90,-180").is_some());
    }

    #[test]
    fn parse_bad() {
        assert!(parse_latlng("").is_none());
        assert!(parse_latlng("foo,bar").is_none());
        assert!(parse_latlng("40.7580").is_none());
        assert!(parse_latlng("91,0").is_none());
        assert!(parse_latlng("0,181").is_none());
    }

    // Regression: a bundled binary invoked through a symlink (Homebrew's
    // bin shim, or a manual ~/.local/bin link) must still be recognized as
    // running inside the .app. Before canonicalization the symlink path
    // ("…/bin/gmaps") did not contain ".app/Contents/MacOS/", so GPS was
    // wrongly skipped.
    #[test]
    fn app_bundle_detected_through_symlink() {
        use std::fs;
        let base = std::env::temp_dir().join(format!(
            "gmaps-cli-bundle-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let macos_dir = base.join("gmaps.app/Contents/MacOS");
        fs::create_dir_all(&macos_dir).unwrap();
        let real_bin = macos_dir.join("gmaps");
        fs::write(&real_bin, b"#!/bin/sh\n").unwrap();

        let bin_dir = base.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let link = bin_dir.join("gmaps");
        std::os::unix::fs::symlink(&real_bin, &link).unwrap();

        // The raw symlink path does not look bundled, but the resolved one does.
        assert!(!link.to_string_lossy().contains(".app/Contents/MacOS/"));
        assert!(exe_is_in_app_bundle(&link));

        // A plain binary outside any bundle is still reported as bare.
        let bare = bin_dir.join("not-bundled");
        fs::write(&bare, b"x").unwrap();
        assert!(!exe_is_in_app_bundle(&bare));

        fs::remove_dir_all(&base).ok();
    }
}
