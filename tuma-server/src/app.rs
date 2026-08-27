use crate::error::ApiErrorResponse;
use crate::error::validation_errors_to_field_errors;
use crate::routes::health_check;
use axum::extract::FromRequest;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse};
use validator::Validate;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug)]
pub struct AppState {
    pub db_pool: Arc<PgPool>,
}

impl AppState {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            db_pool: Arc::new(db_pool),
        }
    }
}

/// Allowed browser origins from `TUMA_CORS_ORIGIN` (comma-separated),
/// empty when unset. Shared by the CORS layer and (later) CSRF checks.
pub fn allowed_origins() -> Vec<String> {
    std::env::var("TUMA_CORS_ORIGIN")
        .unwrap_or_default()
        .split(',')
        .map(|origin| origin.trim().to_string())
        .filter(|origin| !origin.is_empty())
        .collect()
}

pub fn build_app_with_state(state: AppState) -> Router {
    let public = Router::new().route("/health", get(health_check));

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new())
        .on_response(DefaultOnResponse::new());

    // nest() strips the /api prefix before the inner middleware runs.
    let api = Router::new()
        .merge(public)
        .layer(cors_layer())
        .layer(trace_layer)
        .with_state(state);

    Router::new().nest("/api", api)
}

fn cors_layer() -> tower_http::cors::CorsLayer {
    use axum::http::header::{CONTENT_TYPE, HeaderValue};
    use tower_http::cors::AllowOrigin;

    let origins: Vec<HeaderValue> = allowed_origins()
        .into_iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();

    tower_http::cors::CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_headers([CONTENT_TYPE])
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::PATCH,
        ])
        .allow_credentials(true)
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Authentication failed: {0}")]
    Authentication(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Internal server error")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_code, message, details) = match self {
            AppError::Validation(e) => {
                let field_errors = validation_errors_to_field_errors(e);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_failed",
                    "Input validation failed".to_string(),
                    Some(field_errors),
                )
            }
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg, None),
            AppError::Authentication(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg, None),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg, None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg, None),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg, None),
            AppError::Database(e) => {
                tracing::debug!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "An internal error occurred. Please try again later.".to_string(),
                    None,
                )
            }
            AppError::Internal(msg) => {
                tracing::debug!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred. Please try again later.".to_string(),
                    None,
                )
            }
        };
        let body = ApiErrorResponse {
            error: error_code.to_string(),
            message,
            details,
        };
        (status, Json(body)).into_response()
    }
}

/// JSON extractor that validates the payload with `validator` before the
/// handler ever sees it. Rejects with `AppError::BadRequest` (bad JSON) or
/// `AppError::Validation` (422 with per-field details).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ValidatedJson<T: DeserializeOwned>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: Validate + DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(payload) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| AppError::BadRequest(e.body_text()))?;

        payload.validate().map_err(AppError::Validation)?;

        Ok(ValidatedJson(payload))
    }
}
