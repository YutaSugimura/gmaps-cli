use crate::config::Config;
use reqwest::Client as ReqwestClient;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use thiserror::Error;

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum MapsApiError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Request timed out")]
    Timeout,

    /// Non-2xx HTTP response. Carries both the transport-level status code
    /// and the message extracted from the v2 error envelope.
    #[error("{message}")]
    Api {
        status: u16,
        message: String,
        code: Option<String>,
    },

    /// HTTP transport succeeded with 200 but the API body itself reported an
    /// error condition (e.g., the legacy Geocoding API's `status: ZERO_RESULTS`
    /// or our own "Address not found" wrapping). These never had an HTTP
    /// status worth keeping, so the variant only carries the API-level code.
    #[error("{message}")]
    Logical {
        message: String,
        code: Option<String>,
    },

    #[error("Failed to parse response: {0}")]
    Parse(String),
}

impl MapsApiError {
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } | Self::Logical { code, .. } => code.as_deref(),
            _ => None,
        }
    }
}

pub struct MapsClient {
    inner: ReqwestClient,
    api_key: String,
}

impl MapsClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let inner = ReqwestClient::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("Failed to build reqwest::Client");
        Self {
            inner,
            api_key: api_key.into(),
        }
    }

    /// Convenience constructor that clones the API key out of an existing
    /// Config. Most commands open a single client per invocation, so this is
    /// just there to remove the repeated `config.api_key.clone()` boilerplate.
    pub fn from_config(config: &Config) -> Self {
        Self::new(config.api_key.clone())
    }

    /// For modern APIs (Places API New / Routes API):
    /// JSON POST with `X-Goog-Api-Key` and `X-Goog-FieldMask` headers.
    pub async fn post_v2<T, B>(
        &self,
        url: &str,
        body: &B,
        field_mask: &str,
    ) -> Result<T, MapsApiError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        if is_debug() {
            eprintln!("[http] POST {url} (FieldMask: {field_mask})");
            if let Ok(body_json) = serde_json::to_string(body) {
                eprintln!("[http] body: {}", redact_pii(&body_json));
            }
        }
        let res = self
            .inner
            .post(url)
            .header("X-Goog-Api-Key", &self.api_key)
            .header("X-Goog-FieldMask", field_mask)
            .json(body)
            .send()
            .await
            .map_err(map_send_error)?;

        let status = res.status();
        let text = res.text().await.map_err(map_send_error)?;

        if is_debug() {
            eprintln!("[http] response: {}", redact_pii(&text));
        }

        if !status.is_success() {
            let (message, code) = parse_v2_error(&text);
            return Err(MapsApiError::Api {
                status: status.as_u16(),
                message: message.unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
                code,
            });
        }

        serde_json::from_str::<T>(&text).map_err(|e| MapsApiError::Parse(e.to_string()))
    }

    /// For the legacy Geocoding API: GET with the `key=` query parameter auto-appended.
    /// Callers are expected to inspect the response's `status` field themselves.
    pub async fn get_legacy<T>(&self, url: &str, params: &[(&str, &str)]) -> Result<T, MapsApiError>
    where
        T: DeserializeOwned,
    {
        if is_debug() {
            let safe_params: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| {
                    if is_pii_param_key(k) {
                        (*k, "<redacted>")
                    } else {
                        (*k, *v)
                    }
                })
                .collect();
            eprintln!("[http] GET {url} {:?} (key=***)", safe_params);
        }
        let mut all = Vec::with_capacity(params.len() + 1);
        all.extend_from_slice(params);
        all.push(("key", &self.api_key));

        let res = self
            .inner
            .get(url)
            .query(&all)
            .send()
            .await
            .map_err(map_send_error)?;

        let status = res.status();
        if !status.is_success() {
            return Err(MapsApiError::Api {
                status: status.as_u16(),
                message: format!("HTTP {}", status.as_u16()),
                code: None,
            });
        }
        let text = res.text().await.map_err(map_send_error)?;
        serde_json::from_str::<T>(&text).map_err(|e| MapsApiError::Parse(e.to_string()))
    }
}

fn is_debug() -> bool {
    std::env::var("DEBUG").as_deref() == Ok("1")
}

fn map_send_error(e: reqwest::Error) -> MapsApiError {
    if e.is_timeout() {
        return MapsApiError::Timeout;
    }
    // Strip the URL from the error: get_legacy puts `?key=<API_KEY>` in the
    // query string, and reqwest's Display includes the URL by default.
    let e = e.without_url();
    MapsApiError::Network(e.to_string())
}

/// True for legacy Geocoding query-param keys whose values are PII (a user's
/// home coordinates or full address) and must be redacted from debug logs.
fn is_pii_param_key(k: &str) -> bool {
    matches!(k, "latlng" | "address")
}

/// Redact PII fields inside a JSON string. Specifically:
///   - any `latitude` / `longitude` numeric value → "<redacted>"
///   - any `formattedAddress` / `formatted_address` text → "<redacted>"
///   - any `textQuery` text (Places searchText input) → "<redacted>"
///
/// Falls back to the original text if the input is not valid JSON.
fn redact_pii(text: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_string();
    };
    redact_value(&mut v);
    serde_json::to_string(&v).unwrap_or_else(|_| text.to_string())
}

fn redact_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if matches!(
                    k.as_str(),
                    "latitude"
                        | "longitude"
                        | "lat"
                        | "lng"
                        | "formattedAddress"
                        | "formatted_address"
                        | "textQuery"
                        | "text_query"
                ) {
                    *val = serde_json::Value::String("<redacted>".to_string());
                } else {
                    redact_value(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact_value(v);
            }
        }
        _ => {}
    }
}

/// Extract `message` and `status` from the v2 API's JSON error envelope.
fn parse_v2_error(text: &str) -> (Option<String>, Option<String>) {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: Option<ErrorDetails>,
    }
    #[derive(serde::Deserialize)]
    struct ErrorDetails {
        status: Option<String>,
        message: Option<String>,
    }
    match serde_json::from_str::<ErrorBody>(text) {
        Ok(b) => match b.error {
            Some(d) => (d.message, d.status),
            None => (None, None),
        },
        Err(_) => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_pii_masks_request_body() {
        let body = r#"{"locationRestriction":{"circle":{"center":{"latitude":40.7580,"longitude":-73.9855},"radius":500}},"textQuery":"hospital near home"}"#;
        let out = redact_pii(body);
        assert!(!out.contains("40.7580"), "lat leaked: {out}");
        assert!(!out.contains("-73.9855"), "lng leaked: {out}");
        assert!(!out.contains("hospital"), "textQuery leaked: {out}");
        assert!(out.contains("<redacted>"));
        // Non-PII fields preserved.
        assert!(out.contains("500"));
    }

    #[test]
    fn redact_pii_masks_response_body() {
        let body = r#"{"places":[{"id":"abc","formattedAddress":"350 5th Ave, Manhattan, NY","location":{"latitude":40.748,"longitude":-73.985},"rating":4.2}]}"#;
        let out = redact_pii(body);
        assert!(!out.contains("Manhattan"), "address leaked: {out}");
        assert!(!out.contains("40.748"), "lat leaked: {out}");
        assert!(!out.contains("-73.985"), "lng leaked: {out}");
        // Non-PII preserved.
        assert!(out.contains("\"rating\":4.2"));
        assert!(out.contains("\"id\":\"abc\""));
    }

    #[test]
    fn redact_pii_geocoding_legacy_response() {
        let body = r#"{"results":[{"formatted_address":"Grand Central Terminal, New York","geometry":{"location":{"lat":40.7527,"lng":-73.9772}}}],"status":"OK"}"#;
        let out = redact_pii(body);
        assert!(
            !out.contains("Grand Central Terminal"),
            "address leaked: {out}"
        );
        assert!(!out.contains("40.7527"), "lat leaked: {out}");
        assert!(!out.contains("-73.9772"), "lng leaked: {out}");
        assert!(out.contains("\"status\":\"OK\""));
    }

    #[test]
    fn redact_pii_passthrough_on_invalid_json() {
        // Garbage in → returned unchanged so debugging is still possible.
        let body = "not json at all";
        assert_eq!(redact_pii(body), body);
    }

    #[test]
    fn is_pii_param_key_matches_known_keys() {
        assert!(is_pii_param_key("latlng"));
        assert!(is_pii_param_key("address"));
        assert!(!is_pii_param_key("language"));
        assert!(!is_pii_param_key("region"));
    }

    /// Regression test: a network failure on the legacy Geocoding API path
    /// must not surface the API key, which is appended as a `?key=…` query
    /// parameter. Trigger a failure by hitting an RFC 6761-reserved hostname
    /// that DNS is required to never resolve.
    #[tokio::test]
    async fn legacy_network_error_does_not_leak_api_key() {
        let secret = "AIzaSyREGRESSION_TEST_KEY_DO_NOT_USE_42";
        let client = MapsClient::new(secret);
        let result: Result<serde_json::Value, _> = client
            .get_legacy("https://nonexistent.invalid/maps/api/geocode/json", &[])
            .await;
        let err = result.expect_err("expected network error against .invalid");
        let rendered = format!("{err}");
        assert!(
            !rendered.contains(secret),
            "API key leaked into error message: {rendered}"
        );
        // The substring "AIza" alone is fine (it's a public Google prefix), but
        // the unique tail from `secret` must never be present.
        assert!(
            !rendered.contains("REGRESSION_TEST_KEY_DO_NOT_USE_42"),
            "key suffix leaked: {rendered}"
        );
    }
}
