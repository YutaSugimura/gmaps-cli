use crate::api::places::{
    self, PlaceResult, RankPreference, SearchNearbyOptions, SearchTextOptions,
};
use crate::commands::util::{new_table, print_json};
use crate::config::Config;
use crate::format::{format_distance, haversine};
use crate::http::MapsClient;
use crate::location::{LocationResolveOptions, resolve_center};
use anyhow::Result;
use comfy_table::{Cell, Color};
use owo_colors::OwoColorize;

#[derive(Debug)]
pub struct NearbyArgs {
    pub keyword: Vec<String>,
    pub location: Option<String>,
    pub here: bool,
    pub radius: u32,
    pub place_type: Option<String>,
    pub open_now: bool,
    pub limit: u32,
    pub rank: RankPreference,
    pub json: bool,
}

pub async fn run(config: &Config, args: NearbyArgs) -> Result<()> {
    let client = MapsClient::from_config(config);

    let (center, source) = resolve_center(
        &client,
        config,
        &LocationResolveOptions {
            cli_location: args.location.clone(),
            use_here: args.here,
        },
    )
    .await?;

    if !args.json {
        eprintln!(
            "{}",
            format!("Center: {:.5},{:.5} ({source})", center.lat, center.lng).dimmed()
        );
    }

    let radius = args.radius.clamp(1, 50_000);
    let limit = args.limit.clamp(1, 20);
    let rank = args.rank;

    let keyword = args.keyword.join(" ");
    let keyword = keyword.trim();

    let mut filtered_out = 0usize;
    let results: Vec<PlaceResult> = if !keyword.is_empty() {
        // searchText only treats `radius` as a bias (locationBias), so filter client-side.
        let raw = places::search_text(
            &client,
            SearchTextOptions {
                query: keyword,
                center: Some(center),
                radius: Some(radius),
                open_now: args.open_now,
                max_result_count: 20,
                language_code: &config.language,
                region_code: &config.region,
            },
        )
        .await?;
        let total = raw.len();
        let kept: Vec<PlaceResult> = raw
            .into_iter()
            .filter(|p| match p.lat_lng() {
                Some(ll) => haversine(center, ll) <= radius as f64,
                None => false,
            })
            .collect();
        filtered_out = total - kept.len();
        kept.into_iter().take(limit as usize).collect()
    } else {
        // searchNearby uses locationRestriction, so the API enforces `radius` for us.
        places::search_nearby(
            &client,
            SearchNearbyOptions {
                center,
                radius,
                included_types: args.place_type.map(|t| vec![t]),
                max_result_count: limit,
                language_code: &config.language,
                region_code: &config.region,
                rank_preference: rank,
            },
        )
        .await?
    };

    if results.is_empty() {
        let hint = if filtered_out > 0 {
            format!(
                " ({filtered_out} hit(s) lie outside the {radius}m radius; widen with --radius)"
            )
        } else {
            String::new()
        };
        println!("{}", format!("No matching places found.{hint}").yellow());
        return Ok(());
    }

    if args.json {
        print_json(&results);
        return Ok(());
    }

    print_table(&results, center);
    Ok(())
}

fn print_table(results: &[PlaceResult], center: crate::location::LatLng) {
    let mut table = new_table();
    table.set_header(vec![
        Cell::new("#").fg(Color::Cyan),
        Cell::new("Name").fg(Color::Cyan),
        Cell::new("Rating"),
        Cell::new("Reviews"),
        Cell::new("Price"),
        Cell::new("Open"),
        Cell::new("Distance"),
        Cell::new("Address"),
    ]);
    for (i, p) in results.iter().enumerate() {
        let dist = match p.lat_lng() {
            Some(ll) => format_distance(haversine(center, ll)),
            None => "-".into(),
        };
        let open = match p.regular_opening_hours.as_ref().and_then(|h| h.open_now) {
            Some(true) => "Open".to_string(),
            Some(false) => "Closed".to_string(),
            None => "-".to_string(),
        };
        let price = price_label(p.price_level.as_deref());
        let rating = p
            .rating
            .map(|r| format!("{r:.1}"))
            .unwrap_or_else(|| "-".into());
        let count = p
            .user_rating_count
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());
        table.add_row(vec![
            Cell::new((i + 1).to_string()),
            Cell::new(p.display_text()),
            Cell::new(rating),
            Cell::new(count),
            Cell::new(price),
            Cell::new(open),
            Cell::new(dist),
            Cell::new(p.formatted_address.as_deref().unwrap_or("-")),
        ]);
    }
    println!("{table}");
}

fn price_label(level: Option<&str>) -> &'static str {
    match level {
        Some("PRICE_LEVEL_FREE") => "Free",
        Some("PRICE_LEVEL_INEXPENSIVE") => "$",
        Some("PRICE_LEVEL_MODERATE") => "$$",
        Some("PRICE_LEVEL_EXPENSIVE") => "$$$",
        Some("PRICE_LEVEL_VERY_EXPENSIVE") => "$$$$",
        _ => "-",
    }
}
