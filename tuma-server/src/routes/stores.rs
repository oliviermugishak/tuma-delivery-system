//! Stores (the fulfillment boundary). Two audiences in one area:
//! - `/v1/merchant/*` — merchant operators manage their business's stores.
//!   Authorization comes from the account's merchant memberships: owners
//!   cover the whole business, store-scoped managers see only their store,
//!   and a foreign store is indistinguishable from a missing one.
//! - `/v1/stores*` — customers browse open stores.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::domain::catalog;
use crate::domain::geo;
use crate::domain::stores::{self, StoreChanges};
use axum::extract::{Path, Query, State};
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
    pub category: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    /// Straight-line meters from the customer, when the request carried
    /// `lat`/`lng` and the store has coordinates. Server-computed — the
    /// client never does geo math.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<i64>,
    /// Pure ride time at Kigali's effective moto speed (`~` prefix in the
    /// UI). Present exactly when `distance_m` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<i64>,
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
            category: store.category,
            delivery_fee: store.delivery_fee,
            is_open: store.is_open,
            distance_m: None,
            eta_min: None,
            created_at: store.created_at,
            updated_at: store.updated_at,
        }
    }
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
    /// What the store sells, in one word or two (e.g. "Grill", "Bakery").
    #[validate(length(max = 50, message = "category must be at most 50 characters"))]
    pub category: Option<String>,
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
    #[validate(length(max = 50, message = "category must be at most 50 characters"))]
    pub category: Option<String>,
    #[validate(range(min = 0, message = "delivery_fee must be 0 or more"))]
    pub delivery_fee: Option<i64>,
    pub is_open: Option<bool>,
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

/// The signed-in operator's merchant surface. The `require_merchant` guard
/// already proved a membership exists; this lifts it out.
fn merchant_access(context: &UserContext) -> AppResult<crate::app::MerchantAccess> {
    context
        .merchant_access()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

#[utoipa::path(
    post,
    path = "/v1/merchant/stores",
    request_body = CreateStoreInput,
    responses(
        (status = 201, description = "Store created (starts closed)", body = StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or not the business owner"),
        (status = 400, description = "Account belongs to several businesses"),
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
    // Creating a store is a business-level act: owners only, and the
    // account must work for exactly one business.
    let access = merchant_access(&context)?;
    let grant = access.single_grant()?;
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can create stores".into(),
        ));
    }

    let mut conn = app.db_pool.acquire().await?;
    let store = stores::create_store(
        &mut conn,
        grant.merchant_id,
        StoreChanges {
            name,
            description: merge_text(input.description, None),
            image_url: merge_text(input.image_url, None),
            address_text: merge_text(input.address_text, None),
            lat: input.lat,
            lng: input.lng,
            category: merge_text(input.category, None),
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
        (status = 200, description = "The stores this operator can reach, oldest first", body = Vec<StoreResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List own stores", skip_all)]
pub async fn list_own_stores(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
) -> AppResult<Json<Vec<StoreResponse>>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let stores = stores::stores_for_grants(&mut conn, &access.grants).await?;
    Ok(Json(stores.into_iter().map(StoreResponse::from).collect()))
}

#[utoipa::path(
    get,
    path = "/v1/merchant/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 200, description = "One of this operator's stores", body = StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's stores"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Get own store", skip_all)]
pub async fn get_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<StoreResponse>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    // Resolve the grant that owns this store first: the scoping is
    // per-merchant, so the store tells us which grant applies.
    let store = stores::store_by_id(&mut conn, id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or(AppError::NotFound(stores::StoreError::NotFound.to_string()))?;
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
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's stores"),
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
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let store = stores::store_by_id(&mut conn, id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or(AppError::NotFound(stores::StoreError::NotFound.to_string()))?;

    let changes = StoreChanges {
        name: merge_text(input.name, Some(store.name.clone()))
            .expect("current name is never empty"),
        description: merge_text(input.description, store.description),
        image_url: merge_text(input.image_url, store.image_url),
        address_text: merge_text(input.address_text, store.address_text),
        lat: input.lat.or(store.lat),
        lng: input.lng.or(store.lng),
        category: merge_text(input.category, store.category),
        delivery_fee: input.delivery_fee.unwrap_or(store.delivery_fee),
        is_open: input.is_open.unwrap_or(store.is_open),
    };
    let store = stores::update_store(&mut conn, store.id, changes).await?;
    Ok(Json(StoreResponse::from(store)))
}

/// Hard-delete one of the operator's stores. Its store_products follow via
/// ON DELETE CASCADE — permanent, and the platform confirms before calling.
/// Owners only: a store-scoped manager cannot delete their store.
#[utoipa::path(
    delete,
    path = "/v1/merchant/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 204, description = "Store deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or not the business owner"),
        (status = 404, description = "Not one of this operator's stores"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete own store", skip_all)]
pub async fn delete_own_store(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let store = stores::store_by_id(&mut conn, id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or(AppError::NotFound(stores::StoreError::NotFound.to_string()))?;

    let grant = access
        .grants
        .iter()
        .find(|grant| grant.merchant_id == store.merchant_id)
        .expect("grant exists — access was just checked");
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can delete stores".into(),
        ));
    }

    stores::delete_store(&mut conn, store.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Optional customer location on the browse request. Both or neither —
/// one coordinate alone is meaningless, so it's a 400, not a guess.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LocateQuery {
    /// Customer latitude, -90..90.
    pub lat: Option<f64>,
    /// Customer longitude, -180..180.
    pub lng: Option<f64>,
}

#[utoipa::path(
    get,
    path = "/v1/stores",
    params(("lat" = Option<f64>, Query, description = "Customer latitude (-90..90); requires lng"),
           ("lng" = Option<f64>, Query, description = "Customer longitude (-180..180); requires lat")),
    responses(
        (status = 200, description = "Open stores, oldest first — or nearest first when the request carries a location (coordinate-less stores trail)", body = Vec<StoreResponse>),
        (status = 400, description = "Only one of lat/lng, or a coordinate out of range / not a number"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
    ),
    tag = "stores"
)]
#[tracing::instrument(name = "List stores", skip_all)]
pub async fn list_stores(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Query(query): Query<LocateQuery>,
) -> AppResult<Json<Vec<StoreResponse>>> {
    // Location validation at the door: both-or-neither, finite, in range.
    let customer = match (query.lat, query.lng) {
        (None, None) => None,
        (Some(lat), Some(lng)) => {
            let valid = lat.is_finite()
                && lng.is_finite()
                && (-90.0..=90.0).contains(&lat)
                && (-180.0..=180.0).contains(&lng);
            if !valid {
                return Err(AppError::BadRequest(
                    "lat must be -90..90 and lng -180..180 — both or neither".into(),
                ));
            }
            Some((lat, lng))
        }
        _ => {
            return Err(AppError::BadRequest(
                "lat and lng must be provided together".into(),
            ));
        }
    };

    let mut conn = app.db_pool.acquire().await?;
    let stores = stores::open_stores(&mut conn).await?;
    let mut responses: Vec<StoreResponse> = stores.into_iter().map(StoreResponse::from).collect();

    if let Some((customer_lat, customer_lng)) = customer {
        for response in &mut responses {
            if let (Some(lat), Some(lng)) = (response.lat, response.lng) {
                let distance_m = geo::haversine_m(customer_lat, customer_lng, lat, lng);
                response.distance_m = Some(distance_m);
                response.eta_min = Some(geo::ride_minutes(distance_m));
            }
        }
        // Nearest first; coordinate-less stores trail, their creation
        // order preserved. `sort_by` is stable, so equal distances keep
        // their relative order.
        responses.sort_by_key(|response| response.distance_m.unwrap_or(i64::MAX));
    }

    Ok(Json(responses))
}

#[utoipa::path(
    get,
    path = "/v1/stores/{id}",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 200, description = "The store and its menu", body = StoreWithProductsResponse),
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
    let products = catalog::menu_for_store(&mut conn, store.id).await?;

    Ok(Json(StoreWithProductsResponse {
        store: StoreResponse::from(store),
        products: products.into_iter().map(MenuItemResponse::from).collect(),
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreWithProductsResponse {
    pub store: StoreResponse,
    pub products: Vec<MenuItemResponse>,
}

/// A placeholder until the S3 catalog slice lands: the store detail's menu
/// is empty because store_products have no endpoints yet. The mobile store
/// screen renders the honest empty state.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub is_available: bool,
}

impl From<catalog::MenuItem> for MenuItemResponse {
    fn from(item: catalog::MenuItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            description: item.description,
            price: item.price,
            image_url: item.image_url,
            is_available: item.is_available,
        }
    }
}
