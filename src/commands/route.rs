use crate::api::routes::{self, ComputeRoutesOptions, TravelMode};
use crate::commands::util::print_json;
use crate::config::Config;
use crate::format::{format_distance, format_duration, parse_duration_string};
use crate::http::MapsClient;
use crate::location::{LatLng, resolve_input};
use anyhow::Result;
use chrono::DateTime;
use owo_colors::OwoColorize;

#[derive(Debug)]
pub struct RouteArgs {
    pub origin: String,
    pub destination: String,
    pub mode: TravelMode,
    pub depart: Option<String>,
    pub waypoints: Option<String>,
    pub steps: bool,
    pub json: bool,
}

pub async fn run(config: &Config, args: RouteArgs) -> Result<()> {
    let mode = args.mode;
    let client = MapsClient::from_config(config);

    // Resolve origin and destination (@name / lat,lng / address)
    let (origin, destination) = tokio::try_join!(
        resolve_input(&client, &args.origin, &config.language, &config.region),
        resolve_input(&client, &args.destination, &config.language, &config.region),
    )?;

    // Waypoints (separated by '|')
    let mut intermediates: Vec<LatLng> = Vec::new();
    if let Some(wp_str) = &args.waypoints {
        for wp in wp_str.split('|') {
            let wp = wp.trim();
            if wp.is_empty() {
                continue;
            }
            let ll = resolve_input(&client, wp, &config.language, &config.region).await?;
            intermediates.push(ll);
        }
    }

    let departure_time = match &args.depart {
        Some(s) => {
            let parsed = s.parse::<DateTime<chrono::FixedOffset>>().map_err(|_| {
                anyhow::anyhow!(
                    "--depart must be in RFC 3339 format (e.g., 2026-04-30T18:00:00-04:00)"
                )
            })?;
            Some(parsed.with_timezone(&chrono::Utc))
        }
        None => None,
    };

    let route_list = routes::compute_routes(
        &client,
        ComputeRoutesOptions {
            origin,
            destination,
            intermediates,
            travel_mode: mode,
            departure_time,
            language_code: &config.language,
            region_code: &config.region,
        },
    )
    .await?;

    if route_list.is_empty() {
        println!("{}", "No routes found.".yellow());
        if matches!(mode, TravelMode::Transit) {
            println!(
                "{}",
                "  → Routes API transit coverage varies by region and is not supported in many countries.\n  → Try driving / walking / bicycling instead."
                    .dimmed()
            );
        }
        return Ok(());
    }

    if args.json {
        print_json(&route_list);
        return Ok(());
    }

    let route = &route_list[0];
    let total_sec = parse_duration_string(route.duration.as_deref());
    let total_m = route.distance_meters.unwrap_or(0) as f64;

    println!("{} {}", "Travel mode:".bold(), args.mode);
    println!("  {} {}", "Duration:".cyan(), format_duration(total_sec));
    println!("  {} {}", "Distance:".cyan(), format_distance(total_m));

    if let Some(warnings) = &route.warnings
        && !warnings.is_empty()
    {
        println!("{}", "Warnings:".yellow());
        for w in warnings {
            println!("  - {w}");
        }
    }

    if args.steps {
        println!();
        println!("{}", "Steps:".bold());
        let mut idx = 1usize;
        if let Some(legs) = &route.legs {
            for leg in legs {
                if let Some(steps) = &leg.steps {
                    for s in steps {
                        let sec = parse_duration_string(s.static_duration.as_deref());
                        let m = s.distance_meters.unwrap_or(0) as f64;
                        let text = s
                            .navigation_instruction
                            .as_ref()
                            .and_then(|n| n.instructions.as_deref())
                            .unwrap_or("(no instruction)");
                        println!(
                            "  {:>3}. {text} {}",
                            idx,
                            format!("({} / {})", format_distance(m), format_duration(sec)).dimmed()
                        );
                        idx += 1;
                    }
                }
            }
        }
    }

    Ok(())
}
