use crate::error::ApiErrorResponse;
use crate::error::validation_errors_to_field_errors;
use crate::middleware::{
    auth_context, csrf_origin_check, require_admin, require_customer, require_merchant,
    required_auth,
};
use crate::routes::admin::{
    create_merchant, delete_customer, delete_merchant, get_merchant, list_customers,
    list_merchants, summary, update_customer, update_merchant,
};
use crate::routes::auth::{change_password, login, logout, otp_request, otp_verify};
use crate::routes::catalog::{
    create_product, create_store_product, delete_product, delete_store_product, list_products,
    list_store_products, update_product, update_store_product,
};
use crate::routes::health_check;
use crate::routes::me::{me, update_me};
use crate::routes::openapi_json;
use crate::routes::orders::{
    advance_store_order, checkout, get_merchant_store_order, get_order, list_merchant_orders,
    list_orders,
};
use crate::routes::search::search;
use crate::routes::storage::{
    delete_product_image, delete_store_banner, list_product_images, set_product_cover,
    upload_product_image, upload_store_banner,
};
use crate::routes::stores::{
    create_own_store, delete_own_store, get_own_store, get_store, list_own_stores, list_stores,
    update_own_store,
};
use axum::extract::{DefaultBodyLimit, FromRequest};
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
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
    pub accounts: Arc<accounts::AccountManager>,
    pub jwt_signing_key: SecretString,
    /// Fixed OTP code for local development (see app-config `AuthConfig`).
    pub dev_otp_code: Option<String>,
    /// `Secure` flag for session cookies: false on local plain-http, true
    /// behind TLS in production.
    pub cookie_secure: bool,
    /// Object storage (slice U1): images. Backend is config — local disk
    /// in dev, in-memory in tests, S3-compatible R2 in production.
    pub storage: Arc<storage::StorageService>,
}

impl AppState {
    pub fn new(
        db_pool: PgPool,
        accounts: Arc<accounts::AccountManager>,
        jwt_signing_key: SecretString,
        dev_otp_code: Option<String>,
        cookie_secure: bool,
        storage: storage::StorageService,
    ) -> Self {
        Self {
            db_pool: Arc::new(db_pool),
            accounts,
            jwt_signing_key,
            dev_otp_code,
            cookie_secure,
            storage: Arc::new(storage),
        }
    }
}

/// What one membership grants on one merchant business. An owner covers
/// every store; a manager is merchant-wide (`store_ids: None`) or scoped
/// to a set of stores.
#[derive(Debug, Clone)]
pub struct MerchantGrant {
    pub merchant_id: Uuid,
    pub owner: bool,
    /// `None` = every store of the merchant; `Some` = only these.
    pub store_ids: Option<Vec<Uuid>>,
}

impl MerchantGrant {
    pub fn can_access_store(&self, store_id: Uuid) -> bool {
        self.owner
            || self
                .store_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&store_id))
    }
}

/// The merchant-side authorization surface of the signed-in account,
/// computed from its memberships. Customers and plain admins have none.
#[derive(Debug, Clone, Default)]
pub struct MerchantAccess {
    pub grants: Vec<MerchantGrant>,
}

impl MerchantAccess {
    /// Build from fresh membership rows. A merchant is owner-granted when
    /// any owner membership exists; otherwise the manager memberships
    /// define the reachable stores (merchant-wide managers widen to all).
    /// Memberships of SUSPENDED businesses grant nothing — suspension is
    /// how the platform pauses a business.
    pub fn from_memberships(memberships: &[accounts::MembershipView]) -> Self {
        use std::collections::HashMap;
        let mut order: Vec<Uuid> = Vec::new();
        let mut by_merchant: HashMap<Uuid, (bool, Option<Vec<Uuid>>, bool)> = HashMap::new();
        for membership in memberships {
            if membership.merchant_status == accounts::MerchantStatus::Suspended {
                continue;
            }
            let entry = by_merchant
                .entry(membership.merchant_id)
                .or_insert_with(|| {
                    order.push(membership.merchant_id);
                    (false, Some(Vec::new()), false)
                });
            match membership.role {
                accounts::MembershipRole::Owner => entry.0 = true,
                accounts::MembershipRole::Manager => match membership.store_id {
                    None => entry.2 = true, // merchant-wide manager
                    Some(store_id) => {
                        if let Some(ids) = entry.1.as_mut() {
                            ids.push(store_id);
                        }
                    }
                },
            }
        }
        let grants = order
            .into_iter()
            .map(|merchant_id| {
                let (owner, scoped, merchant_wide) = &by_merchant[&merchant_id];
                let store_ids = if *owner || *merchant_wide {
                    None
                } else {
                    scoped.clone()
                };
                MerchantGrant {
                    merchant_id,
                    owner: *owner,
                    store_ids,
                }
            })
            .collect();
        Self { grants }
    }

    /// The single business this account works for. Creation endpoints need
    /// one unambiguous merchant; accounts with memberships in several
    /// businesses cannot create (V1 keeps one business per operator).
    pub fn single_grant(&self) -> AppResult<&MerchantGrant> {
        match self.grants.len() {
            1 => Ok(&self.grants[0]),
            0 => Err(AppError::Forbidden(
                "this account has no merchant membership".into(),
            )),
            _ => Err(AppError::BadRequest(
                "this account belongs to several merchant businesses — select one".into(),
            )),
        }
    }

    pub fn can_access_store(&self, merchant_id: Uuid, store_id: Uuid) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.merchant_id == merchant_id && grant.can_access_store(store_id))
    }

    pub fn can_access_any_store_of(&self, merchant_id: Uuid) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.merchant_id == merchant_id)
    }

    /// The domain's narrow view of the grants — the marketplace crate has
    /// no HTTP or auth-middleware types.
    pub fn store_scopes(&self) -> Vec<marketplace::stores::StoreScope> {
        self.grants
            .iter()
            .map(|grant| marketplace::stores::StoreScope {
                merchant_id: grant.merchant_id,
                store_ids: grant.store_ids.clone(),
            })
            .collect()
    }
}

/// Who is making this request. Built once per request by the `auth_context`
/// middleware (Bearer header or session cookie): the account plus its
/// authorization context, all looked up fresh — a deactivated account, a
/// revoked membership, or a suspended business loses access on the very
/// next request.
#[derive(Clone, Debug, Default)]
pub struct UserContext {
    pub user: Option<accounts::Account>,
    pub authorization: Option<accounts::AuthorizationContext>,
}

impl UserContext {
    pub fn user_id(&self) -> Option<Uuid> {
        self.user.as_ref().map(|user| user.id)
    }

    pub fn customer_id(&self) -> Option<Uuid> {
        self.authorization
            .as_ref()
            .and_then(|auth| auth.customer.as_ref().map(|customer| customer.id))
    }

    pub fn is_admin(&self) -> bool {
        self.authorization
            .as_ref()
            .is_some_and(|auth| auth.admin.is_some())
    }

    /// The merchant-side surface; `None` for accounts with no memberships.
    pub fn merchant_access(&self) -> Option<MerchantAccess> {
        let memberships = &self.authorization.as_ref()?.memberships;
        let access = MerchantAccess::from_memberships(memberships);
        (!access.grants.is_empty()).then_some(access)
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
        .route("/me", get(me).patch(update_me))
        .route("/auth/logout", post(logout))
        .route("/auth/password", post(change_password))
        .layer(middleware::from_fn(required_auth));

    // Admin audience namespace: the platform control plane. Authorization
    // is the admins profile row, resolved fresh per request.
    let admin = Router::new()
        .route("/merchants", post(create_merchant).get(list_merchants))
        .route(
            "/merchants/{id}",
            get(get_merchant)
                .patch(update_merchant)
                .delete(delete_merchant),
        )
        .route("/customers", get(list_customers))
        .route(
            "/customers/{id}",
            patch(update_customer).delete(delete_customer),
        )
        .route("/summary", get(summary))
        .layer(middleware::from_fn(require_admin));

    // Merchant audience namespace: stores + catalog + incoming orders.
    // Authorization is the account's merchant memberships, scoped per
    // store server-side.
    let merchant = Router::new()
        .route("/stores", post(create_own_store).get(list_own_stores))
        .route(
            "/stores/{id}",
            get(get_own_store)
                .patch(update_own_store)
                .delete(delete_own_store),
        )
        .route("/products", post(create_product).get(list_products))
        .route(
            "/products/{id}",
            patch(update_product).delete(delete_product),
        )
        .route(
            "/store-products",
            post(create_store_product).get(list_store_products),
        )
        .route(
            "/store-products/{id}",
            patch(update_store_product).delete(delete_store_product),
        )
        .route("/orders", get(list_merchant_orders))
        .route(
            "/store-orders/{id}",
            get(get_merchant_store_order).patch(advance_store_order),
        )
        // Image uploads (slice U1). The default 2 MB body limit would
        // reject real photos before the 5 MB policy could answer 413, so
        // the multipart routes get a raised ceiling; the domain cap is
        // the actual policy.
        .route(
            "/stores/{id}/banner",
            post(upload_store_banner).delete(delete_store_banner),
        )
        .route(
            "/products/{id}/images",
            post(upload_product_image).get(list_product_images),
        )
        .route(
            "/products/{product_id}/images/{image_id}",
            delete(delete_product_image),
        )
        .route(
            "/products/{product_id}/images/{image_id}/cover",
            post(set_product_cover),
        )
        .layer(DefaultBodyLimit::max(6 * 1024 * 1024))
        .layer(middleware::from_fn(require_merchant));

    // Customer audience namespace: browse open stores and their menus.
    let stores = Router::new()
        .route("/", get(list_stores))
        .route("/{id}", get(get_store))
        .layer(middleware::from_fn(require_customer));

    // Discovery: search products + stores; empty q = popular + open feed.
    let discovery = Router::new()
        .route("/", get(search))
        .layer(middleware::from_fn(require_customer));

    // Business routes live under /api/v1, namespaced by audience
    // (/auth, /me, /admin, /merchant, /stores). The OpenAPI contract is
    // served alongside them. See tuma-docs/Tuma_API_Architecture.md.
    let v1 = Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/files/{*key}", get(crate::routes::files::get_file))
        .route("/auth/otp/request", post(otp_request))
        .route("/auth/otp/verify", post(otp_verify))
        .route("/auth/login", post(login))
        .nest("/admin", admin)
        .nest("/merchant", merchant)
        .nest("/stores", stores)
        .nest("/search", discovery)
        .nest(
            "/orders",
            Router::new()
                .route("/", post(checkout).get(list_orders))
                .route("/{id}", get(get_order))
                .route(
                    "/{id}/store-orders/{store_order_id}/cancel",
                    post(crate::routes::orders::cancel_store_order),
                )
                .layer(middleware::from_fn(require_customer)),
        )
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
    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),
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
            AppError::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                msg,
                None,
            ),
            AppError::UnsupportedMediaType(msg) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                msg,
                None,
            ),
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
