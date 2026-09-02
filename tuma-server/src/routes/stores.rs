//! Stores (the fulfillment boundary). Two audiences in one area:
//! - `/v1/merchant/*` — merchant operators manage their business's stores.
//!   Authorization comes from the account's merchant memberships: owners
//!   cover the whole business, store-scoped managers see only their store,
//!   and a foreign store is indistinguishable from a missing one.
//! - `/v1/stores*` — customers browse open stores.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use marketplace::catalog;
use marketplace::geo;
use marketplace::stores::{self, StoreChanges};
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
    /// The store's contact surface — the app's Get-help / store-info
    /// sheets call and email the store with these.
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl StoreResponse {
    /// Compose against the environment's public storage base: an uploaded
    /// banner wins, a legacy external URL falls through, both absent means
    /// no image (clients render their placeholder).
    pub fn from_store(store: stores::Store, base_url: &str) -> Self {
        Self {
            id: store.id,
            merchant_id: store.merchant_id,
            name: store.name,
            description: store.description,
            image_url: storage::resolve_image_url(
                base_url,
                store.banner_key.as_deref(),
                store.image_url.as_deref(),
            ),
            address_text: store.address_text,
            lat: store.lat,
            lng: store.lng,
            category: store.category,
            delivery_fee: store.delivery_fee,
            is_open: store.is_open,
            contact_phone: store.contact_phone,
            contact_email: store.contact_email,
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
    /// The customer-facing contact surface (Get help / store info).
    #[validate(length(max = 30, message = "contact_phone must be at most 30 characters"))]
    pub contact_phone: Option<String>,
    #[validate(length(max = 200, message = "contact_email must be at most 200 characters"))]
    pub contact_email: Option<String>,
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
    #[validate(length(max = 30, message = "contact_phone must be at most 30 characters"))]
    pub contact_phone: Option<String>,
    #[validate(length(max = 200, message = "contact_email must be at most 200 characters"))]
    pub contact_email: Option<String>,
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
            contact_phone: input.contact_phone,
            contact_email: input.contact_email,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(StoreResponse::from_store(
            store,
            &app.storage.public_base_url,
        )),
    ))
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
    let stores = stores::stores_for_scopes(&mut conn, &access.store_scopes()).await?;
    Ok(Json(
        stores
            .into_iter()
            .map(|s| StoreResponse::from_store(s, &app.storage.public_base_url))
            .collect(),
    ))
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
    Ok(Json(StoreResponse::from_store(
        store,
        &app.storage.public_base_url,
    )))
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
        (status = 409, description = "Changed by someone else since the read — reload and retry"),
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
        contact_phone: merge_text(input.contact_phone, store.contact_phone),
        contact_email: merge_text(input.contact_email, store.contact_email),
    };
    // The row we just merged against is the precondition: if a second
    // writer moved it in between, the domain refuses — no silent
    // last-write-wins. (0 rows on a row we just read is that race, so
    // NotFound here would lie; map it to the conflict.)
    let store = stores::update_store(&mut conn, store.id, store.updated_at, changes)
        .await
        .map_err(|error| match error {
            stores::StoreError::NotFound => {
                AppError::Conflict(stores::StoreError::Stale.to_string())
            }
            other => other.into(),
        })?;
    Ok(Json(StoreResponse::from_store(
        store,
        &app.storage.public_base_url,
    )))
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
        (status = 409, description = "The store has order history — close it instead"),
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

/// Optional customer location and search term on the browse request.
/// Both-or-neither coordinates — one coordinate alone is meaningless, so
/// it's a 400, not a guess. A search term present after trimming drives
/// server-side matching; absent means the plain feed.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct LocateQuery {
    /// Customer latitude, -90..90.
    pub lat: Option<f64>,
    /// Customer longitude, -180..180.
    pub lng: Option<f64>,
    /// Search stores by name or category (case-insensitive substring).
    pub q: Option<String>,
}

/// Optional customer location and search term validation shared by the
/// browse feed and discovery: both-or-neither coordinates, finite, in
/// range; trimmed search term capped at 100 characters.
pub fn validated_coords(
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<Option<(f64, f64)>, AppError> {
    match (lat, lng) {
        (None, None) => Ok(None),
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
            Ok(Some((lat, lng)))
        }
        _ => Err(AppError::BadRequest(
            "lat and lng must be provided together".into(),
        )),
    }
}

pub fn validated_term(q: Option<String>) -> Result<Option<String>, AppError> {
    // Trim to nothing = absent; a bound on length keeps one request from
    // asking Postgres to think hard about a 10 KB string.
    let term = q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    if term.is_some_and(|raw| raw.chars().count() > 100) {
        return Err(AppError::BadRequest(
            "search term must be 100 characters or fewer".into(),
        ));
    }
    Ok(term.map(str::to_string))
}

/// Rows the feed can place: coordinates in, distance/ETA out. One trait
/// for stores and product hits keeps the geo presentation in one place.
pub trait Located {
    fn coords(&self) -> (Option<f64>, Option<f64>);
    fn distance_m(&self) -> Option<i64>;
    fn set_distance(&mut self, distance_m: i64, eta_min: i64);
}

/// Attaches server-computed distance/ETA when the request carried a
/// location. Existing server order is preserved.
pub fn attach_distance<T: Located>(rows: &mut [T], customer: Option<(f64, f64)>) {
    let Some((customer_lat, customer_lng)) = customer else {
        return;
    };
    for row in rows.iter_mut() {
        let (Some(lat), Some(lng)) = row.coords() else {
            continue;
        };
        let distance_m = geo::haversine_m(customer_lat, customer_lng, lat, lng);
        row.set_distance(distance_m, geo::ride_minutes(distance_m));
    }
}

/// [`attach_distance`] plus the nearest-first sort (stable — equal
/// distances keep their relative order, coordinate-less rows trail).
pub fn attach_distance_and_sort<T: Located>(rows: &mut [T], customer: Option<(f64, f64)>) {
    attach_distance(rows, customer);
    rows.sort_by_key(|row| row.distance_m().unwrap_or(i64::MAX));
}

impl Located for StoreResponse {
    fn coords(&self) -> (Option<f64>, Option<f64>) {
        (self.lat, self.lng)
    }
    fn distance_m(&self) -> Option<i64> {
        self.distance_m
    }
    fn set_distance(&mut self, distance_m: i64, eta_min: i64) {
        self.distance_m = Some(distance_m);
        self.eta_min = Some(eta_min);
    }
}

#[utoipa::path(
    get,
    path = "/v1/stores",
    params(("lat" = Option<f64>, Query, description = "Customer latitude (-90..90); requires lng"),
           ("lng" = Option<f64>, Query, description = "Customer longitude (-180..180); requires lat"),
           ("q" = Option<String>, Query, description = "Search term matched against store name and category; empty = no filter")),
    responses(
        (status = 200, description = "Open stores, oldest first — or nearest first when the request carries a location (coordinate-less stores trail); filtered to name/category matches when q is present", body = Vec<StoreResponse>),
        (status = 400, description = "Only one of lat/lng, a coordinate out of range / not a number, or a search term over 100 characters"),
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
    let customer = validated_coords(query.lat, query.lng)?;
    let term = validated_term(query.q)?;

    let mut conn = app.db_pool.acquire().await?;
    let stores = match term.as_deref() {
        Some(raw) => {
            let pattern = format!("%{}%", stores::escape_like(raw));
            stores::search_stores(&mut conn, &pattern).await?
        }
        None => stores::open_stores(&mut conn).await?,
    };
    let base_url = app.storage.public_base_url.clone();
    let mut responses: Vec<StoreResponse> = stores
        .into_iter()
        .map(|store| StoreResponse::from_store(store, &base_url))
        .collect();
    attach_distance_and_sort(&mut responses, customer);

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
        store: StoreResponse::from_store(store, &app.storage.public_base_url),
        products: products
            .into_iter()
            .map(|item| MenuItemResponse::from_item(item, &app.storage.public_base_url))
            .collect(),
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreWithProductsResponse {
    pub store: StoreResponse,
    pub products: Vec<MenuItemResponse>,
}

/// One item of a store's menu, as the customer sees it.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    /// The full gallery, cover first (composed URLs). Empty when the
    /// product has no uploads.
    pub images: Vec<String>,
    pub is_available: bool,
}

impl MenuItemResponse {
    /// Gallery cover wins; the product's legacy external URL falls through.
    pub fn from_item(item: catalog::MenuItem, base_url: &str) -> Self {
        Self {
            id: item.id,
            name: item.name,
            description: item.description,
            price: item.price,
            image_url: storage::resolve_image_url(
                base_url,
                item.cover_key.as_deref(),
                item.image_url.as_deref(),
            ),
            images: item
                .images
                .iter()
                .map(|key| storage::public_url(base_url, key))
                .collect(),
            is_available: item.is_available,
        }
    }
}
