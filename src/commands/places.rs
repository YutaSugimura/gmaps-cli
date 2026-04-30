use crate::api::geocoding;
use crate::commands::util::new_table;
use crate::config::Config;
use crate::http::MapsClient;
use crate::location::{is_app_bundle, parse_latlng};
use crate::places::{self, Place};
use anyhow::{Context, Result, bail};
use comfy_table::{Cell, Color};
use owo_colors::OwoColorize;

/// `add`: resolve coordinates from lat,lng / address / -H GPS and save them.
pub async fn add(
    config: &Config,
    name: &str,
    location: Option<&str>,
    here: bool,
    note: Option<String>,
) -> Result<()> {
    if name.starts_with('@') {
        bail!("Place name cannot start with '@': {name}");
    }
    if name.is_empty() {
        bail!("Place name is required");
    }

    let (lat, lng, hint) = if here {
        if !is_app_bundle() {
            bail!(
                "--here requires running through the .app bundle.\n  → run './scripts/build.sh' to build gmaps.app and invoke via 'gmaps' (nix shell wrapper)"
            );
        }
        eprintln!(
            "{}",
            "Fetching current location via GPS... (up to 15s)".dimmed()
        );
        let ll = crate::location::get_current_location_via_gps()?;
        (ll.lat, ll.lng, Some("from GPS".to_string()))
    } else {
        let location = location.expect("clap enforces required_unless_present");
        if let Some(ll) = parse_latlng(location) {
            (ll.lat, ll.lng, None)
        } else {
            // Treat the input as an address and resolve via the Geocoding API.
            eprintln!("{}", "Geocoding...".dimmed());
            let client = MapsClient::from_config(config);
            let results = geocoding::geocode(&client, location, &config.language, &config.region)
                .await
                .context("Geocoding failed")?;
            if results.is_empty() {
                bail!("Address not found: {location}");
            }
            let g = &results[0];
            (
                g.geometry.location.lat,
                g.geometry.location.lng,
                Some(g.formatted_address.clone()),
            )
        }
    };

    let mut places = places::load()?;
    let updated = places.upsert(Place {
        name: name.to_string(),
        lat,
        lng,
        note,
    });
    places::save(&places)?;

    let action = if updated { "Updated" } else { "Added" };
    println!(
        "{} {} \"{}\" → {:.6},{:.6}",
        "✓".green().bold(),
        action,
        name.cyan(),
        lat,
        lng
    );
    if let Some(h) = hint {
        println!("  {}", h.dimmed());
    }
    Ok(())
}

pub fn list() -> Result<()> {
    let places = places::load()?;
    if places.places.is_empty() {
        println!(
            "{}",
            "No places saved. Add one with 'gmaps places add <name> <lat,lng|address>' or 'gmaps places add <name> -H'."
                .yellow()
        );
        return Ok(());
    }
    let mut table = new_table();
    table.set_header(vec![
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Lat"),
        Cell::new("Lng"),
        Cell::new("Note"),
    ]);
    for p in &places.places {
        table.add_row(vec![
            Cell::new(&p.name),
            Cell::new(format!("{:.6}", p.lat)),
            Cell::new(format!("{:.6}", p.lng)),
            Cell::new(p.note.as_deref().unwrap_or("")),
        ]);
    }
    println!("{table}");
    println!();
    println!(
        "{}",
        "Reference in other commands: --location @<name> / route @<name> @<other>".dimmed()
    );
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let mut places = places::load()?;
    if !places.remove(name) {
        bail!("Place \"{name}\" is not registered. Run 'gmaps places list' to inspect.");
    }
    places::save(&places)?;
    println!("{} Removed \"{}\"", "✓".green().bold(), name.cyan());
    Ok(())
}
