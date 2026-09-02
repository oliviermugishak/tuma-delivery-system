//! Admin-only endpoints: the platform control plane. Admins provision
//! merchant BUSINESSES (each with an owner account + membership), manage
//! customer accounts, manage Tuma's riders, and read platform-wide facts.
//! Authorization is the admins profile row, resolved fresh per request by
//! the guard.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::routes::auth::validate_phone;
use crate::routes::orders::PageQuery;
use accounts::customers;
use accounts::merchants::{self, Merchant, MerchantStatus};
use accounts::riders::{self, Rider, RiderError};
use accounts::users;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// A merchant business as the admin sees it.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantResponse {
    pub id: Uuid,
    pub name: String,
    pub business_email: Option<String>,
    pub business_phone: Option<String>,
    #[schema(value_type = String)]
    pub status: MerchantStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<Merchant> for MerchantResponse {
    fn from(merchant: Merchant) -> Self {
        Self {
            id: merchant.id,
            name: merchant.name,
            business_email: merchant.business_email,
            business_phone: merchant.business_phone,
            status: merchant.status,
            created_at: merchant.created_at,
        }
    }
}

/// Provision a merchant business: the business row, the owner's account
/// (email + password, hashed by the AccountManager), and the owner
/// membership — one transaction. The owner signs into the merchant wing
/// with that account; the business itself never logs in.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateMerchantInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
    #[validate(email(message = "business_email must be a valid address"))]
    pub business_email: Option<String>,
    #[validate(email(message = "email must be a valid address"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "password must be 8-128 characters"))]
    pub password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CreatedMerchantResponse {
    #[serde(flatten)]
    pub merchant: MerchantResponse,
    pub owner_email: String,
}

#[utoipa::path(
    post,
    path = "/v1/admin/merchants",
    request_body = CreateMerchantInput,
    responses(
        (status = 201, description = "Merchant business created with its owner account", body = CreatedMerchantResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 409, description = "Owner email already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Create merchant", skip_all)]
pub async fn create_merchant(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateMerchantInput>,
) -> AppResult<(StatusCode, Json<CreatedMerchantResponse>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    let owner_email = input.email.trim().to_lowercase();
    let business_email = input
        .business_email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_lowercase);

    let mut conn = app.db_pool.acquire().await?;
    let mut tx = conn.begin().await?;
    let merchant = merchants::create(&mut tx, &name, business_email.as_deref(), None).await?;
    let owner = app
        .accounts
        .create_password_account(&mut tx, &owner_email, &input.password)
        .await?;
    accounts::memberships::create(
        &mut tx,
        owner.id,
        merchant.id,
        accounts::MembershipRole::Owner,
        None,
    )
    .await?;
    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedMerchantResponse {
            merchant: MerchantResponse::from(merchant),
            owner_email,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/v1/admin/merchants",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "Merchant businesses, oldest first, one page", body = Vec<MerchantResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "List merchants", skip_all)]
pub async fn list_merchants(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<MerchantResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let merchants = merchants::list(&mut conn, page.limit(), page.offset()).await?;
    Ok(Json(
        merchants.into_iter().map(MerchantResponse::from).collect(),
    ))
}

/// One business with everything the admin detail page shows: the business,
/// its stores with assortment counts, and its members. Admin sees facts;
/// the merchant manages the menu — it stays out of the admin's response.
#[utoipa::path(
    get,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant business id")),
    responses(
        (status = 200, description = "The business, its stores, and its members", body = MerchantDetailResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant business with that id"),
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
    let merchant = merchants::by_id(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;
    let stores = marketplace::stores::store_summaries_for_merchant(&mut conn, merchant.id).await?;
    let members = accounts::memberships::list_for_merchant(&mut conn, merchant.id).await?;
    Ok(Json(MerchantDetailResponse {
        merchant: MerchantResponse::from(merchant),
        stores: stores.into_iter().map(AdminStoreResponse::from).collect(),
        members: members
            .into_iter()
            .map(|member| MemberResponse {
                user_id: member.user_id,
                email: member.email,
                role: member.role,
                store_id: member.store_id,
                created_at: member.created_at,
            })
            .collect(),
    }))
}

/// A store inside the admin's business detail — standing facts plus how
/// many store products it sells. No location or image until something
/// displays them.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AdminStoreResponse {
    pub id: Uuid,
    pub name: String,
    pub address_text: Option<String>,
    pub category: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    pub product_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<marketplace::stores::StoreSummary> for AdminStoreResponse {
    fn from(store: marketplace::stores::StoreSummary) -> Self {
        Self {
            id: store.id,
            name: store.name,
            address_text: store.address_text,
            category: store.category,
            delivery_fee: store.delivery_fee,
            is_open: store.is_open,
            product_count: store.product_count,
            created_at: store.created_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MemberResponse {
    pub user_id: Uuid,
    pub email: Option<String>,
    #[schema(value_type = String)]
    pub role: accounts::MembershipRole,
    pub store_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantDetailResponse {
    #[serde(flatten)]
    pub merchant: MerchantResponse,
    pub stores: Vec<AdminStoreResponse>,
    pub members: Vec<MemberResponse>,
}

/// Admin edits to a business. Every field is optional: provided fields
/// overwrite, an empty string clears an optional contact field, absent
/// fields keep their value. Suspension is the pause tool — a suspended
/// business is refused by the merchant wing and stops taking orders.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateMerchantInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: Option<String>,
    #[validate(email(message = "business_email must be a valid address"))]
    pub business_email: Option<String>,
    // Contact, not identity — but still one phone dialect: E.164 with the
    // country code, same as every other number in the system.
    #[validate(custom(function = "validate_phone"))]
    pub business_phone: Option<String>,
    #[schema(value_type = String)]
    pub status: Option<MerchantStatus>,
}

#[utoipa::path(
    patch,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant business id")),
    request_body = UpdateMerchantInput,
    responses(
        (status = 200, description = "The updated business", body = MerchantResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant business with that id"),
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
) -> AppResult<Json<MerchantResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let merchant = merchants::by_id(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;

    let name = match input.name {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest("name must not be empty".into()));
            }
            trimmed.to_string()
        }
        None => merchant.name.clone(),
    };
    let business_email = match input.business_email {
        Some(value) => {
            let trimmed = value.trim().to_lowercase();
            (!trimmed.is_empty()).then_some(trimmed)
        }
        None => merchant.business_email.clone(),
    };
    let business_phone = match input.business_phone {
        Some(value) => {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }
        None => merchant.business_phone.clone(),
    };
    let status = input.status.unwrap_or(merchant.status);

    let updated = merchants::update(
        &mut conn,
        id,
        &name,
        business_email.as_deref(),
        business_phone.as_deref(),
        status,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("merchant not found".into()))?;
    Ok(Json(MerchantResponse::from(updated)))
}

/// Hard-delete a business. Its memberships, stores (and their assortments),
/// and catalog products follow via ON DELETE CASCADE. Member ACCOUNTS are
/// identities, not parts of the business — they survive the delete.
#[utoipa::path(
    delete,
    path = "/v1/admin/merchants/{id}",
    params(("id" = Uuid, Path, description = "Merchant business id")),
    responses(
        (status = 204, description = "Merchant business deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No merchant business with that id"),
        (status = 409, description = "The business has order history — suspend it instead"),
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
    // With order history the delete is a 409 naming the remedy (suspend);
    // the From impl carries it.
    if !merchants::delete(&mut conn, id).await? {
        return Err(AppError::NotFound("merchant not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
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
    let merchant_count = merchants::count(&mut conn).await?;
    let customer_count = customers::count(&mut conn).await?;
    let catalog = marketplace::stores::catalog_counts(&mut conn).await?;
    let orders_in_progress = commerce::orders::count_in_progress(&mut conn).await?;
    Ok(Json(AdminSummaryResponse {
        merchants: merchant_count,
        customers: customer_count,
        stores: catalog.stores,
        open_stores: catalog.open_stores,
        products: catalog.products,
        store_products: catalog.store_products,
        orders_in_progress,
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AdminSummaryResponse {
    /// Merchant businesses.
    pub merchants: i64,
    pub customers: i64,
    pub stores: i64,
    pub open_stores: i64,
    /// Merchant-level catalog products.
    pub products: i64,
    /// Store-level sellable items.
    pub store_products: i64,
    /// Store orders still moving (not delivered/cancelled).
    pub orders_in_progress: i64,
}

/// A customer account on the admin's list: the profile (name) joined with
/// the account facts (phone, active flag).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CustomerAdminResponse {
    pub user_id: Uuid,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<customers::CustomerListRow> for CustomerAdminResponse {
    fn from(row: customers::CustomerListRow) -> Self {
        Self {
            user_id: row.user_id,
            name: row.name,
            phone: row.phone,
            is_active: row.is_active,
            created_at: row.created_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/admin/customers",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "Customer accounts, oldest first, one page", body = Vec<CustomerAdminResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "List customers", skip_all)]
pub async fn list_customers(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<CustomerAdminResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let customers = customers::list_customers(&mut conn, page.limit(), page.offset()).await?;
    Ok(Json(
        customers
            .into_iter()
            .map(CustomerAdminResponse::from)
            .collect(),
    ))
}

/// Admin edits to a customer account. Provided fields overwrite, an empty
/// name clears it, absent fields keep their value. Phone stays editable
/// because a typo'd number during OTP signup is exactly the kind of thing
/// an admin corrects; a taken phone is the typed 409.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateCustomerInput {
    #[validate(length(max = 100, message = "name must be at most 100 characters"))]
    pub name: Option<String>,
    #[validate(custom(function = "validate_phone"))]
    pub phone: Option<String>,
    pub is_active: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/v1/admin/customers/{id}",
    params(("id" = Uuid, Path, description = "Customer account id")),
    request_body = UpdateCustomerInput,
    responses(
        (status = 200, description = "The updated customer", body = CustomerAdminResponse),
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
) -> AppResult<Json<CustomerAdminResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    // Scope to customer accounts: an admin's or operator's id is a 404, so
    // this endpoint can never touch a non-customer identity.
    let current = customers::list_row_for_user(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("customer not found".into()))?;

    if let Some(name) = &input.name {
        let trimmed = name.trim();
        let cleared = trimmed.is_empty();
        customers::update_name(
            &mut conn,
            id,
            (!cleared).then(|| trimmed.to_string()).as_deref(),
        )
        .await?;
    }

    if let Some(phone) = &input.phone {
        let phone = phone.trim().to_string();
        if phone != current.phone.unwrap_or_default() {
            users::update_phone(&mut conn, id, &phone).await?;
        }
    }

    if let Some(is_active) = input.is_active
        && is_active != current.is_active
    {
        users::set_active(&mut conn, id, is_active)
            .await?
            .ok_or_else(|| AppError::NotFound("customer not found".into()))?;
    }

    let row = customers::list_row_for_user(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("customer not found".into()))?;
    Ok(Json(CustomerAdminResponse::from(row)))
}

/// Hard-delete a customer account. The profile, refresh tokens, and order
/// history follow via ON DELETE CASCADE — permanent, and the platform
/// confirms before calling. Scoped to customer accounts: any other identity
/// is a 404.
#[utoipa::path(
    delete,
    path = "/v1/admin/customers/{id}",
    params(("id" = Uuid, Path, description = "Customer account id")),
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
    if !customers::delete_customer_account(&mut conn, id).await? {
        return Err(AppError::NotFound("customer not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// A rider as the admin sees them.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RiderAdminResponse {
    pub id: Uuid,
    pub rider_number: i64,
    pub name: String,
    pub phone: String,
    /// `false` = not assignable at handoff (the rider can still sign in).
    pub is_active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<Rider> for RiderAdminResponse {
    fn from(rider: Rider) -> Self {
        Self {
            id: rider.id,
            rider_number: rider.rider_number,
            name: rider.name,
            phone: rider.phone,
            is_active: rider.is_active,
            created_at: rider.created_at,
        }
    }
}

/// Create a Tuma rider: an OTP account (phone only — the rider signs in
/// with the exact customer flow) plus the rider profile with its unique
/// rider number, in one transaction. The number is generated server-side;
/// the merchant will ask the rider for it at handoff.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateRiderInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
    #[validate(custom(function = "validate_phone"))]
    pub phone: String,
}

#[utoipa::path(
    post,
    path = "/v1/admin/riders",
    request_body = CreateRiderInput,
    responses(
        (status = 201, description = "Rider created with its OTP account and unique rider number", body = RiderAdminResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 409, description = "Phone already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Create rider", skip_all)]
pub async fn create_rider(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateRiderInput>,
) -> AppResult<(StatusCode, Json<RiderAdminResponse>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    let phone = input.phone.trim().to_string();

    let mut conn = app.db_pool.acquire().await?;
    let mut tx = conn.begin().await?;
    let account = app.accounts.create_phone_account(&mut tx, &phone).await?;
    let rider = riders::create(&mut tx, account.id, &name, &phone).await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(RiderAdminResponse::from(rider))))
}

#[utoipa::path(
    get,
    path = "/v1/admin/riders",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "Riders, oldest first, one page", body = Vec<RiderAdminResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "List riders", skip_all)]
pub async fn list_riders(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<RiderAdminResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let riders = riders::list(&mut conn, page.limit(), page.offset()).await?;
    Ok(Json(
        riders.into_iter().map(RiderAdminResponse::from).collect(),
    ))
}

/// Admin edits to a rider. Provided fields overwrite, absent fields keep
/// their value. A phone edit changes the OTP anchor (the account) and the
/// rider's display phone together; a taken phone is the typed 409. An
/// empty name is a 400 — rider names are NOT NULL, there is nothing to
/// clear to.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateRiderInput {
    #[validate(length(max = 100, message = "name must be at most 100 characters"))]
    pub name: Option<String>,
    #[validate(custom(function = "validate_phone"))]
    pub phone: Option<String>,
    pub is_active: Option<bool>,
}

#[utoipa::path(
    patch,
    path = "/v1/admin/riders/{id}",
    params(("id" = Uuid, Path, description = "Rider id")),
    request_body = UpdateRiderInput,
    responses(
        (status = 200, description = "The updated rider", body = RiderAdminResponse),
        (status = 400, description = "Empty name"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No rider with that id"),
        (status = 409, description = "Phone already taken"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Update rider", skip_all)]
pub async fn update_rider(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateRiderInput>,
) -> AppResult<Json<RiderAdminResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let current = riders::by_id(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("rider not found".into()))?;

    let name = match input.name {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest("name must not be empty".into()));
            }
            trimmed.to_string()
        }
        None => current.name.clone(),
    };
    let phone = match input.phone {
        Some(phone) => phone.trim().to_string(),
        None => current.phone.clone(),
    };
    let is_active = input.is_active.unwrap_or(current.is_active);

    let mut tx = conn.begin().await?;
    if phone != current.phone {
        users::update_phone(&mut tx, current.account_id, &phone).await?;
    }
    let updated = riders::update(&mut tx, id, &name, &phone, is_active)
        .await?
        .ok_or_else(|| AppError::NotFound("rider not found".into()))?;
    tx.commit().await?;
    Ok(Json(RiderAdminResponse::from(updated)))
}

/// Hard-delete a rider's account (the profile cascades) — the remedy for a
/// typo'd phone at creation. Blocked with a typed 409 when any delivery
/// ever referenced the rider: assignment history is operationally real,
/// deactivation is the tool for a rider who stops riding.
#[utoipa::path(
    delete,
    path = "/v1/admin/riders/{id}",
    params(("id" = Uuid, Path, description = "Rider id")),
    responses(
        (status = 204, description = "Rider deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not an admin"),
        (status = 404, description = "No rider with that id"),
        (status = 409, description = "Rider has delivery history — deactivate instead"),
    ),
    tag = "admin"
)]
#[tracing::instrument(name = "Delete rider", skip_all)]
pub async fn delete_rider(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut conn = app.db_pool.acquire().await?;
    // Scope to rider ids: any other identity is the same 404.
    let rider = riders::by_id(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("rider not found".into()))?;
    match riders::delete_account(&mut conn, rider.account_id).await? {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        // 0 rows: the account vanished mid-request — same answer as missing.
        Ok(false) => Err(AppError::NotFound("rider not found".into())),
        Err(RiderError::HasDeliveries) => Err(AppError::Conflict(
            "this rider has delivery history — deactivate instead".into(),
        )),
        Err(other) => Err(other.into()),
    }
}
