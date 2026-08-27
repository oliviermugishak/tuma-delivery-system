use crate::app::{AppError, AppResult, UserContext};
use accounts::UserRole;
use axum::{Extension, Json};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

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
