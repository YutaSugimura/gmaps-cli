use crate::api::geocoding;
use crate::config::{self, Config, LocationProvider};
use crate::http::{MapsApiError, MapsClient};
use crate::places::{self, Place};
use anyhow::{Result, bail};
use inquire::validator::Validation;
use inquire::{Confirm, CustomUserError, Password, PasswordDisplayMode, Select, Text};
use owo_colors::OwoColorize;

#[derive(Clone)]
struct ProviderOption {
    label: &'static str,
    value: LocationProvider,
}

impl std::fmt::Display for ProviderOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

fn provider_options() -> Vec<ProviderOption> {
    vec![
        ProviderOption {
            label: "Use a fixed place (registered in places)",
            value: LocationProvider::Default,
        },
        ProviderOption {
            label: "Use GPS (CoreLocation)",
            value: LocationProvider::Gps,
        },
        ProviderOption {
            label: "Always require --location",
            value: LocationProvider::Manual,
        },
    ]
}

fn validate_place_name(input: &str) -> Result<Validation, CustomUserError> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(Validation::Invalid("Enter a place name".into()));
    }
    if s.starts_with('@') {
        return Ok(Validation::Invalid(
            "Place name cannot start with '@'".into(),
        ));
    }
    Ok(Validation::Valid)
}

fn validate_latlng(input: &str) -> Result<Validation, CustomUserError> {
    if input.trim().is_empty() {
        return Ok(Validation::Invalid("Enter coordinates as lat,lng".into()));
    }
    let Some((lat_s, lng_s)) = input.trim().split_once(',') else {
        return Ok(Validation::Invalid(
            "Invalid format. Example: 40.7580,-73.9855".into(),
        ));
    };
    let Ok(lat) = lat_s.trim().parse::<f64>() else {
        return Ok(Validation::Invalid("Latitude is not a number".into()));
    };
    let Ok(lng) = lng_s.trim().parse::<f64>() else {
        return Ok(Validation::Invalid("Longitude is not a number".into()));
    };
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return Ok(Validation::Invalid("Coordinates out of range".into()));
    }
    Ok(Validation::Valid)
}

fn validate_non_empty(input: &str) -> Result<Validation, CustomUserError> {
    if input.trim().is_empty() {
        Ok(Validation::Invalid("Required".into()))
    } else {
        Ok(Validation::Valid)
    }
}

pub async fn run_wizard() -> Result<Config> {
    let path = config::config_path()?;
    let existing = config::load()?;
    let is_re_init = existing.is_some();

    if is_re_init {
        println!(
            "{}",
            format!(
                "Existing configuration detected ({}). Press Enter to keep, type to overwrite.",
                path.display()
            )
            .dimmed()
        );
    } else {
        println!("{}", "Setting up the Google Maps CLI.".bold());
        println!(
            "{}",
            "Get an API key at https://console.cloud.google.com/google/maps-apis/credentials"
                .dimmed()
        );
    }
    println!();

    // ── API key: keep existing on bare Enter ──
    let api_key = match existing.as_ref() {
        Some(prev) => {
            println!(
                "{}",
                format!("Current API key: {}", config::mask_api_key(&prev.api_key)).dimmed()
            );
            let input = Password::new("API key (Enter to keep):")
                .with_display_mode(PasswordDisplayMode::Masked)
                .with_display_toggle_enabled()
                .with_help_message("Press Enter to keep the existing key, or type to overwrite")
                .without_confirmation()
                .prompt()?;
            if input.trim().is_empty() {
                prev.api_key.clone()
            } else {
                input.trim().to_string()
            }
        }
        None => Password::new("Google Maps Platform API key:")
            .with_display_mode(PasswordDisplayMode::Masked)
            .with_display_toggle_enabled()
            .with_help_message("Ctrl+R to toggle masking")
            .without_confirmation()
            .with_validator(validate_non_empty)
            .prompt()?
            .trim()
            .to_string(),
    };

    // ── provider: place cursor on the existing value ──
    let starting_cursor = match existing.as_ref().map(|c| c.location_provider) {
        Some(LocationProvider::Default) => 0,
        Some(LocationProvider::Gps) => 1,
        Some(LocationProvider::Manual) => 2,
        None => 0,
    };
    let provider = Select::new("Default location source:", provider_options())
        .with_starting_cursor(starting_cursor)
        .prompt()?
        .value;

    let default_place = match provider {
        LocationProvider::Default => {
            println!();
            println!(
                "{}",
                "Registering the default place (edit later via 'gmaps places ...').".dimmed()
            );

            // If a previous default_place exists, prefill its lat/lng from places.yaml.
            let prev_place = existing
                .as_ref()
                .and_then(|c| c.default_place.as_ref())
                .and_then(|name| {
                    places::load()
                        .ok()
                        .and_then(|p| p.find(name).cloned().map(|pl| (name.clone(), pl)))
                });
            let default_name = prev_place
                .as_ref()
                .map(|(n, _)| n.as_str())
                .unwrap_or("home");
            let default_latlng = prev_place
                .as_ref()
                .map(|(_, p)| format!("{:.6},{:.6}", p.lat, p.lng));

            let name = Text::new("Place name:")
                .with_default(default_name)
                .with_validator(validate_place_name)
                .prompt()?
                .trim()
                .to_string();

            let mut latlng_prompt =
                Text::new("Coordinates (e.g., 40.7580,-73.9855):").with_validator(validate_latlng);
            if let Some(d) = default_latlng.as_deref() {
                latlng_prompt = latlng_prompt.with_default(d);
            }
            let latlng_str = latlng_prompt.prompt()?;
            let (lat_s, lng_s) = latlng_str
                .trim()
                .split_once(',')
                .expect("validate_latlng guarantees a comma");
            let lat: f64 = lat_s
                .trim()
                .parse()
                .expect("validate_latlng guarantees a number");
            let lng: f64 = lng_s
                .trim()
                .parse()
                .expect("validate_latlng guarantees a number");
            let mut all = places::load()?;
            all.upsert(Place {
                name: name.clone(),
                lat,
                lng,
                note: None,
            });
            places::save(&all)?;
            Some(name)
        }
        LocationProvider::Gps => {
            println!();
            if !crate::location::is_app_bundle() {
                println!(
                    "{}",
                    "[!] Not running inside an .app bundle; the GPS test will be skipped.".yellow()
                );
                println!(
                    "{}",
                    "    → Use 'cargo run' for development; use 'gmaps' (the .app) for real GPS."
                        .dimmed()
                );
            } else {
                println!(
                    "{}",
                    "Fetching current location via GPS... (up to 15s)".dimmed()
                );
                match crate::location::get_current_location_via_gps() {
                    Ok(ll) => {
                        println!(
                            "{} Current location: {:.6}, {:.6}",
                            "✓".green(),
                            ll.lat,
                            ll.lng
                        );
                    }
                    Err(e) => {
                        println!("{} GPS lookup failed: {e:#}", "✗".red());
                        println!(
                            "{}",
                            "    → Allow gmaps under System Settings > Privacy & Security > Location Services."
                                .dimmed()
                        );
                    }
                }
            }
            None
        }
        LocationProvider::Manual => None,
    };

    let lang_default = existing
        .as_ref()
        .map(|c| c.language.as_str())
        .unwrap_or("en");
    let region_default = existing.as_ref().map(|c| c.region.as_str()).unwrap_or("US");
    let language = Text::new("Language code:")
        .with_default(lang_default)
        .with_validator(validate_non_empty)
        .prompt()?
        .trim()
        .to_string();
    let region = Text::new("Region code:")
        .with_default(region_default)
        .with_validator(validate_non_empty)
        .prompt()?
        .trim()
        .to_string();

    println!();
    println!("{}", "Verifying API key...".dimmed());
    let client = MapsClient::new(api_key.clone());
    match geocoding::ping_api_key(&client).await {
        Ok(()) => {
            println!("{}", "✓ OK".green());
        }
        Err(e) => {
            println!("{} {e}", "✗ Verification failed:".red());
            if matches!(&e, MapsApiError::Api { code: Some(c), .. } if c == "REQUEST_DENIED") {
                println!(
                    "{}",
                    "  → API key is invalid, or Geocoding API is not enabled.".dimmed()
                );
                println!(
                    "{}",
                    "  → Enable Geocoding API / Places API (New) / Routes API in Cloud Console."
                        .dimmed()
                );
            }
            let proceed =
                Confirm::new("Save anyway? (You can re-run 'gmaps init' to fix it later.)")
                    .with_default(false)
                    .prompt()?;
            if !proceed {
                println!("{}", "Setup aborted.".dimmed());
                bail!("Aborted by user");
            }
        }
    }

    let cfg = Config {
        api_key,
        default_place,
        language,
        region,
        location_provider: provider,
    };
    config::save(&cfg)?;
    println!();
    println!("{} Saved to {} (mode 0600).", "✓".green(), path.display());
    Ok(cfg)
}
