pub mod addresses;
pub mod admin;
pub mod auth;
pub mod catalog;
pub mod deliveries;
pub mod files;
pub mod me;
pub mod orders;
pub mod search;
pub mod storage;
pub mod stores;

use crate::api_doc::ApiDoc;
use crate::app::{AppError, AppResult};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server is healthy", body = String),
    ),
    tag = "health"
)]
#[tracing::instrument(name = "Health check")]
pub async fn health_check() -> &'static str {
    "OK"
}

/// Serves the OpenAPI contract. This is the single source of truth for
/// clients: the platform generates its API client from it (see
/// tuma-docs/Tuma_API_Architecture.md), the mobile client is written
/// against it by hand.
#[utoipa::path(
    get,
    path = "/v1/openapi.json",
    responses(
        (status = 200, description = "OpenAPI contract for the Tuma API", body = String),
    ),
    tag = "meta"
)]
#[tracing::instrument(name = "OpenAPI spec")]
pub async fn openapi_json() -> AppResult<String> {
    ApiDoc::openapi()
        .to_pretty_json()
        .map_err(|e| AppError::Internal(format!("OpenAPI serialization failed: {e}")))
}
