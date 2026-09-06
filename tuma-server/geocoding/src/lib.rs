//! Reverse geocoding as SAVE-TIME enrichment (geocoding-rebuild slice).
//!
//! The model: the pin is the truth; the display text is derived from it.
//! When a customer saves an address — or a merchant saves a store — with
//! a pin, the server asks a geocoding provider for the place name and
//! stores it as `address_text`. Nobody types it; no client sends it; a
//! coordinate pair can never become display text.
//!
//! The provider trait mirrors `routing`'s: `Ok(None)` is the honest
//! no-result answer, and every failure degrades — the caller passes a
//! fallback (the label word) and the save always lands.

use app_config::GeocodingConfig;
use secrecy::ExposeSecret;

/// A WGS84 point.
#[derive(Debug, Clone, Copy)]
pub struct Coord {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum GeocodingError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Request error: {0}")]
    Request(String),
}

#[async_trait::async_trait]
pub trait GeocodingProvider: std::fmt::Debug + Send + Sync {
    /// The place name for `pin`, or `None` when the provider honestly
    /// knows of none. Never invents a place; failures are Err and the
    /// caller's fallback owns the degrade.
    async fn reverse(&self, pin: Coord) -> Result<Option<String>, GeocodingError>;
}

/// The backend until a key is configured: no name, ever.
#[derive(Debug, Default)]
pub struct NoGeocoding;

#[async_trait::async_trait]
impl GeocodingProvider for NoGeocoding {
    async fn reverse(&self, _pin: Coord) -> Result<Option<String>, GeocodingError> {
        Ok(None)
    }
}

/// Deterministic test backend: answers "Test place N" from an internal
/// counter, so integration tests can prove the cache — a repeated pin
/// must reach the provider exactly once and stay "Test place 1".
#[derive(Debug, Default)]
pub struct MemoryGeocoding {
    counter: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl GeocodingProvider for MemoryGeocoding {
    async fn reverse(&self, _pin: Coord) -> Result<Option<String>, GeocodingError> {
        let n = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        Ok(Some(format!("Test place {n}")))
    }
}

/// The Google reverse-geocode backend. The client is built ONCE (with a
/// timeout — the old proxy's per-call client and unbounded hangs are the
/// implementation this crate replaces) and shared across every call.
#[derive(Debug)]
pub struct GoogleGeocoding {
    client: reqwest::Client,
    api_key: String,
    region: Option<String>,
}

impl GoogleGeocoding {
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

#[async_trait::async_trait]
impl GeocodingProvider for GoogleGeocoding {
    async fn reverse(&self, pin: Coord) -> Result<Option<String>, GeocodingError> {
        let mut url = reqwest::Url::parse("https://maps.googleapis.com/maps/api/geocode/json")
            .expect("static geocode URL");
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("latlng", &format!("{},{}", pin.lat, pin.lng));
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
            .map_err(|error| GeocodingError::Request(error.to_string()))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| GeocodingError::Request(error.to_string()))?;
        if !status.is_success() {
            return Err(GeocodingError::Request(format!("Geocoding HTTP {status}")));
        }
        parse_geocode_response(&body)
    }
}

/// What one Geocoding response says, reduced to the one display string
/// the save path needs. Pure so it is testable without network.
fn parse_geocode_response(body: &serde_json::Value) -> Result<Option<String>, GeocodingError> {
    let status = body["status"].as_str().unwrap_or_default();
    match status {
        "OK" => {}
        // The honest no-place answer: the caller's fallback owns what an
        // unnamed pin means.
        "ZERO_RESULTS" | "NOT_FOUND" => return Ok(None),
        other => {
            let detail = body["error_message"].as_str().unwrap_or(other);
            return Err(GeocodingError::Request(format!(
                "Geocoding API answered {status}: {detail}"
            )));
        }
    }
    let formatted = body["results"]
        .as_array()
        .and_then(|results| results.first())
        .and_then(|result| result["formatted_address"].as_str())
        .ok_or_else(|| GeocodingError::Request("Geocoding OK with no results".into()))?;
    Ok(Some(formatted.to_string()))
}

/// Build the backend the config names. The Google backend needs its key —
/// a misconfiguration surfaces at startup, it never silently degrades.
pub fn build_service(
    config: &GeocodingConfig,
) -> Result<std::sync::Arc<dyn GeocodingProvider>, GeocodingError> {
    match config.backend {
        app_config::GeocodingBackend::None => Ok(std::sync::Arc::new(NoGeocoding)),
        app_config::GeocodingBackend::Memory => Ok(std::sync::Arc::new(MemoryGeocoding::default())),
        app_config::GeocodingBackend::Google => {
            let key = config
                .api_key
                .as_ref()
                .map(|key| key.expose_secret().trim())
                .filter(|key| !key.is_empty())
                .ok_or_else(|| {
                    GeocodingError::Configuration(
                        "backend is google but no API key is configured".into(),
                    )
                })?;
            Ok(std::sync::Arc::new(GoogleGeocoding::new(
                key,
                config.region.clone(),
            )))
        }
    }
}

/// The cache key: the pin rounded to 4 decimals (~11 m grid) as
/// "lat|lng". GPS jitter of a few metres must hit the same row.
pub fn pin_key(pin: Coord) -> String {
    format!(
        "{:.4}|{:.4}",
        (pin.lat * 1e4).round() / 1e4,
        (pin.lng * 1e4).round() / 1e4
    )
}

/// Does this string look like a coordinate pair ("lat, lng")? The
/// checkout snapshot refuses such text as a name — a coordinate pair is
/// a pin, never a place.
pub fn is_coordinate_pair(text: &str) -> bool {
    let mut parts = text.trim().split(',');
    let (Some(lat), Some(lng), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |v: &str| v.trim().parse::<f64>().is_ok();
    ok(lat) && ok(lng)
}

/// The save-time derivation: cache-aside around the provider, always
/// answering something — a cache hit, the provider's place name, or the
/// caller's fallback. Infallible: a geocoding outage can never block a
/// save or put coordinates where a name belongs.
pub async fn place_name(
    conn: &mut sqlx::PgConnection,
    provider: &dyn GeocodingProvider,
    pin: Coord,
    fallback: &str,
) -> Result<String, sqlx::Error> {
    let key = pin_key(pin);

    let cached = sqlx::query_scalar!(
        r#"
        SELECT display FROM geocoding.cache WHERE pin_key = $1
        "#,
        key
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(display) = cached {
        return Ok(display);
    }

    match provider.reverse(pin).await {
        Ok(Some(display)) => {
            sqlx::query!(
                r#"
                INSERT INTO geocoding.cache (pin_key, display)
                VALUES ($1, $2)
                ON CONFLICT (pin_key) DO NOTHING
                "#,
                key,
                display
            )
            .execute(&mut *conn)
            .await?;
            Ok(display)
        }
        Ok(None) => {
            tracing::warn!(pin_key = %key, "geocoder knows no place for the pin — using the fallback");
            Ok(fallback.to_string())
        }
        Err(error) => {
            tracing::warn!(pin_key = %key, %error, "geocoding provider failed — using the fallback");
            Ok(fallback.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_geocode_response_into_the_display_name() {
        let body = serde_json::json!({
            "status": "OK",
            "results": [
                { "formatted_address": "KN 5 Rd, Kigali, Rwanda" },
                { "formatted_address": "second place is ignored" }
            ]
        });
        assert_eq!(
            parse_geocode_response(&body).unwrap(),
            Some("KN 5 Rd, Kigali, Rwanda".to_string())
        );
    }

    #[test]
    fn zero_results_is_an_honest_no_place_not_an_error() {
        let body = serde_json::json!({ "status": "ZERO_RESULTS" });
        assert!(parse_geocode_response(&body).unwrap().is_none());
    }

    #[test]
    fn google_failures_carry_their_message() {
        let body = serde_json::json!({
            "status": "REQUEST_DENIED",
            "error_message": "The provided API key is invalid."
        });
        let error = parse_geocode_response(&body).unwrap_err();
        assert!(error.to_string().contains("REQUEST_DENIED"));
        assert!(error.to_string().contains("invalid"));
    }

    #[test]
    fn the_pin_key_rounds_to_an_eleven_metre_grid() {
        // A few metres of GPS jitter must land on the same key.
        let a = pin_key(Coord {
            lat: -1.944123,
            lng: 30.061977,
        });
        let b = pin_key(Coord {
            lat: -1.944130,
            lng: 30.061971,
        });
        assert_eq!(a, b);
        // A different place is a different key.
        let far = pin_key(Coord {
            lat: -1.95,
            lng: 30.06,
        });
        assert_ne!(a, far);
    }

    #[test]
    fn coordinate_shaped_strings_are_recognized() {
        assert!(is_coordinate_pair("-1.9449, 30.0619"));
        assert!(is_coordinate_pair("30.06,-1.94"));
        assert!(!is_coordinate_pair("KN 5 Rd, Kigali"));
        assert!(!is_coordinate_pair("near Simba"));
        assert!(!is_coordinate_pair("-1.9449, 30.0619, Rwanda"));
        assert!(!is_coordinate_pair(""));
    }

    #[test]
    fn build_service_names_the_backend() {
        use app_config::GeocodingBackend;
        // `none` builds with or without a key.
        assert!(
            build_service(&GeocodingConfig {
                backend: GeocodingBackend::None,
                api_key: None,
                region: None,
            })
            .is_ok()
        );
        // `google` without a key is a startup error — misconfig surfaces,
        // it never silently degrades.
        assert!(
            build_service(&GeocodingConfig {
                backend: GeocodingBackend::Google,
                api_key: None,
                region: None,
            })
            .is_err()
        );
        assert!(
            build_service(&GeocodingConfig {
                backend: GeocodingBackend::Google,
                api_key: Some(secrecy::SecretString::new("AIza-test".into())),
                region: Some("rw".into()),
            })
            .is_ok()
        );
        // `memory` builds (the test backend).
        assert!(
            build_service(&GeocodingConfig {
                backend: GeocodingBackend::Memory,
                api_key: None,
                region: None,
            })
            .is_ok()
        );
    }
}
