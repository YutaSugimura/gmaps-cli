use crate::api::geocoding::{self, GeocodeResult};
use crate::commands::util::{new_table, print_json};
use crate::config::Config;
use crate::http::MapsClient;
use crate::location::{LatLng, parse_latlng};
use anyhow::{Result, bail};
use comfy_table::Cell;
use owo_colors::OwoColorize;

pub async fn run_geocode(config: &Config, address: &str, json: bool, limit: usize) -> Result<()> {
    let client = MapsClient::from_config(config);
    let results = geocoding::geocode(&client, address, &config.language, &config.region).await?;
    print_geocode_results(&results, json, limit, /* with_types: */ false);
    Ok(())
}

pub async fn run_reverse(config: &Config, latlng: &str, json: bool, limit: usize) -> Result<()> {
    let Some(LatLng { lat, lng }) = parse_latlng(latlng) else {
        bail!("Invalid format. Provide 'lat,lng' (e.g., 40.7580,-73.9855).");
    };
    let client = MapsClient::from_config(config);
    let results = geocoding::reverse_geocode(&client, lat, lng, &config.language).await?;
    print_geocode_results(&results, json, limit, /* with_types: */ true);
    Ok(())
}

fn print_geocode_results(results: &[GeocodeResult], json: bool, limit: usize, with_types: bool) {
    let limited: Vec<&GeocodeResult> = results.iter().take(limit).collect();
    if limited.is_empty() {
        println!("{}", "No matching addresses found.".yellow());
        return;
    }
    if json {
        print_json(&limited);
        return;
    }
    let mut table = new_table();
    if with_types {
        table.set_header(vec![
            Cell::new("Address").fg(comfy_table::Color::Cyan),
            Cell::new("Types"),
        ]);
    } else {
        table.set_header(vec![
            Cell::new("Address").fg(comfy_table::Color::Cyan),
            Cell::new("Lat"),
            Cell::new("Lng"),
        ]);
    }
    for r in &limited {
        if with_types {
            let types = r
                .types
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            table.add_row(vec![Cell::new(&r.formatted_address), Cell::new(types)]);
        } else {
            table.add_row(vec![
                Cell::new(&r.formatted_address),
                Cell::new(format!("{:.6}", r.geometry.location.lat)),
                Cell::new(format!("{:.6}", r.geometry.location.lng)),
            ]);
        }
    }
    println!("{table}");
}
