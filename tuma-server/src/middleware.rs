use crate::app::{AppError, AppState, UserContext};
use accounts::jwt;
use axum::Extension;
use axum::extract::{Request, State};
use axum::http::Method;
use axum::http::header::{AUTHORIZATION, ORIGIN};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;
use secrecy::ExposeSecret;

pub const AUTH_COOKIE: &str = "tuma-auth-token";
pub const REFRESH_COOKIE: &str = "tuma-auth-refresh";

/// Builds the per-request [`UserContext`] from either an `Authorization:
/// Bearer` header (mobile) or the session cookie (web).
///
/// Every valid token triggers a fresh lookup of the user row: a deactivated
/// or deleted user loses access on their very next request, even while
/// holding a long-lived mobile token. One primary-key lookup per
/// authenticated request is a fair price for that.
pub async fn auth_context(
    State(app): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    let jar = CookieJar::from_headers(request.headers());
    let token: Option<String> = bearer_token(&request).map(str::to_string).or_else(|| {
        jar.get(AUTH_COOKIE)
            .map(|cookie| cookie.value().to_string())
    });

    let mut context = UserContext::default();
    if let Some(token) = token {
        let secret = app.jwt_signing_key.expose_secret().as_bytes();
        match jwt::verify(&token, secret) {
            Ok(claims) => {
                let mut conn = app.db_pool.acquire().await?;
                match accounts::users::by_id(&mut conn, claims.sub).await {
                    Ok(Some(user)) if user.is_active => context.user = Some(user),
                    Ok(Some(_)) => {
                        tracing::debug!(user_id = %claims.sub, "token valid but user is inactive")
                    }
                    Ok(None) => {
                        tracing::debug!(user_id = %claims.sub, "token for a deleted user")
                    }
                    Err(error) => return Err(AppError::Database(error)),
                }
            }
            Err(_) => {
                // Invalid or expired token: the request continues anonymous;
                // route guards turn that into a 401 where needed.
            }
        }
    }

    request.extensions_mut().insert(context);
    Ok(next.run(request).await)
}

fn bearer_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Gate for routes any authenticated role may use (e.g. `/v1/me`).
pub async fn required_auth(
    Extension(context): Extension<UserContext>,
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    if context.user.is_none() {
        return Err(AppError::Authentication("Access denied".into()));
    }
    Ok(next.run(request).await)
}

/// Role guard for audience namespaces: wire with
/// `middleware::from_fn_with_state(&[UserRole::Admin][..], require_role)`.
pub async fn require_role(
    State(roles): State<&'static [accounts::UserRole]>,
    Extension(context): Extension<UserContext>,
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    match context.user {
        Some(user) if roles.contains(&user.role) => Ok(next.run(request).await),
        Some(_) => Err(AppError::Forbidden("Insufficient role".into())),
        None => Err(AppError::Authentication("Access denied".into())),
    }
}

/// CSRF defense for cookie sessions: a state-changing request coming from a
/// browser must carry an `Origin` header on the allow list
/// (`TUMA_CORS_ORIGIN`). Requests without an Origin header (mobile Bearer
/// clients, curl, server-to-server) pass — no browser is involved, so no
/// cookie is at risk.
pub async fn csrf_origin_check(
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    let mutates = matches!(
        request.method(),
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    );
    if mutates
        && let Some(origin) = request
            .headers()
            .get(ORIGIN)
            .and_then(|value| value.to_str().ok())
    {
        let allowed = crate::app::allowed_origins();
        if !allowed.iter().any(|candidate| candidate == origin) {
            tracing::warn!(origin, "rejected request from disallowed origin");
            return Err(AppError::Forbidden("Origin not allowed".into()));
        }
    }
    Ok(next.run(request).await)
}
