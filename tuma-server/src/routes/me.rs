use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use axum::extract::State;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// The authenticated account with its authorization context. Deliberately
/// not the raw domain rows — responses never carry `password_hash`. The
/// account itself is roleless; what it can do is visible in the profiles
/// and memberships below, all resolved fresh per request.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MeResponse {
    pub id: Uuid,
    pub phone: Option<String>,
    pub email: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub customer: Option<ProfileResponse>,
    pub admin: Option<ProfileResponse>,
    /// The rider profile — present only on rider accounts. A rider has no
    /// customer profile; rider mode is their surface.
    pub rider: Option<RiderProfileResponse>,
    pub merchant_memberships: Vec<MembershipResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProfileResponse {
    pub id: Uuid,
    pub name: Option<String>,
}

/// The rider profile as the account itself sees it: identity plus the
/// number the merchant asks for at handoff. The phone lives at the account
/// level (the OTP anchor); assignability (`is_active`) matters to the
/// rider's own screen too.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RiderProfileResponse {
    pub id: Uuid,
    pub rider_number: i64,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MembershipResponse {
    pub merchant_id: Uuid,
    pub merchant_name: String,
    #[schema(value_type = String)]
    pub role: accounts::MembershipRole,
    /// `null` = the whole business; a value = this member is scoped to that
    /// one store.
    pub store_id: Option<Uuid>,
    #[schema(value_type = String)]
    pub merchant_status: accounts::MerchantStatus,
}

impl MeResponse {
    pub fn build(
        account: &accounts::Account,
        authorization: &accounts::AuthorizationContext,
    ) -> Self {
        Self {
            id: account.id,
            phone: account.phone.clone(),
            email: account.email.clone(),
            created_at: account.created_at,
            customer: authorization
                .customer
                .as_ref()
                .map(|customer| ProfileResponse {
                    id: customer.id,
                    name: customer.name.clone(),
                }),
            admin: authorization.admin.as_ref().map(|admin| ProfileResponse {
                id: admin.id,
                name: admin.name.clone(),
            }),
            rider: authorization
                .rider
                .as_ref()
                .map(|rider| RiderProfileResponse {
                    id: rider.id,
                    rider_number: rider.rider_number,
                    name: rider.name.clone(),
                    is_active: rider.is_active,
                }),
            merchant_memberships: authorization
                .memberships
                .iter()
                .map(|membership| MembershipResponse {
                    merchant_id: membership.merchant_id,
                    merchant_name: membership.merchant_name.clone(),
                    role: membership.role,
                    store_id: membership.store_id,
                    merchant_status: membership.merchant_status,
                })
                .collect(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/me",
    responses(
        (status = 200, description = "The authenticated account with its profiles and memberships", body = MeResponse),
        (status = 401, description = "Missing or invalid credentials"),
    ),
    tag = "auth"
)]
#[tracing::instrument(name = "Current session")]
pub async fn me(Extension(context): Extension<UserContext>) -> AppResult<Json<MeResponse>> {
    let user = context
        .user
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let authorization = context
        .authorization
        .ok_or_else(|| AppError::Internal("authenticated account without authorization".into()))?;
    Ok(Json(MeResponse::build(&user, &authorization)))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateMeInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
}

/// Updates the caller's own profile name — the customer profile when there
/// is one, else the admin profile. Merchant-operator accounts have no
/// editable name in V1 (their identity on the wing is the business).
#[utoipa::path(
    patch,
    path = "/v1/me",
    request_body = UpdateMeInput,
    responses(
        (status = 200, description = "The updated profile", body = MeResponse),
        (status = 400, description = "Empty name, or no editable profile"),
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
    let authorization = context
        .authorization
        .ok_or_else(|| AppError::Internal("authenticated account without authorization".into()))?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }

    let mut conn = app.db_pool.acquire().await?;
    if authorization.customer.is_some() {
        accounts::customers::set_name(&mut conn, user.id, name).await?;
    } else if authorization.admin.is_some() {
        accounts::admins::set_name(&mut conn, user.id, name).await?;
    } else {
        return Err(AppError::BadRequest(
            "this account has no editable profile name".into(),
        ));
    }

    // The account existed a few lines up (we hold its row); a missing row
    // here is a mid-request delete race, same class as a missing context.
    let (_, authorization) = accounts::authorization_for(&mut conn, user.id)
        .await?
        .ok_or_else(|| AppError::Internal("authenticated account without authorization".into()))?;
    Ok(Json(MeResponse::build(&user, &authorization)))
}
