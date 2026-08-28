use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use accounts::UserRole;
use axum::extract::State;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// The authenticated user. Deliberately not the raw `accounts::User` —
/// response types never carry `password_hash`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MeResponse {
    pub id: Uuid,
    #[schema(value_type = String, examples("customer"))]
    pub role: UserRole,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<accounts::User> for MeResponse {
    fn from(user: accounts::User) -> Self {
        Self {
            id: user.id,
            role: user.role,
            name: user.name,
            phone: user.phone,
            email: user.email,
            is_active: user.is_active,
            created_at: user.created_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/me",
    responses(
        (status = 200, description = "The authenticated user", body = MeResponse),
        (status = 401, description = "Missing or invalid credentials"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Current session")]
pub async fn me(Extension(context): Extension<UserContext>) -> AppResult<Json<MeResponse>> {
    let user = context
        .user
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    Ok(Json(MeResponse::from(user)))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateMeInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
}

/// Updates the caller's own profile. Name is the only mutable field in V1
/// (see tuma-docs/Tuma_API_Architecture.md §3). Used by the mobile name
/// capture step: OTP verify consumes the code, so a fresh user's name lands
/// here, not in a second verify call.
#[utoipa::path(
    patch,
    path = "/v1/me",
    request_body = UpdateMeInput,
    responses(
        (status = 200, description = "The updated user", body = MeResponse),
        (status = 400, description = "Empty name"),
        (status = 401, description = "Missing or invalid credentials"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Update profile", skip_all)]
pub async fn update_me(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<UpdateMeInput>,
) -> AppResult<Json<MeResponse>> {
    let user = context
        .user
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    let mut conn = app.db_pool.acquire().await?;
    let updated = accounts::users::set_name(&mut conn, user.id, name).await?;
    Ok(Json(MeResponse::from(updated)))
}
