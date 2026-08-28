//! Admin-only endpoints: provisioning and controlling merchant and customer
//! accounts, plus read-only visibility (merchant detail, platform summary).
//! Merchants never self-signup in V1 — an admin creates them here.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::domain;
use crate::routes::auth::validate_phone;
use crate::routes::me::MeResponse;
use accounts::UserRole;
use accounts::users;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateMerchantInput {
    #[validate(length(max = 100, message = "name must be at most 100 characters"))]
    pub name: Option<String>,
    #[validate(email(message = "email must be a valid address"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/v1/admin/merchants",
    request_body = CreateMerchantInput,
    responses(
        (status = 201, description = "Merchant created", body = MeResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 409, description = "Email already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Create merchant", skip_all)]
pub async fn create_merchant(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateMerchantInput>,
) -> AppResult<(StatusCode, Json<MeResponse>)> {
    let email = input.email.trim().to_lowercase();
    let name = input
        .name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());

    let mut conn = app.db_pool.acquire().await?;
    let user = app
        .accounts
        .create_staff(
            &mut conn,
            UserRole::Merchant,
            name.as_deref(),
            &email,
            &input.password,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(MeResponse::from(user))))
}

#[utoipa::path(
    get,
    path = "/v1/admin/merchants",
    responses(
        (status = 200, description = "All merchant accounts, oldest first", body = Vec<MeResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "List merchants")]
pub async fn list_merchants(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
) -> AppResult<Json<Vec<MeResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let merchants = users::list_merchants(&mut conn).await?;
    Ok(Json(merchants.into_iter().map(MeResponse::from).collect()))
}

/// Admin edits to a merchant account. Every field is optional: provided
/// fields overwrite, an empty name clears it, absent fields keep their
/// current value. `is_active` rides the same endpoint — toggling is an
/// edit like any other.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateMerchantInput {
    #[validate(length(max = 100, message = "name must be at most 100 characters"))]
    pub name: Option<String>,
    #[validate(email(message = "email must be a valid address"))]
    pub email: Option<String>,
    pub is_active: Option<bool>,
}

/// Edit a merchant (name, email, active flag). A taken email is a 409.
#[utoipa::path(
    patch,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant user id")),
    request_body = UpdateMerchantInput,
    responses(
        (status = 200, description = "The updated merchant", body = MeResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant with that id"),
        (status = 409, description = "Email already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Update merchant", skip_all)]
pub async fn update_merchant(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateMerchantInput>,
) -> AppResult<Json<MeResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let mut user = users::by_id(&mut conn, id)
        .await?
        .filter(|user| user.role == UserRole::Merchant)
        .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;

    if input.name.is_some() || input.email.is_some() {
        let name = match &input.name {
            Some(name) => {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => user.name.clone(),
        };
        let email = match &input.email {
            Some(email) => email.trim().to_lowercase(),
            None => user.email.clone().expect("merchants always have an email"),
        };
        user = users::update_merchant(&mut conn, id, name.as_deref(), &email).await?;
    }

    if let Some(is_active) = input.is_active
        && is_active != user.is_active
    {
        user = users::set_merchant_active(&mut conn, id, is_active)
            .await?
            .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;
    }

    Ok(Json(MeResponse::from(user)))
}

/// Hard-delete a merchant. Their stores and products follow via ON DELETE
/// CASCADE — this is permanent, and the platform confirms before calling.
#[utoipa::path(
    delete,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant user id")),
    responses(
        (status = 204, description = "Merchant deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant with that id"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Delete merchant", skip_all)]
pub async fn delete_merchant(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut conn = app.db_pool.acquire().await?;
    if !users::delete_user(&mut conn, id, UserRole::Merchant).await? {
        return Err(AppError::NotFound("merchant not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// One merchant account with everything the admin detail page shows: the
/// account itself and their stores with product counts. Admin sees facts;
/// the merchant manages the menu — it stays out of the admin's response.
/// Unknown ids and non-merchant ids are the same 404.
#[utoipa::path(
    get,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant user id")),
    responses(
        (status = 200, description = "The merchant and their stores", body = MerchantDetailResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant with that id"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Merchant detail", skip_all)]
pub async fn get_merchant(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<MerchantDetailResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let user = users::by_id(&mut conn, id)
        .await?
        .filter(|user| user.role == UserRole::Merchant)
        .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;
    let stores = domain::stores::store_summaries_for_merchant(&mut conn, user.id).await?;
    Ok(Json(MerchantDetailResponse {
        id: user.id,
        role: user.role,
        name: user.name,
        phone: user.phone,
        email: user.email,
        is_active: user.is_active,
        created_at: user.created_at,
        stores: stores.into_iter().map(AdminStoreResponse::from).collect(),
    }))
}

/// A store inside the admin's merchant detail — standing facts plus how
/// many products it has. No location or image until something displays them.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AdminStoreResponse {
    pub id: Uuid,
    pub name: String,
    pub address_text: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    pub product_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<domain::stores::StoreSummary> for AdminStoreResponse {
    fn from(store: domain::stores::StoreSummary) -> Self {
        Self {
            id: store.id,
            name: store.name,
            address_text: store.address_text,
            delivery_fee: store.delivery_fee,
            is_open: store.is_open,
            product_count: store.product_count,
            created_at: store.created_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantDetailResponse {
    pub id: Uuid,
    #[schema(value_type = String, examples("merchant"))]
    pub role: UserRole,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub stores: Vec<AdminStoreResponse>,
}

/// Platform-wide counts for the admin dashboard.
#[utoipa::path(
    get,
    path = "/v1/admin/summary",
    responses(
        (status = 200, description = "Platform-wide counts", body = AdminSummaryResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Platform summary")]
pub async fn summary(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
) -> AppResult<Json<AdminSummaryResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let merchants = users::count_by_role(&mut conn, UserRole::Merchant).await?;
    let customers = users::count_by_role(&mut conn, UserRole::Customer).await?;
    let catalog = domain::stores::catalog_counts(&mut conn).await?;
    Ok(Json(AdminSummaryResponse {
        merchants,
        customers,
        stores: catalog.stores,
        open_stores: catalog.open_stores,
        products: catalog.products,
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AdminSummaryResponse {
    pub merchants: i64,
    pub customers: i64,
    pub stores: i64,
    pub open_stores: i64,
    pub products: i64,
}

#[utoipa::path(
    get,
    path = "/v1/admin/customers",
    responses(
        (status = 200, description = "All customer accounts, oldest first", body = Vec<MeResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "List customers")]
pub async fn list_customers(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
) -> AppResult<Json<Vec<MeResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let customers = users::list_customers(&mut conn).await?;
    Ok(Json(customers.into_iter().map(MeResponse::from).collect()))
}

/// Admin edits to a customer account. Same semantics as merchant edits:
/// provided fields overwrite, an empty name clears it, absent fields keep
/// their value. Phone stays editable because a typo'd number during OTP
/// signup is exactly the kind of thing an admin corrects. `is_active`
/// rides the same endpoint.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateCustomerInput {
    #[validate(length(max = 100, message = "name must be at most 100 characters"))]
    pub name: Option<String>,
    #[validate(custom(function = "validate_phone"))]
    pub phone: Option<String>,
    pub is_active: Option<bool>,
}

/// Edit a customer (name, phone, active flag). A taken phone is a 409.
#[utoipa::path(
    patch,
    path = "/v1/admin/customers/{id}",
    params(("id" = Uuid, Path, description = "Customer user id")),
    request_body = UpdateCustomerInput,
    responses(
        (status = 200, description = "The updated customer", body = MeResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No customer with that id"),
        (status = 409, description = "Phone already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Update customer", skip_all)]
pub async fn update_customer(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateCustomerInput>,
) -> AppResult<Json<MeResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let mut user = users::by_id(&mut conn, id)
        .await?
        .filter(|user| user.role == UserRole::Customer)
        .ok_or_else(|| AppError::NotFound("customer not found".into()))?;

    if input.name.is_some() || input.phone.is_some() {
        let name = match &input.name {
            Some(name) => {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            None => user.name.clone(),
        };
        let phone = match &input.phone {
            Some(phone) => phone.trim().to_string(),
            None => user.phone.clone().expect("customers always have a phone"),
        };
        user = users::update_customer(&mut conn, id, name.as_deref(), &phone).await?;
    }

    if let Some(is_active) = input.is_active
        && is_active != user.is_active
    {
        user = users::set_customer_active(&mut conn, id, is_active)
            .await?
            .ok_or_else(|| AppError::NotFound("customer not found".into()))?;
    }

    Ok(Json(MeResponse::from(user)))
}

/// Hard-delete a customer. Permanent — the platform confirms before
/// calling. Scoped to customers: a merchant or admin id is a 404.
#[utoipa::path(
    delete,
    path = "/v1/admin/customers/{id}",
    params(("id" = Uuid, Path, description = "Customer user id")),
    responses(
        (status = 204, description = "Customer deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No customer with that id"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Delete customer", skip_all)]
pub async fn delete_customer(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut conn = app.db_pool.acquire().await?;
    if !users::delete_user(&mut conn, id, UserRole::Customer).await? {
        return Err(AppError::NotFound("customer not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
