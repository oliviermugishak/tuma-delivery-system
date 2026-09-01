//! The geocoding proxy (customer-guarded): the map's search field and
//! the reverse-geocoded address line under the pin. The Google key never
//! reaches a client — the server calls the Geocoding API and hands back
//! only the small shape the location editor needs. Rwanda-biased.

use axum::extract::{Query, State};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::app::{AppError, AppResult, AppState, UserContext};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ReverseQuery {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Deserialize, Validate, utoipa::IntoParams)]
pub struct SearchQuery {
    #[validate(length(min = 2, message = "query must be at least 2 characters"))]
    pub q: String,
}

/// One geocode hit: what the location editor renders and what Save
/// stores.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GeoHit {
    pub address_text: String,
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    status: String,
    #[serde(default)]
    results: Vec<GeocodeResult>,
    #[serde(default)]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeocodeResult {
    formatted_address: String,
    geometry: GeocodeGeometry,
}

#[derive(Debug, Deserialize)]
struct GeocodeGeometry {
    location: GeocodeLocation,
}

#[derive(Debug, Deserialize)]
struct GeocodeLocation {
    lat: f64,
    lng: f64,
}

async fn geocode(app: &AppState, params: &[(&str, String)]) -> Result<Vec<GeoHit>, AppError> {
    let key = app
        .google_keys
        .geocoding
        .as_deref()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            // Degrade honestly (P2): no key means no geocode — a 502 the
            // client maps to "move the map and type the line yourself".
            AppError::BadGateway("geocoding is not configured".into())
        })?;
    let mut url = reqwest::Url::parse("https://maps.googleapis.com/maps/api/geocode/json")
        .expect("static geocode URL");
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params {
            query.append_pair(name, value);
        }
        query.append_pair("region", "rw");
        query.append_pair("key", key);
    }

    let response = reqwest::get(url)
        .await
        .map_err(|error| AppError::BadGateway(error.to_string()))?;
    let body: GeocodeResponse = response
        .json()
        .await
        .map_err(|error| AppError::BadGateway(error.to_string()))?;
    if body.status != "OK" && body.status != "ZERO_RESULTS" {
        // Google names the failure (over limit, denied, bad request) —
        // pass it through as a 502 with the message.
        return Err(AppError::BadGateway(
            body.error_message.unwrap_or(body.status),
        ));
    }
    Ok(body
        .results
        .into_iter()
        .map(|result| GeoHit {
            address_text: result.formatted_address,
            lat: result.geometry.location.lat,
            lng: result.geometry.location.lng,
        })
        .collect())
}

fn customer_id(context: &UserContext) -> AppResult<uuid::Uuid> {
    context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

#[utoipa::path(
    get,
    path = "/v1/geo/reverse",
    params(("lat" = f64, Query, description = "Latitude"),
           ("lng" = f64, Query, description = "Longitude")),
    responses(
        (status = 200, description = "Addresses for the point, best first", body = Vec<GeoHit>),
        (status = 401, description = "Not authenticated"),
        (status = 502, description = "Geocoding backend failed or is not configured"),
    ),
    tag = "geo"
)]
#[tracing::instrument(name = "Reverse geocode", skip_all)]
pub async fn reverse_geocode(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(query): Query<ReverseQuery>,
) -> AppResult<Json<Vec<GeoHit>>> {
    let _ = customer_id(&context)?;
    let hits = geocode(&app, &[("latlng", format!("{},{}", query.lat, query.lng))]).await?;
    Ok(Json(hits))
}

#[utoipa::path(
    get,
    path = "/v1/geo/search",
    params(("q" = String, Query, description = "Street or place text")),
    responses(
        (status = 200, description = "Address suggestions for the text", body = Vec<GeoHit>),
        (status = 401, description = "Not authenticated"),
        (status = 502, description = "Geocoding backend failed or is not configured"),
    ),
    tag = "geo"
)]
#[tracing::instrument(name = "Geocode search", skip_all)]
pub async fn search_geocode(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<GeoHit>>> {
    let _ = customer_id(&context)?;
    query.validate().map_err(AppError::Validation)?;
    let hits = geocode(&app, &[("address", query.q)]).await?;
    Ok(Json(hits))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/geo/reverse", axum::routing::get(reverse_geocode))
        .route("/geo/search", axum::routing::get(search_geocode))
}
