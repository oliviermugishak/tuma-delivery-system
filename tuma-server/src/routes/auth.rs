use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::middleware::{AUTH_COOKIE, REFRESH_COOKIE, auth_cookie, expire_cookie, refresh_cookie};
use crate::routes::me::MeResponse;
use accounts::jwt;
use accounts::otp::{self, RequestError};
use accounts::{UserRole, refresh_tokens, users};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use axum_extra::extract::CookieJar;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Loose E.164-ish check: 9–15 digits with an optional leading `+`.
/// Strict per-country normalization is a later slice.
pub(crate) fn validate_phone(phone: &str) -> Result<(), ValidationError> {
    let digits = phone.strip_prefix('+').unwrap_or(phone);
    let valid = (9..=15).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit());
    valid.then_some(()).ok_or_else(|| {
        ValidationError::new("invalid_phone")
            .with_message("expected 9-15 digits with an optional leading +".into())
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
        (status = 200, description = "Code valid — account fetched or created, token issued", body = OtpVerifyResponse),
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
    let user = otp::verify(
        &app.accounts,
        &mut conn,
        &input.phone,
        &input.code,
        input.name.as_deref(),
    )
    .await?;

    let claims = jwt::Claims::new(user.id, user.role, jwt::MOBILE_TOKEN_TTL_SECS);
    let token = jwt::generate(&claims, app.jwt_signing_key.expose_secret().as_bytes())
        .map_err(|e| AppError::Internal(format!("token generation failed: {e}")))?;

    Ok(Json(OtpVerifyResponse {
        token,
        user: MeResponse::from(user),
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

/// Merchant/admin sign-in for the platform. Sets the two session cookies
/// (15-min access JWT + 30-day opaque refresh); the body is empty.
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
    let user = users::by_email(&mut conn, &email).await?;

    // One generic 401 for every failure. The manager equalizes timing for
    // accounts that cannot match (unknown email, customer without a
    // password) by burning a verify against a dummy hash.
    let verified = app
        .accounts
        .verify_staff_password(user.as_ref(), &input.password)
        .await;

    let Some(user) =
        user.filter(|user| verified && user.is_active && user.role != UserRole::Customer)
    else {
        return Err(AppError::Authentication(INVALID_CREDENTIALS.into()));
    };

    let refresh = refresh_tokens::issue(&mut conn, user.id).await?;
    let claims = jwt::Claims::new(user.id, user.role, jwt::WEB_ACCESS_TTL_SECS);
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

/// Change the caller's own password. Merchant/admin only in practice —
/// customers have no password (OTP accounts), which is a 400.
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
