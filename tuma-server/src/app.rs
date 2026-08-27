use crate::error::ApiErrorResponse;
use crate::error::validation_errors_to_field_errors;
use crate::middleware::{auth_context, csrf_origin_check, required_auth};
use crate::routes::auth::{logout, otp_request, otp_verify};
use crate::routes::health_check;
use crate::routes::me::me;
use crate::routes::openapi_json;
use axum::extract::FromRequest;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse};
use uuid::Uuid;
use validator::Validate;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug)]
pub struct AppState {
    pub db_pool: Arc<PgPool>,
    pub jwt_signing_key: SecretString,
    /// Fixed OTP code for local development (see app-config `AuthConfig`).
    pub dev_otp_code: Option<String>,
}

impl AppState {
    pub fn new(
        db_pool: PgPool,
        jwt_signing_key: SecretString,
        dev_otp_code: Option<String>,
    ) -> Self {
        Self {
            db_pool: Arc::new(db_pool),
            jwt_signing_key,
            dev_otp_code,
        }
    }
}

/// Who is making this request. Built once per request by the `auth_context`
/// middleware (Bearer header or session cookie) and read by guards/handlers
/// via `Extension<UserContext>`. `None` user means anonymous.
#[derive(Clone, Debug, Default)]
pub struct UserContext {
    pub user: Option<accounts::User>,
}

impl UserContext {
    pub fn user_id(&self) -> Option<Uuid> {
        self.user.as_ref().map(|user| user.id)
    }

    pub fn role(&self) -> Option<accounts::UserRole> {
        self.user.as_ref().map(|user| user.role)
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
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new())
        .on_response(DefaultOnResponse::new());

    // Routes any authenticated role may use.
    let authenticated = Router::new()
        .route("/me", get(me))
        .route("/auth/logout", post(logout))
        .layer(middleware::from_fn(required_auth));

    // Business routes live under /api/v1, namespaced by audience
    // (/auth, /me, /admin, /merchant, ...). The OpenAPI contract is served
    // alongside them. See tuma-docs/Tuma_API_Architecture.md.
    let v1 = Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/auth/otp/request", post(otp_request))
        .route("/auth/otp/verify", post(otp_verify))
        .merge(authenticated);

    // Layer order (outermost runs first): trace → auth context → CSRF → CORS.
    // /api/health stays unversioned: it is an infra probe, not business API.
    let api = Router::new()
        .route("/health", get(health_check))
        .nest("/v1", v1)
        .layer(cors_layer())
        .layer(middleware::from_fn(csrf_origin_check))
        .layer(middleware::from_fn_with_state(state.clone(), auth_context))
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
