//! Stores + products (S9, multi-store since S10b). Two audiences in one
//! area:
//! - `/v1/merchant/*` — merchants manage their stores and menus. Ownership
//!   is the signed-in user (`stores.merchant_id`), never a claim.
//! - `/v1/stores*` — customers browse open stores and available products.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::domain::stores::{self, NewProduct, ProductChanges, StoreChanges};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// A store as the API returns it. Integer RWF for money, ISO-8601 times.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreResponse {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub address_text: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub delivery_fee: i64,
    pub is_open: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<stores::Store> for StoreResponse {
    fn from(store: stores::Store) -> Self {
        Self {
            id: store.id,
            merchant_id: store.merchant_id,
            name: store.name,
            description: store.description,
            image_url: store.image_url,
            address_text: store.address_text,
            lat: store.lat,
            lng: store.lng,
            delivery_fee: store.delivery_fee,
            is_open: store.is_open,
            created_at: store.created_at,
            updated_at: store.updated_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProductResponse {
    pub id: Uuid,
    pub store_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub is_available: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<stores::Product> for ProductResponse {
    fn from(product: stores::Product) -> Self {
        Self {
            id: product.id,
            store_id: product.store_id,
            name: product.name,
            description: product.description,
            price: product.price,
            image_url: product.image_url,
            is_available: product.is_available,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreWithProductsResponse {
    pub store: StoreResponse,
    pub products: Vec<ProductResponse>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateStoreInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
    #[validate(length(max = 200, message = "address must be at most 200 characters"))]
    pub address_text: Option<String>,
    #[validate(range(min = -90.0, max = 90.0, message = "lat must be between -90 and 90"))]
    pub lat: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0, message = "lng must be between -180 and 180"))]
    pub lng: Option<f64>,
    /// Integer RWF. Defaults to 0 (free delivery).
    #[validate(range(min = 0, message = "delivery_fee must be 0 or more"))]
    pub delivery_fee: Option<i64>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateStoreInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: Option<String>,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
    #[validate(length(max = 200, message = "address must be at most 200 characters"))]
    pub address_text: Option<String>,
    #[validate(range(min = -90.0, max = 90.0, message = "lat must be between -90 and 90"))]
    pub lat: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0, message = "lng must be between -180 and 180"))]
    pub lng: Option<f64>,
    #[validate(range(min = 0, message = "delivery_fee must be 0 or more"))]
    pub delivery_fee: Option<i64>,
    pub is_open: Option<bool>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateProductInput {
    /// Which of the merchant's stores this product joins.
    pub store_id: Uuid,
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    /// Integer RWF, server-owned truth for the order snapshot later.
    #[validate(range(min = 0, message = "price must be 0 or more"))]
    pub price: i64,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
    /// Defaults to true.
    pub is_available: Option<bool>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateProductInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: Option<String>,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    #[validate(range(min = 0, message = "price must be 0 or more"))]
    pub price: Option<i64>,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
    pub is_available: Option<bool>,
}

/// PATCH text semantics: provided overwrites (trimmed), an empty string
/// clears the field, absent keeps the current value.
fn merge_text(provided: Option<String>, current: Option<String>) -> Option<String> {
    match provided {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => current,
    }
}

/// The signed-in merchant's id. The role guard already proved the user is a
/// merchant; this just lifts it out of the `Option`.
fn merchant_id(context: &UserContext) -> AppResult<Uuid> {
    context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

#[utoipa::path(
    post,
    path = "/v1/merchant/stores",
    request_body = CreateStoreInput,
    responses(
        (status = 201, description = "Store created (starts closed)", body = StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Create own store", skip_all)]
pub async fn create_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateStoreInput>,
) -> AppResult<(StatusCode, Json<StoreResponse>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }

    let mut conn = app.db_pool.acquire().await?;
    let store = stores::create_store(
        &mut conn,
        merchant_id(&context)?,
        StoreChanges {
            name,
            description: merge_text(input.description, None),
            image_url: merge_text(input.image_url, None),
            address_text: merge_text(input.address_text, None),
            lat: input.lat,
            lng: input.lng,
            delivery_fee: input.delivery_fee.unwrap_or(0),
            is_open: false,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(StoreResponse::from(store))))
}

#[utoipa::path(
    get,
    path = "/v1/merchant/stores",
    responses(
        (status = 200, description = "The merchant's stores, oldest first", body = Vec<StoreResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List own stores", skip_all)]
pub async fn list_own_stores(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
) -> AppResult<Json<Vec<StoreResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let stores = stores::stores_for_merchant(&mut conn, merchant_id(&context)?).await?;
    Ok(Json(stores.into_iter().map(StoreResponse::from).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/merchant/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 200, description = "One of the merchant's stores", body = StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "Not one of this merchant's stores"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Get own store", skip_all)]
pub async fn get_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StoreResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    let store = stores::store_for_merchant(&mut conn, merchant_id(&context)?, id).await?;
    Ok(Json(StoreResponse::from(store)))
}

#[utoipa::path(
    patch,
    path = "/v1/merchant/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    request_body = UpdateStoreInput,
    responses(
        (status = 200, description = "The updated store", body = StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "Not one of this merchant's stores"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Update own store", skip_all)]
pub async fn update_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateStoreInput>,
) -> AppResult<Json<StoreResponse>> {
    let mid = merchant_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let current = stores::store_for_merchant(&mut conn, mid, id).await?;

    let changes = StoreChanges {
        name: merge_text(input.name, Some(current.name)).expect("current name is never empty"),
        description: merge_text(input.description, current.description),
        image_url: merge_text(input.image_url, current.image_url),
        address_text: merge_text(input.address_text, current.address_text),
        lat: input.lat.or(current.lat),
        lng: input.lng.or(current.lng),
        delivery_fee: input.delivery_fee.unwrap_or(current.delivery_fee),
        is_open: input.is_open.unwrap_or(current.is_open),
    };
    let store = stores::update_store(&mut conn, mid, id, changes).await?;
    Ok(Json(StoreResponse::from(store)))
}

/// Hard-delete one of the merchant's stores. Its products follow via
/// ON DELETE CASCADE — permanent, and the platform confirms before calling.
#[utoipa::path(
    delete,
    path = "/v1/merchant/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 204, description = "Store deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "Not one of this merchant's stores"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete own store", skip_all)]
pub async fn delete_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut conn = app.db_pool.acquire().await?;
    stores::delete_store(&mut conn, merchant_id(&context)?, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/v1/merchant/products",
    responses(
        (status = 200, description = "The merchant's full menu, oldest first", body = Vec<ProductResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List own products", skip_all)]
pub async fn list_own_products(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
) -> AppResult<Json<Vec<ProductResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let products = stores::products_for_merchant(&mut conn, merchant_id(&context)?).await?;
    Ok(Json(
        products.into_iter().map(ProductResponse::from).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/merchant/products",
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Product added to the menu", body = ProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "The store_id is not one of this merchant's stores"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Create product", skip_all)]
pub async fn create_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateProductInput>,
) -> AppResult<(StatusCode, Json<ProductResponse>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }

    let mut conn = app.db_pool.acquire().await?;
    let product = stores::create_product(
        &mut conn,
        merchant_id(&context)?,
        NewProduct {
            store_id: input.store_id,
            name,
            description: merge_text(input.description, None),
            price: input.price,
            image_url: merge_text(input.image_url, None),
            is_available: input.is_available.unwrap_or(true),
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ProductResponse::from(product))))
}

#[utoipa::path(
    patch,
    path = "/v1/merchant/products/{id}",
    params(("id" = Uuid, Path, description = "Product id")),
    request_body = UpdateProductInput,
    responses(
        (status = 200, description = "The updated product", body = ProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "Not one of this merchant's products"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Update product", skip_all)]
pub async fn update_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateProductInput>,
) -> AppResult<Json<ProductResponse>> {
    let mid = merchant_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let current = stores::product_for_merchant(&mut conn, mid, id).await?;

    let changes = ProductChanges {
        name: merge_text(input.name, Some(current.name)).expect("current name is never empty"),
        description: merge_text(input.description, current.description),
        price: input.price.unwrap_or(current.price),
        image_url: merge_text(input.image_url, current.image_url),
        is_available: input.is_available.unwrap_or(current.is_available),
    };
    let product = stores::update_product(&mut conn, mid, id, changes).await?;
    Ok(Json(ProductResponse::from(product)))
}

/// Hard-delete one of the merchant's products. Permanent — the platform
/// confirms before calling.
#[utoipa::path(
    delete,
    path = "/v1/merchant/products/{id}",
    params(("id" = Uuid, Path, description = "Product id")),
    responses(
        (status = 204, description = "Product deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a merchant"),
        (status = 404, description = "Not one of this merchant's products"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete product", skip_all)]
pub async fn delete_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let mut conn = app.db_pool.acquire().await?;
    stores::delete_product(&mut conn, merchant_id(&context)?, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/v1/stores",
    responses(
        (status = 200, description = "Open stores, oldest first", body = Vec<StoreResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
    ),
    tag = "stores"
)]
#[tracing::instrument(name = "List stores")]
pub async fn list_stores(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
) -> AppResult<Json<Vec<StoreResponse>>> {
    let mut conn = app.db_pool.acquire().await?;
    let stores = stores::open_stores(&mut conn).await?;
    Ok(Json(stores.into_iter().map(StoreResponse::from).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 200, description = "The store and its available products", body = StoreWithProductsResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
        (status = 404, description = "No open store with that id"),
    ),
    tag = "stores"
)]
#[tracing::instrument(name = "Get store", skip_all)]
pub async fn get_store(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StoreWithProductsResponse>> {
    let mut conn = app.db_pool.acquire().await?;
    // Closed stores are indistinguishable from missing ones for customers.
    let store = stores::store_by_id(&mut conn, id)
        .await?
        .filter(|store| store.is_open)
        .ok_or_else(|| AppError::NotFound("store not found".into()))?;
    let products = stores::products_for_store(&mut conn, store.id, true).await?;

    Ok(Json(StoreWithProductsResponse {
        store: StoreResponse::from(store),
        products: products.into_iter().map(ProductResponse::from).collect(),
    }))
}
