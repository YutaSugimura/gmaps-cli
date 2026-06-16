use crate::api::places::{
    self, PlaceResult, RankPreference, SearchNearbyOptions, SearchTextOptions,
};
use crate::commands::util::{new_table, print_json};
use crate::config::Config;
use crate::format::{format_distance, haversine};
use crate::http::MapsClient;
use crate::location::{LatLng, LocationResolveOptions, resolve_center};
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
    pub map: bool,
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
    if args.map {
        print_map(&results, center, radius);
    }
    Ok(())
}

fn print_table(results: &[PlaceResult], center: LatLng) {
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

// --- mini-map rendering -------------------------------------------------
//
// Renders a small ASCII map of the search results around the center.
// Uses only narrow (single column) characters so the grid stays aligned
// regardless of locale or East-Asian-Width handling.
//
// Coordinates are projected with the equirectangular approximation: good
// enough for any radius the API accepts (≤ 50 km).

const MAP_W: usize = 41;
const MAP_H: usize = 21;

/// Project `place` into integer grid coordinates `(col, row)` around `center`.
/// Returns the cell where the place falls; values may be outside the grid.
fn project_to_grid(
    center: LatLng,
    place: LatLng,
    radius_m: u32,
    width: usize,
    height: usize,
) -> (i64, i64) {
    let cx = (width / 2) as f64;
    let cy = (height / 2) as f64;
    let m_per_deg_lat = 111_320.0_f64;
    let m_per_deg_lng = 111_320.0_f64 * center.lat.to_radians().cos();
    let m_per_col = radius_m as f64 / cx;
    let m_per_row = radius_m as f64 / cy;
    let dx_m = (place.lng - center.lng) * m_per_deg_lng;
    let dy_m = (place.lat - center.lat) * m_per_deg_lat;
    let col = cx + dx_m / m_per_col;
    let row = cy - dy_m / m_per_row;
    (col.round() as i64, row.round() as i64)
}

/// Try to write `label` at `(row, col)`, falling back to nearby cells if the
/// preferred spot collides with an existing label or compass marker. Returns
/// true on success.
///
/// Compass letters and previously placed digits are treated as occupied;
/// crosshair characters (─ │ ┼) are considered free and may be overdrawn.
/// To avoid ambiguous adjacency ("1" + "2" reading as "12"), the cells
/// immediately left and right of the placed label are also required to be
/// non-digit.
fn place_label(grid: &mut [Vec<char>], row: usize, col: usize, label: &[char]) -> bool {
    const OFFSETS: &[(i64, i64)] = &[
        (0, 0),
        (0, 1),
        (0, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (-1, -1),
        (1, 1),
        (1, -1),
        (0, 2),
        (0, -2),
        (-2, 0),
        (2, 0),
        (-1, 2),
        (-1, -2),
        (1, 2),
        (1, -2),
        (-2, 1),
        (-2, -1),
        (2, 1),
        (2, -1),
    ];

    let h = grid.len() as i64;
    let w = grid[0].len() as i64;
    let lw = label.len() as i64;

    for &(dr, dc) in OFFSETS {
        let r = row as i64 + dr;
        let c0 = col as i64 + dc;
        if r < 0 || r >= h {
            continue;
        }
        if c0 < 0 || c0 + lw > w {
            continue;
        }
        let r_u = r as usize;
        let c0_u = c0 as usize;
        let span_free = (0..label.len()).all(|k| !grid[r_u][c0_u + k].is_ascii_alphanumeric());
        let left_free = c0_u == 0 || !grid[r_u][c0_u - 1].is_ascii_digit();
        let right_idx = c0_u + label.len();
        let right_free = right_idx >= grid[r_u].len() || !grid[r_u][right_idx].is_ascii_digit();
        if span_free && left_free && right_free {
            for (k, &ch) in label.iter().enumerate() {
                grid[r_u][c0_u + k] = ch;
            }
            return true;
        }
    }
    false
}

fn print_map(results: &[PlaceResult], center: LatLng, radius_m: u32) {
    let cx = MAP_W / 2;
    let cy = MAP_H / 2;

    let mut grid: Vec<Vec<char>> = vec![vec![' '; MAP_W]; MAP_H];

    for cell in grid[cy].iter_mut() {
        *cell = '─';
    }
    for row in grid.iter_mut() {
        row[cx] = '│';
    }
    grid[cy][cx] = '┼';

    grid[0][cx] = 'N';
    grid[MAP_H - 1][cx] = 'S';
    grid[cy][0] = 'W';
    grid[cy][MAP_W - 1] = 'E';

    let m_per_col = radius_m as f64 / cx as f64;
    let m_per_row = radius_m as f64 / cy as f64;

    let mut overflow: Vec<usize> = Vec::new();

    for (i, p) in results.iter().enumerate() {
        let Some(ll) = p.lat_lng() else { continue };
        let label = (i + 1).to_string();
        let chars: Vec<char> = label.chars().collect();
        let (col_i, row_i) = project_to_grid(center, ll, radius_m, MAP_W, MAP_H);

        let in_bounds = col_i >= 0
            && col_i + chars.len() as i64 <= MAP_W as i64
            && row_i >= 0
            && row_i < MAP_H as i64;

        if !in_bounds || !place_label(&mut grid, row_i as usize, col_i as usize, &chars) {
            overflow.push(i + 1);
        }
    }

    println!();
    println!(
        "{}",
        format!("Map (radius {radius_m}m, ~{m_per_col:.0}m/col, ~{m_per_row:.0}m/row)").dimmed()
    );

    for row in &grid {
        let mut line = String::with_capacity(MAP_W * 6);
        for &ch in row {
            let s = ch.to_string();
            let styled = match ch {
                'N' | 'S' | 'E' | 'W' => format!("{}", s.yellow().bold()),
                '─' | '│' | '┼' => format!("{}", s.bright_black()),
                d if d.is_ascii_digit() => format!("{}", d.to_string().cyan().bold()),
                _ => s,
            };
            line.push_str(&styled);
        }
        println!("  {line}");
    }

    if !overflow.is_empty() {
        let ids: Vec<String> = overflow.iter().map(|n| format!("#{n}")).collect();
        println!("{}", format!("  (off-map: {})", ids.join(", ")).yellow());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lng: f64) -> LatLng {
        LatLng { lat, lng }
    }

    #[test]
    fn projection_centers_on_origin() {
        let c = ll(35.0, 139.0);
        let (col, row) = project_to_grid(c, c, 500, MAP_W, MAP_H);
        assert_eq!(col, (MAP_W / 2) as i64);
        assert_eq!(row, (MAP_H / 2) as i64);
    }

    #[test]
    fn projection_north_lands_above_center() {
        let c = ll(35.0, 139.0);
        // ~100m north (1m ≈ 1/111320 deg lat).
        let n = ll(35.0 + 100.0 / 111_320.0, 139.0);
        let (col, row) = project_to_grid(c, n, 500, MAP_W, MAP_H);
        assert_eq!(col, (MAP_W / 2) as i64, "north should keep the same column");
        assert!(
            row < (MAP_H / 2) as i64,
            "north should reduce the row (rows grow downward), got row={row}"
        );
    }

    #[test]
    fn projection_east_lands_right_of_center() {
        let c = ll(35.0, 139.0);
        let cos_lat = c.lat.to_radians().cos();
        // ~100m east.
        let e = ll(35.0, 139.0 + 100.0 / (111_320.0 * cos_lat));
        let (col, row) = project_to_grid(c, e, 500, MAP_W, MAP_H);
        assert!(
            col > (MAP_W / 2) as i64,
            "east should increase the column, got col={col}"
        );
        assert_eq!(row, (MAP_H / 2) as i64);
    }

    #[test]
    fn place_label_writes_at_preferred_cell() {
        let mut grid = vec![vec![' '; 10]; 5];
        assert!(place_label(&mut grid, 2, 4, &['1']));
        assert_eq!(grid[2][4], '1');
    }

    #[test]
    fn place_label_shifts_on_collision() {
        let mut grid = vec![vec![' '; 10]; 5];
        grid[2][4] = '1';
        assert!(place_label(&mut grid, 2, 4, &['2']));
        assert_eq!(grid[2][4], '1', "'1' must be preserved");

        let mut found = None;
        for (r, row) in grid.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                if ch == '2' {
                    assert!(found.is_none(), "more than one '2' written");
                    found = Some((r, c));
                }
            }
        }
        let (r, c) = found.expect("'2' was written somewhere");
        if r == 2 {
            assert!(
                c != 3 && c != 5,
                "'2' must keep a gap from '1' on the same row; got col={c}"
            );
        }
    }

    #[test]
    fn place_label_overdraws_crosshair() {
        let mut grid = vec![vec![' '; 10]; 5];
        grid[2][4] = '┼';
        assert!(place_label(&mut grid, 2, 4, &['1']));
        assert_eq!(grid[2][4], '1');
    }

    #[test]
    fn projection_beyond_radius_lands_off_grid() {
        let c = ll(35.0, 139.0);
        let radius = 500u32;
        let cos_lat = c.lat.to_radians().cos();
        // ~2x the radius east must project past the right edge, so the
        // caller treats it as off-map.
        let far_east = ll(35.0, 139.0 + (2.0 * radius as f64) / (111_320.0 * cos_lat));
        let (col, _row) = project_to_grid(c, far_east, radius, MAP_W, MAP_H);
        assert!(
            col >= MAP_W as i64,
            "a point well beyond the radius should fall off the grid, got col={col}"
        );
    }

    #[test]
    fn place_label_fails_when_no_room() {
        // A grid already full of digits leaves no free span, so placement
        // fails and the caller routes the marker to the off-map list.
        let mut grid = vec![vec!['9'; 3]; 3];
        assert!(!place_label(&mut grid, 1, 1, &['1']));
    }
}
