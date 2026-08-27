use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::routes::me::MeResponse;
use accounts::jwt;
use accounts::otp::{self, RequestError, VerifyError};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Loose E.164-ish check: 9–15 digits with an optional leading `+`.
/// Strict per-country normalization is a later slice.
fn validate_phone(phone: &str) -> Result<(), ValidationError> {
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
    let user = otp::verify(&mut conn, &input.phone, &input.code, input.name.as_deref())
        .await
        .map_err(|error| match error {
            VerifyError::InvalidCode => AppError::BadRequest("Invalid or expired code".into()),
            VerifyError::Database(error) => AppError::Database(error),
        })?;

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
pub async fn logout(Extension(_context): Extension<UserContext>) -> AppResult<StatusCode> {
    // Bearer clients hold a stateless JWT: there is nothing to revoke
    // server-side, the client deletes its token. Web cookie/refresh
    // revocation is added to this handler with the web session slice.
    Ok(StatusCode::NO_CONTENT)
}
