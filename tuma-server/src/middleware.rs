use crate::app::{AppError, AppState, UserContext};
use accounts::jwt;
use axum::Extension;
use axum::extract::{Request, State};
use axum::http::Method;
use axum::http::header::{AUTHORIZATION, ORIGIN};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use secrecy::ExposeSecret;

pub const AUTH_COOKIE: &str = "tuma-auth-token";
pub const REFRESH_COOKIE: &str = "tuma-auth-refresh";
/// Both session cookies live under `/api`: the refresh cookie must reach
/// every API request so silent refresh can see it wherever the session
/// lapses, not only on `/auth/*`.
const COOKIE_PATH: &str = "/api";

/// Short-lived access JWT cookie (15 min).
pub(crate) fn auth_cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build((AUTH_COOKIE.to_string(), token))
        .path(COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(time::Duration::seconds(jwt::WEB_ACCESS_TTL_SECS as i64))
        .build()
}

/// Long-lived opaque refresh cookie (30 days). Only its hash is stored
/// server-side (see accounts::refresh_tokens).
pub(crate) fn refresh_cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build((REFRESH_COOKIE.to_string(), token))
        .path(COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(time::Duration::seconds(jwt::WEB_REFRESH_TTL_SECS as i64))
        .build()
}

/// Identity (name + path) of an expired cookie, for `CookieJar::remove`.
pub(crate) fn expire_cookie(name: &'static str) -> Cookie<'static> {
    Cookie::build((name, "")).path(COOKIE_PATH).build()
}

/// Builds the per-request [`UserContext`] from either an `Authorization:
/// Bearer` header (mobile) or the session cookie (web).
///
/// Every valid token triggers a fresh lookup of the account row plus its
/// authorization context (customer profile, admin profile, merchant
/// memberships): a deactivated account, a revoked membership, or a
/// suspended business loses access on their very next request, even while
/// holding a long-lived mobile token. A few indexed lookups per
/// authenticated request is a fair price for that.
///
/// Cookie sessions get silent refresh: an expired or missing access cookie
/// with a live refresh cookie re-mints the access cookie on the response.
/// A dead refresh cookie clears both cookies. Bearer clients never touch
/// cookies.
pub async fn auth_context(
    State(app): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let jar = CookieJar::from_headers(request.headers());
    let bearer = bearer_token(&request).map(str::to_string);
    let token = bearer.clone().or_else(|| {
        jar.get(AUTH_COOKIE)
            .map(|cookie| cookie.value().to_string())
    });
    let secret = app.jwt_signing_key.expose_secret().as_bytes();

    let mut context = UserContext::default();
    let mut remint: Option<String> = None;
    let mut clear_session = false;

    match token.as_deref() {
        Some(token) => match jwt::verify(token, secret) {
            Ok(claims) => {
                let mut conn = app.db_pool.acquire().await?;
                match accounts::authorization_for(&mut conn, claims.sub).await? {
                    Some((user, authorization)) if user.is_active => {
                        context.user = Some(user);
                        context.authorization = Some(authorization);
                    }
                    Some(_) => {
                        tracing::debug!(account_id = %claims.sub, "token valid but account is inactive")
                    }
                    None => {
                        tracing::debug!(account_id = %claims.sub, "token for a deleted account")
                    }
                }
            }
            // An invalid *cookie* token may just be expired — try refresh.
            // An invalid *bearer* token is simply rejected (guards 401).
            Err(_) if bearer.is_none() => match silent_refresh(&app, &jar).await? {
                Refresh::Renewed { user, access } => {
                    let mut conn = app.db_pool.acquire().await?;
                    if let Some((_, authorization)) =
                        accounts::authorization_for(&mut conn, user.id).await?
                    {
                        context.authorization = Some(authorization);
                    }
                    context.user = Some(user);
                    remint = Some(access);
                }
                Refresh::Dead => clear_session = true,
                Refresh::NoCookie => {}
            },
            Err(_) => {}
        },
        // No access token at all — a browser may still hold a live refresh
        // cookie (no cookie present makes this a cheap no-op).
        None => match silent_refresh(&app, &jar).await? {
            Refresh::Renewed { user, access } => {
                let mut conn = app.db_pool.acquire().await?;
                if let Some((_, authorization)) =
                    accounts::authorization_for(&mut conn, user.id).await?
                {
                    context.authorization = Some(authorization);
                }
                context.user = Some(user);
                remint = Some(access);
            }
            Refresh::Dead => clear_session = true,
            Refresh::NoCookie => {}
        },
    }

    request.extensions_mut().insert(context);
    let mut response = next.run(request).await;
    if let Some(access) = remint {
        let jar = jar.add(auth_cookie(access, app.cookie_secure));
        response = apply_jar(jar, response);
    } else if clear_session {
        let jar = jar
            .remove(expire_cookie(AUTH_COOKIE))
            .remove(expire_cookie(REFRESH_COOKIE));
        response = apply_jar(jar, response);
    }
    Ok(response)
}

/// Apply a cookie jar's pending changes (adds/removals only — the jar's
/// delta) to an already-built response. The tuple merge routes through
/// `IntoResponseParts`, which is the only public way to fold a jar into an
/// existing response.
fn apply_jar(jar: CookieJar, response: Response) -> Response {
    (jar, response).into_response()
}

enum Refresh {
    /// Live refresh token and active account — here is a fresh access JWT.
    Renewed {
        user: accounts::Account,
        access: String,
    },
    /// A refresh cookie was present but dead (expired, revoked, or its
    /// account gone/inactive). The caller should clear the session cookies.
    Dead,
    /// No refresh cookie; nothing to do.
    NoCookie,
}

async fn silent_refresh(app: &AppState, jar: &CookieJar) -> Result<Refresh, AppError> {
    let Some(refresh) = jar
        .get(REFRESH_COOKIE)
        .map(|cookie| cookie.value().to_string())
    else {
        return Ok(Refresh::NoCookie);
    };
    let mut conn = app.db_pool.acquire().await?;
    let Some(account_id) = accounts::refresh_tokens::validate(&mut conn, &refresh).await? else {
        return Ok(Refresh::Dead);
    };
    let Some(account) = accounts::users::by_id(&mut conn, account_id).await? else {
        return Ok(Refresh::Dead);
    };
    if !account.is_active {
        return Ok(Refresh::Dead);
    }
    let claims = jwt::Claims::new(account.id, jwt::WEB_ACCESS_TTL_SECS);
    let access = jwt::generate(&claims, app.jwt_signing_key.expose_secret().as_bytes())
        .map_err(|e| AppError::Internal(format!("token generation failed: {e}")))?;
    Ok(Refresh::Renewed {
        user: account,
        access,
    })
}

fn bearer_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Gate for routes any authenticated account may use (e.g. `/v1/me`).
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

/// Authorization is a capability resolved from fresh database state, never
/// a claim in the token. Each audience namespace has one guard:
/// `require_customer` / `require_admin` / `require_merchant`.
macro_rules! capability_guard {
    ($name:ident, $granted:expr, $label:literal) => {
        pub async fn $name(
            Extension(context): Extension<UserContext>,
            request: Request,
            next: Next,
        ) -> Result<impl IntoResponse, AppError> {
            match &context.user {
                Some(_) if $granted(&context) => Ok(next.run(request).await),
                Some(_) => Err(AppError::Forbidden(concat!("not a ", $label).into())),
                None => Err(AppError::Authentication("Access denied".into())),
            }
        }
    };
}

capability_guard!(
    require_customer,
    |context: &UserContext| context.customer_id().is_some(),
    "customer"
);
capability_guard!(
    require_admin,
    |context: &UserContext| context.is_admin(),
    "platform admin"
);
capability_guard!(
    require_merchant,
    |context: &UserContext| context.merchant_access().is_some(),
    "merchant operator"
);
// Rider presence, not assignability: `riders.is_active` decides whether a
// merchant may hand the rider NEW deliveries — a rider mid-run delivers
// their assignment regardless (tracking doc §5).
capability_guard!(
    require_rider,
    |context: &UserContext| context.rider_id().is_some(),
    "rider"
);

/// Per-IP rate limiting on the auth doors — the only endpoints where
/// unlimited attempts become password grinding. Everything else passes
/// through untouched: the middleware classifies the path, and a request
/// with no `ConnectInfo` (no IP fact to be fair to) fails open. The doors
/// count ATTEMPTS, not outcomes — a failed login and a successful one
/// both spend the budget, so a burst can't double-dip the window.
pub async fn rate_limit_middleware(
    State(app): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = &app.rate_limit;
    if config.enabled
        && let Some(ip) = request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|connect_info| connect_info.0.ip())
    {
        let class_limit = if request.uri().path().ends_with("/v1/auth/login") {
            Some(crate::rate_limit::DoorClass::Login)
        } else if request.uri().path().ends_with("/v1/auth/otp/request") {
            Some(crate::rate_limit::DoorClass::Otp)
        } else {
            None
        };
        if let Some(class) = class_limit
            && !app.rate_limiter.check(class, ip)
        {
            return AppError::TooManyRequests("too many attempts — try again in a minute".into())
                .into_response();
        }
    }
    next.run(request).await
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
