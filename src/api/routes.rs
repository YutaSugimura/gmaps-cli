use crate::http::{MapsApiError, MapsClient};
use crate::location::LatLng;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

const URL: &str = "https://routes.googleapis.com/directions/v2:computeRoutes";

const FIELD_MASK: &str = "routes.duration,\
routes.distanceMeters,\
routes.legs.duration,\
routes.legs.distanceMeters,\
routes.legs.steps,\
routes.warnings";

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum TravelMode {
    #[value(alias = "drive")]
    Driving,
    #[value(alias = "walk")]
    Walking,
    #[value(alias = "bicycle")]
    Bicycling,
    Transit,
    #[value(alias = "two-wheeler", alias = "two_wheeler")]
    TwoWheeler,
}

impl TravelMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Driving => "DRIVE",
            Self::Walking => "WALK",
            Self::Bicycling => "BICYCLE",
            Self::Transit => "TRANSIT",
            Self::TwoWheeler => "TWO_WHEELER",
        }
    }

    fn supports_traffic_aware(self) -> bool {
        matches!(self, Self::Driving | Self::TwoWheeler)
    }
}

impl std::fmt::Display for TravelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Driving => "driving",
            Self::Walking => "walking",
            Self::Bicycling => "bicycling",
            Self::Transit => "transit",
            Self::TwoWheeler => "two_wheeler",
        })
    }
}

// ─────────────────────── Request types ───────────────────────

#[derive(Debug, Serialize)]
struct Waypoint {
    location: WaypointLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WaypointLocation {
    lat_lng: PlaceLatLng,
}

#[derive(Debug, Serialize)]
struct PlaceLatLng {
    latitude: f64,
    longitude: f64,
}

fn to_waypoint(p: LatLng) -> Waypoint {
    Waypoint {
        location: WaypointLocation {
            lat_lng: PlaceLatLng {
                latitude: p.lat,
                longitude: p.lng,
            },
        },
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputeRoutesRequest<'a> {
    origin: Waypoint,
    destination: Waypoint,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    intermediates: Vec<Waypoint>,
    travel_mode: &'static str,
    language_code: &'a str,
    region_code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    routing_preference: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    departure_time: Option<String>,
}

// ─────────────────────── Response types ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub distance_meters: Option<u64>,
    #[serde(default)]
    pub legs: Option<Vec<Leg>>,
    #[serde(default)]
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leg {
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub distance_meters: Option<u64>,
    #[serde(default)]
    pub steps: Option<Vec<Step>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(default)]
    pub distance_meters: Option<u64>,
    #[serde(default)]
    pub static_duration: Option<String>,
    #[serde(default)]
    pub travel_mode: Option<String>,
    #[serde(default)]
    pub navigation_instruction: Option<NavInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavInstruction {
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RoutesResponse {
    #[serde(default)]
    routes: Option<Vec<Route>>,
}

// ─────────────────────── Public API ───────────────────────

pub struct ComputeRoutesOptions<'a> {
    pub origin: LatLng,
    pub destination: LatLng,
    pub intermediates: Vec<LatLng>,
    pub travel_mode: TravelMode,
    pub departure_time: Option<DateTime<Utc>>,
    pub language_code: &'a str,
    pub region_code: &'a str,
}

pub async fn compute_routes(
    client: &MapsClient,
    opts: ComputeRoutesOptions<'_>,
) -> Result<Vec<Route>, MapsApiError> {
    let routing_preference = opts
        .travel_mode
        .supports_traffic_aware()
        .then_some("TRAFFIC_AWARE");
    let departure_time = if opts.travel_mode.supports_traffic_aware() {
        opts.departure_time.map(|t| t.to_rfc3339())
    } else {
        None
    };

    let body = ComputeRoutesRequest {
        origin: to_waypoint(opts.origin),
        destination: to_waypoint(opts.destination),
        intermediates: opts.intermediates.into_iter().map(to_waypoint).collect(),
        travel_mode: opts.travel_mode.as_str(),
        language_code: opts.language_code,
        region_code: opts.region_code,
        routing_preference,
        departure_time,
    };

    let resp: RoutesResponse = client.post_v2(URL, &body, FIELD_MASK).await?;
    Ok(resp.routes.unwrap_or_default())
}
