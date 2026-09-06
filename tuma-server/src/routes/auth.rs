use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::middleware::{AUTH_COOKIE, REFRESH_COOKIE, auth_cookie, expire_cookie, refresh_cookie};
use crate::routes::me::MeResponse;
use accounts::jwt;
use accounts::otp::{self, RequestError};
use accounts::{authorization_for, refresh_tokens, users};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use axum_extra::extract::CookieJar;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// E.164 or nothing: every phone in the system carries its country code —
/// `+` followed by 9–15 digits (`+250783002002`). Local formats (`07…`,
/// bare digits) are rejected rather than guessed at, so two spellings of
/// one number can never become two identities. Clients submit the full
/// number: the mobile prepends the dial code; the platform forms require
/// it. Trimming is the only processing anywhere.
pub(crate) fn validate_phone(phone: &str) -> Result<(), ValidationError> {
    let valid = phone.strip_prefix('+').is_some_and(|digits| {
        (9..=15).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
    });
    valid.then_some(()).ok_or_else(|| {
        ValidationError::new("invalid_phone").with_message(
            "include the country code — the full international number, e.g. +250783002002".into(),
        )
    })
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct OtpRequestInput {
    #[validate(custom(function = "validate_phone"))]
    pub phone: String,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct OtpVerifyInput {
    #[validate(custom(function = "validate_phone"))]
    pub phone: String,
    #[validate(length(min = 6, max = 6, message = "code must be 6 digits"))]
    pub code: String,
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OtpRequestResponse {
    pub message: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OtpVerifyResponse {
    pub token: String,
    pub user: MeResponse,
}

const OTP_REQUEST_MESSAGE: &str = "If this phone number is on Tuma, a code is on its way.";

#[utoipa::path(
    post,
    path = "/v1/auth/otp/request",
    request_body = OtpRequestInput,
    responses(
        (status = 200, description = "Always the same response — reveals nothing about the phone number", body = OtpRequestResponse),
        (status = 422, description = "Invalid phone number"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Request OTP", skip_all)]
pub async fn otp_request(
    State(app): State<AppState>,
    ValidatedJson(input): ValidatedJson<OtpRequestInput>,
) -> AppResult<Json<OtpRequestResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    match otp::request(&mut conn, &input.phone, app.dev_otp_code.as_deref()).await {
        // Same response whether a code was issued or the cooldown blocked
        // it — the shape must never reveal which happened.
        Ok(()) | Err(RequestError::TooSoon) => {}
        Err(RequestError::Database(error)) => return Err(AppError::Database(error)),
    }
    Ok(Json(OtpRequestResponse {
        message: OTP_REQUEST_MESSAGE.to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/v1/auth/otp/verify",
    request_body = OtpVerifyInput,
    responses(
        (status = 200, description = "Code valid — account and customer profile fetched or created, token issued", body = OtpVerifyResponse),
        (status = 400, description = "Invalid or expired code"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Verify OTP", skip_all)]
pub async fn otp_verify(
    State(app): State<AppState>,
    ValidatedJson(input): ValidatedJson<OtpVerifyInput>,
) -> AppResult<Json<OtpVerifyResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let sign_in = otp::verify(
        &app.accounts,
        &mut conn,
        &input.phone,
        &input.code,
        input.name.as_deref(),
    )
    .await?;

    let claims = jwt::Claims::new(sign_in.account.id, jwt::MOBILE_TOKEN_TTL_SECS);
    let token = jwt::generate(&claims, app.jwt_signing_key.expose_secret().as_bytes())
        .map_err(|e| AppError::Internal(format!("token generation failed: {e}")))?;

    // The account row was just fetched or created by the sign-in; a missing
    // row here is a mid-request delete race, not a client answer.
    let (_, authorization) = authorization_for(&mut conn, sign_in.account.id)
        .await?
        .ok_or_else(|| AppError::Internal("account vanished during sign-in".into()))?;
    Ok(Json(OtpVerifyResponse {
        token,
        user: MeResponse::build(&sign_in.account, &authorization),
    }))
}

#[utoipa::path(
    post,
    path = "/v1/auth/logout",
    responses(
        (status = 204, description = "Logged out"),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Logout", skip_all)]
pub async fn logout(
    State(app): State<AppState>,
    jar: CookieJar,
    Extension(_context): Extension<UserContext>,
) -> AppResult<Response> {
    // Bearer clients hold a stateless JWT: there is nothing to revoke
    // server-side, the client deletes its token. Cookie sessions revoke
    // their refresh row; the access JWT dies within 15 minutes on its own.
    if let Some(refresh) = jar
        .get(REFRESH_COOKIE)
        .map(|cookie| cookie.value().to_string())
    {
        let mut conn = app.db_pool.acquire().await?;
        refresh_tokens::revoke(&mut conn, &refresh).await?;
    }
    let jar = jar
        .remove(expire_cookie(AUTH_COOKIE))
        .remove(expire_cookie(REFRESH_COOKIE));
    let mut response = jar.into_response();
    *response.status_mut() = StatusCode::NO_CONTENT;
    Ok(response)
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LoginInput {
    #[validate(email(message = "email must be a valid address"))]
    pub email: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

const INVALID_CREDENTIALS: &str = "Invalid email or password";

/// Platform sign-in (merchant operators and admins) with email + password.
/// Sets the two session cookies (15-min access JWT + 30-day opaque refresh);
/// the body is empty. Authorization is resolved from profiles — an account
/// with neither an admin profile nor a merchant membership cannot enter the
/// platform, and the answer never reveals which part failed.
#[utoipa::path(
    post,
    path = "/v1/auth/login",
    request_body = LoginInput,
    responses(
        (status = 204, description = "Session cookies set (tuma-auth-token + tuma-auth-refresh)"),
        (status = 401, description = "One generic error for every failure path"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Login", skip_all)]
pub async fn login(
    State(app): State<AppState>,
    jar: CookieJar,
    ValidatedJson(input): ValidatedJson<LoginInput>,
) -> AppResult<Response> {
    // Emails are stored lowercase; normalize so login is case-insensitive.
    let email = input.email.trim().to_lowercase();
    let mut conn = app.db_pool.acquire().await?;
    let account = users::by_email(&mut conn, &email).await?;

    // One generic 401 for every failure. The manager equalizes timing for
    // accounts that cannot match (unknown email, account without a
    // password) by burning a verify against a dummy hash.
    let verified = app
        .accounts
        .verify_login_password(account.as_ref(), &input.password)
        .await;

    let Some(account) = account.filter(|account| verified && account.is_active) else {
        return Err(AppError::Authentication(INVALID_CREDENTIALS.into()));
    };

    // A password match alone is not platform entry: the account must be an
    // admin or hold a merchant membership. Customers (OTP accounts) have no
    // password and already failed above.
    let (_, authorization) = authorization_for(&mut conn, account.id)
        .await?
        .ok_or_else(|| AppError::Authentication(INVALID_CREDENTIALS.into()))?;
    if authorization.admin.is_none() && authorization.memberships.is_empty() {
        return Err(AppError::Authentication(INVALID_CREDENTIALS.into()));
    }

    let refresh = refresh_tokens::issue(&mut conn, account.id).await?;
    let claims = jwt::Claims::new(account.id, jwt::WEB_ACCESS_TTL_SECS);
    let access = jwt::generate(&claims, app.jwt_signing_key.expose_secret().as_bytes())
        .map_err(|e| AppError::Internal(format!("token generation failed: {e}")))?;

    let jar = jar
        .add(auth_cookie(access, app.cookie_secure))
        .add(refresh_cookie(refresh, app.cookie_secure));
    let mut response = jar.into_response();
    *response.status_mut() = StatusCode::NO_CONTENT;
    Ok(response)
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct ChangePasswordInput {
    #[validate(length(min = 1, message = "current password is required"))]
    pub current_password: String,
    #[validate(length(min = 8, max = 128, message = "new password must be 8-128 characters"))]
    pub new_password: String,
}

/// Change the caller's own password. Merchant operators and admins only in
/// practice — customers have no password (OTP accounts), which is a 400.
#[utoipa::path(
    post,
    path = "/v1/auth/password",
    request_body = ChangePasswordInput,
    responses(
        (status = 204, description = "Password changed"),
        (status = 400, description = "Account has no password (OTP sign-in)"),
        (status = 401, description = "Not authenticated, or current password is wrong"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Change password", skip_all)]
pub async fn change_password(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<ChangePasswordInput>,
) -> AppResult<StatusCode> {
    let user = context
        .user
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let mut conn = app.db_pool.acquire().await?;
    app.accounts
        .change_password(
            &mut conn,
            &user,
            &input.current_password,
            &input.new_password,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
