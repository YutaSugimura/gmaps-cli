use crate::api::geocoding;
use crate::commands::util::new_table;
use crate::config::Config;
use crate::http::MapsClient;
use crate::location::{LatLng, get_current_location_via_gps, is_app_bundle};
use anyhow::{Result, bail};
use comfy_table::{Cell, Color};
use owo_colors::OwoColorize;

pub async fn run(config: &Config, json: bool, limit: usize) -> Result<()> {
    if !is_app_bundle() {
        bail!(
            "GPS requires running through the .app bundle.\n  → run './scripts/build.sh' to build gmaps.app and invoke via 'gmaps' (nix shell wrapper)"
        );
    }

    eprintln!(
        "{}",
        "Fetching current location via GPS... (up to 15s)".dimmed()
    );
    let ll = get_current_location_via_gps()?;

    let client = MapsClient::from_config(config);
    // Reverse geocoding is a best-effort enrichment of the GPS reading; if it
    // fails we still want to show the coordinates and exit successfully, so
    // we render the error here instead of bubbling it to main and exiting 1.
    let addresses =
        match geocoding::reverse_geocode(&client, ll.lat, ll.lng, &config.language).await {
            Ok(v) => v,
            Err(e) => {
                print_coords(ll);
                eprintln!("{} {e}", "Reverse geocoding failed:".yellow().bold());
                if let Some(c) = e.code() {
                    eprintln!("  {} {c}", "code:".dimmed());
                }
                return Ok(());
            }
        };

    if json {
        let output = serde_json::json!({
            "lat": ll.lat,
            "lng": ll.lng,
            "addresses": addresses.iter().take(limit).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    print_coords(ll);

    if addresses.is_empty() {
        println!("{}", "  No address information available.".dimmed());
        return Ok(());
    }

    println!();
    let mut table = new_table();
    table.set_header(vec![
        Cell::new("Address").fg(Color::Cyan),
        Cell::new("Types"),
    ]);
    for r in addresses.iter().take(limit) {
        let types = r
            .types
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row(vec![Cell::new(&r.formatted_address), Cell::new(types)]);
    }
    println!("{table}");
    println!(
        "{}",
        "  Types 'establishment' / 'point_of_interest' include place / building names.".dimmed()
    );
    Ok(())
}

fn print_coords(ll: LatLng) {
    println!("{} {:.6},{:.6}", "Current location:".bold(), ll.lat, ll.lng);
}
