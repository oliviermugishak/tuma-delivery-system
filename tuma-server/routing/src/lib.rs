//! Road routing for the delivery tracking system — the `RoutingProvider`
//! adapter the tracking doc §4 locked. Google Directions is called by OUR
//! server (the key never reaches a client) and cached on the delivery at
//! handoff; the backend behind this trait is config, exactly like object
//! storage. Until a Directions key is configured the `none` backend
//! answers honestly: there is no route — callers fall back to their own
//! estimates and no geometry is ever invented.

pub mod polyline;

use app_config::RoutingConfig;
use async_trait::async_trait;
use secrecy::ExposeSecret;

pub use polyline::{decode_polyline, encode_polyline};

#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("the configured routing backend is not usable: {0}")]
    Configuration(String),
    #[error("the routing request failed: {0}")]
    Request(String),
}

/// A WGS84 point.
#[derive(Debug, Clone, Copy)]
pub struct Coord {
    pub lat: f64,
    pub lng: f64,
}

/// A road route between two points, as Google Directions reports it.
#[derive(Debug, Clone)]
pub struct Route {
    /// The encoded Google polyline — decoded client-side, stored verbatim.
    pub polyline: String,
    pub duration_secs: i64,
}

#[async_trait]
pub trait RoutingProvider: std::fmt::Debug + Send + Sync {
    /// The road route `from → to`, or `None` when no route is available
    /// (no routing configured, or Google found no road path). Never
    /// invents geometry.
    async fn route(&self, from: Coord, to: Coord) -> Result<Option<Route>, RoutingError>;
}

/// The backend used until a Directions key is configured: no route, ever.
#[derive(Debug, Default)]
pub struct NoRouting;

#[async_trait]
impl RoutingProvider for NoRouting {
    async fn route(&self, _from: Coord, _to: Coord) -> Result<Option<Route>, RoutingError> {
        Ok(None)
    }
}

/// The Google Directions backend (tracking doc §4): our server calls
/// Directions, the key stays here, and the encoded polyline travels
/// verbatim to clients. The client is built ONCE (with a timeout — the
/// per-call client and its unbounded hangs are the behavior this
/// replaces) and shared across every call.
#[derive(Debug)]
pub struct GoogleRouting {
    client: reqwest::Client,
    api_key: String,
    region: Option<String>,
}

impl GoogleRouting {
    pub fn new(api_key: &str, region: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            api_key: api_key.to_string(),
            region,
        }
    }
}

/// What one Directions response says, reduced to the fields the delivery
/// cache needs. Pure so it is testable without network.
fn parse_directions_response(body: &serde_json::Value) -> Result<Option<Route>, RoutingError> {
    let status = body["status"].as_str().unwrap_or_default();
    match status {
        "OK" => {}
        // The honest no-route answer: the caller's fallback owns what a
        // missing road means (an ETA estimate, no geometry).
        "ZERO_RESULTS" | "NOT_FOUND" => return Ok(None),
        other => {
            let detail = body["error_message"].as_str().unwrap_or(other);
            return Err(RoutingError::Request(format!(
                "Directions API answered {status}: {detail}"
            )));
        }
    }

    let route = body["routes"]
        .as_array()
        .and_then(|routes| routes.first())
        .ok_or_else(|| RoutingError::Request("Directions OK with no routes".into()))?;
    let polyline = route["overview_polyline"]["points"]
        .as_str()
        .ok_or_else(|| RoutingError::Request("Directions route has no polyline".into()))?;
    let leg = route["legs"]
        .as_array()
        .and_then(|legs| legs.first())
        .ok_or_else(|| RoutingError::Request("Directions route has no legs".into()))?;
    let duration_secs = leg["duration"]["value"]
        .as_i64()
        .ok_or_else(|| RoutingError::Request("Directions leg has no duration".into()))?;

    Ok(Some(Route {
        polyline: polyline.to_string(),
        duration_secs,
    }))
}

#[async_trait]
impl RoutingProvider for GoogleRouting {
    async fn route(&self, from: Coord, to: Coord) -> Result<Option<Route>, RoutingError> {
        let mut url = reqwest::Url::parse("https://maps.googleapis.com/maps/api/directions/json")
            .expect("static directions URL");
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("origin", &format!("{},{}", from.lat, from.lng));
            query.append_pair("destination", &format!("{},{}", to.lat, to.lng));
            query.append_pair("mode", "driving");
            if let Some(region) = &self.region {
                query.append_pair("region", region);
            }
            query.append_pair("key", &self.api_key);
        }

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| RoutingError::Request(error.to_string()))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| RoutingError::Request(error.to_string()))?;
        if !status.is_success() {
            return Err(RoutingError::Request(format!("Directions HTTP {status}")));
        }
        parse_directions_response(&body)
    }
}

/// Build the backend the config names. The Google backend needs its key —
/// a misconfiguration is a startup error, not a silent fallback.
pub fn build_service(
    config: &RoutingConfig,
) -> Result<std::sync::Arc<dyn RoutingProvider>, RoutingError> {
    match config.backend {
        app_config::RoutingBackend::None => Ok(std::sync::Arc::new(NoRouting)),
        app_config::RoutingBackend::Google => {
            let key = config
                .api_key
                .as_ref()
                .map(|key| key.expose_secret())
                .unwrap_or("");
            if key.is_empty() {
                return Err(RoutingError::Configuration(
                    "routing.backend = google requires routing.api_key".into(),
                ));
            }
            Ok(std::sync::Arc::new(GoogleRouting::new(
                key,
                config.region.clone(),
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_no_backend_returns_no_route() {
        let routing = NoRouting;
        let from = Coord {
            lat: -1.9620,
            lng: 30.1290,
        };
        let to = Coord {
            lat: -1.9499,
            lng: 30.0622,
        };
        assert!(routing.route(from, to).await.unwrap().is_none());
    }

    #[test]
    fn build_service_names_the_backend() {
        // `none` builds with or without a key.
        assert!(
            build_service(&RoutingConfig {
                backend: app_config::RoutingBackend::None,
                api_key: None,
                region: None,
            })
            .is_ok()
        );
        // `google` without a key is a startup error — misconfig surfaces,
        // it never silently degrades.
        assert!(
            build_service(&RoutingConfig {
                backend: app_config::RoutingBackend::Google,
                api_key: None,
                region: None,
            })
            .is_err()
        );
        assert!(
            build_service(&RoutingConfig {
                backend: app_config::RoutingBackend::Google,
                api_key: Some(secrecy::SecretString::new("AIza-test".into())),
                region: None,
            })
            .is_ok()
        );
        // A configured region rides along (multinational deployments omit it).
        assert!(
            build_service(&RoutingConfig {
                backend: app_config::RoutingBackend::Google,
                api_key: Some(secrecy::SecretString::new("AIza-test".into())),
                region: Some("rw".into()),
            })
            .is_ok()
        );
    }

    #[test]
    fn parses_a_directions_response_into_the_cached_route() {
        let body = serde_json::json!({
            "status": "OK",
            "routes": [
                {
                    "overview_polyline": { "points": "_p~iF~ps|U_ulLnnqC_mqNvxq`~" },
                    "legs": [
                        {
                            "distance": { "text": "6.4 km", "value": 6421 },
                            "duration": { "text": "15 mins", "value": 917 }
                        }
                    ]
                }
            ]
        });
        let route = parse_directions_response(&body).unwrap().unwrap();
        assert_eq!(route.polyline, "_p~iF~ps|U_ulLnnqC_mqNvxq`~");
        assert_eq!(route.duration_secs, 917);
    }

    #[test]
    fn zero_results_is_an_honest_no_route_not_an_error() {
        let body = serde_json::json!({ "status": "ZERO_RESULTS" });
        assert!(parse_directions_response(&body).unwrap().is_none());
    }

    #[test]
    fn google_failures_carry_their_message() {
        let body = serde_json::json!({
            "status": "REQUEST_DENIED",
            "error_message": "The provided API key is invalid."
        });
        let error = parse_directions_response(&body).unwrap_err();
        assert!(error.to_string().contains("REQUEST_DENIED"));
        assert!(error.to_string().contains("invalid"));
    }
}
