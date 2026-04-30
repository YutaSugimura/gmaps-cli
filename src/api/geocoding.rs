use crate::http::{MapsApiError, MapsClient};
use crate::location::LatLng;
use serde::{Deserialize, Serialize};

const URL: &str = "https://maps.googleapis.com/maps/api/geocode/json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeocodeResult {
    pub formatted_address: String,
    pub geometry: Geometry,
    pub place_id: String,
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub location: LatLng,
}

#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    status: String,
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default)]
    results: Vec<GeocodeResult>,
}

fn handle_response(resp: GeocodeResponse) -> Result<Vec<GeocodeResult>, MapsApiError> {
    match resp.status.as_str() {
        "OK" | "ZERO_RESULTS" => Ok(resp.results),
        other => Err(MapsApiError::Logical {
            message: resp
                .error_message
                .unwrap_or_else(|| format!("Geocoding API error: {other}")),
            code: Some(other.to_string()),
        }),
    }
}

pub async fn geocode(
    client: &MapsClient,
    address: &str,
    language: &str,
    region: &str,
) -> Result<Vec<GeocodeResult>, MapsApiError> {
    let resp: GeocodeResponse = client
        .get_legacy(
            URL,
            &[
                ("address", address),
                ("language", language),
                ("region", region),
            ],
        )
        .await?;
    handle_response(resp)
}

pub async fn reverse_geocode(
    client: &MapsClient,
    lat: f64,
    lng: f64,
    language: &str,
) -> Result<Vec<GeocodeResult>, MapsApiError> {
    let latlng = format!("{lat},{lng}");
    let resp: GeocodeResponse = client
        .get_legacy(URL, &[("latlng", latlng.as_str()), ("language", language)])
        .await?;
    handle_response(resp)
}

/// Lightweight check used to validate that the API key works.
pub async fn ping_api_key(client: &MapsClient) -> Result<(), MapsApiError> {
    let resp: GeocodeResponse = client
        .get_legacy(URL, &[("address", "New York"), ("language", "en")])
        .await?;
    handle_response(resp)?;
    Ok(())
}
