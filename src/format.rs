use crate::location::LatLng;

/// Format a distance in human-readable form (m / km).
pub fn format_distance(meters: f64) -> String {
    if meters < 1000.0 {
        format!("{} m", meters.round() as i64)
    } else if meters < 10_000.0 {
        format!("{:.2} km", meters / 1000.0)
    } else {
        format!("{:.1} km", meters / 1000.0)
    }
}

/// Format a duration in human-readable form (seconds / minutes / hours).
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{}s", seconds.round() as i64);
    }
    let minutes = (seconds / 60.0).round() as i64;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = minutes / 60;
    let remain = minutes % 60;
    if remain == 0 {
        format!("{hours} h")
    } else {
        format!("{hours} h {remain} min")
    }
}

/// Convert the Routes API "1234s" string format to seconds.
pub fn parse_duration_string(s: Option<&str>) -> f64 {
    let Some(s) = s else { return 0.0 };
    let trimmed = s.trim_end_matches('s');
    trimmed.parse::<f64>().unwrap_or(0.0)
}

/// Haversine distance between two points (meters).
pub fn haversine(a: LatLng, b: LatLng) -> f64 {
    const R: f64 = 6_371_000.0;
    let to_rad = |d: f64| d.to_radians();
    let d_lat = to_rad(b.lat - a.lat);
    let d_lng = to_rad(b.lng - a.lng);
    let lat1 = to_rad(a.lat);
    let lat2 = to_rad(b.lat);
    let h = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lng / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_formatting() {
        assert_eq!(format_distance(0.0), "0 m");
        assert_eq!(format_distance(123.4), "123 m");
        assert_eq!(format_distance(999.0), "999 m");
        assert_eq!(format_distance(1000.0), "1.00 km");
        assert_eq!(format_distance(1234.5), "1.23 km");
        assert_eq!(format_distance(10_000.0), "10.0 km");
        assert_eq!(format_distance(12_345.0), "12.3 km");
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(45.0), "45s");
        assert_eq!(format_duration(60.0), "1 min");
        assert_eq!(format_duration(125.0), "2 min");
        assert_eq!(format_duration(3600.0), "1 h");
        assert_eq!(format_duration(3700.0), "1 h 2 min");
    }

    #[test]
    fn duration_parsing() {
        assert_eq!(parse_duration_string(None), 0.0);
        assert_eq!(parse_duration_string(Some("1234s")), 1234.0);
        assert_eq!(parse_duration_string(Some("60s")), 60.0);
        assert_eq!(parse_duration_string(Some("garbage")), 0.0);
    }

    #[test]
    fn haversine_known_distance() {
        // Times Square → Battery Park ≈ 6.6 km
        let times_square = LatLng {
            lat: 40.7580,
            lng: -73.9855,
        };
        let battery_park = LatLng {
            lat: 40.7033,
            lng: -74.0170,
        };
        let d = haversine(times_square, battery_park);
        assert!((6_000.0..7_000.0).contains(&d), "expected ~6km, got {d}");
    }
}
