use crate::http::{MapsApiError, MapsClient};
use crate::location::LatLng;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

const SEARCH_NEARBY_URL: &str = "https://places.googleapis.com/v1/places:searchNearby";
const SEARCH_TEXT_URL: &str = "https://places.googleapis.com/v1/places:searchText";

/// FieldMask covers only the Essentials + Pro tiers.
/// Including Enterprise-tier fields (reviews, photos, etc.) increases billing.
const DEFAULT_FIELD_MASK: &str = "places.id,\
places.displayName,\
places.formattedAddress,\
places.location,\
places.types,\
places.primaryType,\
places.rating,\
places.userRatingCount,\
places.priceLevel,\
places.regularOpeningHours.openNow,\
places.businessStatus,\
places.googleMapsUri";

// ─────────────────────── Response types ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceResult {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<DisplayName>,
    #[serde(default)]
    pub formatted_address: Option<String>,
    #[serde(default)]
    pub location: Option<PlaceLatLng>,
    #[serde(default)]
    pub types: Option<Vec<String>>,
    #[serde(default)]
    pub primary_type: Option<String>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub user_rating_count: Option<i64>,
    #[serde(default)]
    pub price_level: Option<String>,
    #[serde(default)]
    pub regular_opening_hours: Option<OpeningHours>,
    #[serde(default)]
    pub business_status: Option<String>,
    #[serde(default)]
    pub google_maps_uri: Option<String>,
}

impl PlaceResult {
    pub fn lat_lng(&self) -> Option<LatLng> {
        self.location.map(|l| LatLng {
            lat: l.latitude,
            lng: l.longitude,
        })
    }

    pub fn display_text(&self) -> &str {
        self.display_name
            .as_ref()
            .map(|n| n.text.as_str())
            .unwrap_or("-")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayName {
    pub text: String,
    #[serde(rename = "languageCode", default)]
    pub language_code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlaceLatLng {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningHours {
    #[serde(default)]
    pub open_now: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PlacesResponse {
    #[serde(default)]
    places: Option<Vec<PlaceResult>>,
}

// ─────────────────────── Request types ───────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchNearbyRequest<'a> {
    location_restriction: LocationCircle,
    max_result_count: u32,
    language_code: &'a str,
    region_code: &'a str,
    rank_preference: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    included_types: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchTextRequest<'a> {
    text_query: &'a str,
    max_result_count: u32,
    language_code: &'a str,
    region_code: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    location_bias: Option<LocationCircle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    open_now: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocationCircle {
    circle: Circle,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Circle {
    center: PlaceLatLng,
    radius: u32,
}

// ─────────────────────── Public API ───────────────────────

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum RankPreference {
    Distance,
    Popularity,
}

impl RankPreference {
    fn as_str(self) -> &'static str {
        match self {
            Self::Distance => "DISTANCE",
            Self::Popularity => "POPULARITY",
        }
    }
}

pub struct SearchNearbyOptions<'a> {
    pub center: LatLng,
    pub radius: u32,
    pub included_types: Option<Vec<String>>,
    pub max_result_count: u32,
    pub language_code: &'a str,
    pub region_code: &'a str,
    pub rank_preference: RankPreference,
}

pub async fn search_nearby(
    client: &MapsClient,
    opts: SearchNearbyOptions<'_>,
) -> Result<Vec<PlaceResult>, MapsApiError> {
    let body = SearchNearbyRequest {
        location_restriction: LocationCircle {
            circle: Circle {
                center: PlaceLatLng {
                    latitude: opts.center.lat,
                    longitude: opts.center.lng,
                },
                radius: opts.radius,
            },
        },
        max_result_count: opts.max_result_count.clamp(1, 20),
        language_code: opts.language_code,
        region_code: opts.region_code,
        rank_preference: opts.rank_preference.as_str(),
        included_types: opts.included_types,
    };
    let resp: PlacesResponse = client
        .post_v2(SEARCH_NEARBY_URL, &body, DEFAULT_FIELD_MASK)
        .await?;
    Ok(resp.places.unwrap_or_default())
}

pub struct SearchTextOptions<'a> {
    pub query: &'a str,
    pub center: Option<LatLng>,
    pub radius: Option<u32>,
    pub open_now: bool,
    pub max_result_count: u32,
    pub language_code: &'a str,
    pub region_code: &'a str,
}

pub async fn search_text(
    client: &MapsClient,
    opts: SearchTextOptions<'_>,
) -> Result<Vec<PlaceResult>, MapsApiError> {
    let location_bias = match (opts.center, opts.radius) {
        (Some(c), Some(r)) => Some(LocationCircle {
            circle: Circle {
                center: PlaceLatLng {
                    latitude: c.lat,
                    longitude: c.lng,
                },
                radius: r,
            },
        }),
        _ => None,
    };
    let body = SearchTextRequest {
        text_query: opts.query,
        max_result_count: opts.max_result_count.clamp(1, 20),
        language_code: opts.language_code,
        region_code: opts.region_code,
        location_bias,
        open_now: opts.open_now.then_some(true),
    };
    let resp: PlacesResponse = client
        .post_v2(SEARCH_TEXT_URL, &body, DEFAULT_FIELD_MASK)
        .await?;
    Ok(resp.places.unwrap_or_default())
}
