//! Catalog management for merchant operators: the business-level product
//! catalog plus per-store assortments (store_products). Ownership follows
//! the memberships — the catalog is owner territory; assortments are
//! manageable by anyone who can reach the store.

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use crate::routes::orders::PageQuery;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use marketplace::catalog::{self, ProductChanges, StoreProductChanges, StoreProductError};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// A catalog product as the API returns it.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProductResponse {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<catalog::Product> for ProductResponse {
    fn from(product: catalog::Product) -> Self {
        Self {
            id: product.id,
            merchant_id: product.merchant_id,
            name: product.name,
            description: product.description,
            image_url: product.image_url,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

impl ProductResponse {
    /// The list view: the gallery cover wins; the product's legacy
    /// external URL falls through when nothing is uploaded.
    pub fn from_with_cover(product: catalog::ProductWithCover, base_url: &str) -> Self {
        Self {
            id: product.id,
            merchant_id: product.merchant_id,
            name: product.name,
            description: product.description,
            image_url: storage::resolve_image_url(
                base_url,
                product.cover_key.as_deref(),
                product.image_url.as_deref(),
            ),
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

/// A store_product as the merchant sees it: its catalog identity, its
/// store, and its sell configuration.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreProductResponse {
    pub id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub price: i64,
    /// `null` = untracked (made to order). A number is reserved atomically
    /// at checkout.
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl StoreProductResponse {
    /// Gallery cover wins; the product's legacy external URL falls through.
    pub fn from_view(view: catalog::StoreProductView, base_url: &str) -> Self {
        Self {
            id: view.id,
            store_id: view.store_id,
            store_name: view.store_name,
            product_id: view.product_id,
            product_name: view.product_name,
            description: view.product_description,
            image_url: storage::resolve_image_url(
                base_url,
                view.cover_key.as_deref(),
                view.image_url.as_deref(),
            ),
            price: view.price,
            stock: view.stock,
            is_available: view.is_available,
            sku: view.sku,
            created_at: view.created_at,
        }
    }
}

fn merchant_access(context: &UserContext) -> AppResult<crate::app::MerchantAccess> {
    context
        .merchant_access()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
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

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateProductInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: String,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateProductInput {
    #[validate(length(min = 1, max = 100, message = "name must be 1-100 characters"))]
    pub name: Option<String>,
    #[validate(length(max = 1000, message = "description must be at most 1000 characters"))]
    pub description: Option<String>,
    #[validate(length(max = 500, message = "image_url must be at most 500 characters"))]
    pub image_url: Option<String>,
}

#[utoipa::path(
    post,
    path = "/v1/merchant/products",
    request_body = CreateProductInput,
    responses(
        (status = 201, description = "Catalog product created", body = ProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or not the business owner"),
        (status = 400, description = "Account belongs to several businesses"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Create catalog product", skip_all)]
pub async fn create_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateProductInput>,
) -> AppResult<(StatusCode, Json<ProductResponse>)> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    // The catalog is business-level: owners only, one business per operator.
    let access = merchant_access(&context)?;
    let grant = access.single_grant()?;
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can manage the catalog".into(),
        ));
    }

    let mut conn = app.db_pool.acquire().await?;
    let product = catalog::create_product(
        &mut conn,
        grant.merchant_id,
        name,
        merge_text(input.description, None),
        merge_text(input.image_url, None),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ProductResponse::from(product))))
}

#[utoipa::path(
    get,
    path = "/v1/merchant/products",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "The business's catalog, oldest first, one page", body = Vec<ProductResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List catalog products", skip_all)]
pub async fn list_products(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<ProductResponse>>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    // Catalog is per business; aggregate across the operator's businesses.
    let mut products = Vec::new();
    for grant in &access.grants {
        products.extend(
            catalog::products_for_merchant(
                &mut conn,
                grant.merchant_id,
                page.limit(),
                page.offset(),
            )
            .await?,
        );
    }
    Ok(Json(
        products
            .into_iter()
            .map(|p| ProductResponse::from_with_cover(p, &app.storage.public_base_url))
            .collect(),
    ))
}

#[utoipa::path(
    patch,
    path = "/v1/merchant/products/{id}",
    params(("id" = Uuid, Path, description = "Catalog product id")),
    request_body = UpdateProductInput,
    responses(
        (status = 200, description = "The updated catalog product", body = ProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or not the business owner"),
        (status = 404, description = "Not one of this business's products"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Update catalog product", skip_all)]
pub async fn update_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateProductInput>,
) -> AppResult<Json<ProductResponse>> {
    let access = merchant_access(&context)?;
    let grant = access.single_grant()?;
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can manage the catalog".into(),
        ));
    }

    let mut conn = app.db_pool.acquire().await?;
    let current = catalog::product_for_merchant(&mut conn, grant.merchant_id, id).await?;
    let changes = ProductChanges {
        name: merge_text(input.name, Some(current.name.clone()))
            .expect("current name is never empty"),
        description: merge_text(input.description, current.description),
        image_url: merge_text(input.image_url, current.image_url),
    };
    let product = catalog::update_product(&mut conn, id, changes).await?;
    Ok(Json(ProductResponse::from(product)))
}

/// Delete a catalog product. Every store selling it loses it at once
/// (ON DELETE CASCADE) — permanent, and the platform confirms first.
#[utoipa::path(
    delete,
    path = "/v1/merchant/products/{id}",
    params(("id" = Uuid, Path, description = "Catalog product id")),
    responses(
        (status = 204, description = "Catalog product deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or not the business owner"),
        (status = 404, description = "Not one of this business's products"),
        (status = 409, description = "The product has order history — mark it unavailable instead"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete catalog product", skip_all)]
pub async fn delete_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let access = merchant_access(&context)?;
    let grant = access.single_grant()?;
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can manage the catalog".into(),
        ));
    }

    let mut conn = app.db_pool.acquire().await?;
    // Ownership check first: a foreign id must be a 404, not a silent no-op.
    catalog::product_for_merchant(&mut conn, grant.merchant_id, id).await?;
    catalog::delete_product(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateStoreProductInput {
    /// Which catalog product this is.
    pub product_id: Uuid,
    /// Which of the operator's stores sells it.
    pub store_id: Uuid,
    /// Integer RWF, the store's own price for this product.
    #[validate(range(min = 0, message = "price must be 0 or more"))]
    pub price: i64,
    /// `null` (or absent) = untracked. A number is the on-hand count the
    /// checkout reserves.
    #[validate(range(min = 0, message = "stock must be 0 or more"))]
    pub stock: Option<i64>,
    /// Defaults to true.
    pub is_available: Option<bool>,
    #[validate(length(max = 64, message = "sku must be at most 64 characters"))]
    pub sku: Option<String>,
}

#[utoipa::path(
    post,
    path = "/v1/merchant/store-products",
    request_body = CreateStoreProductInput,
    responses(
        (status = 201, description = "Product attached to the store's assortment", body = StoreProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or store out of scope"),
        (status = 404, description = "Unknown store or product (or another business's)"),
        (status = 409, description = "This store already sells this product"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Attach product to store", skip_all)]
pub async fn create_store_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateStoreProductInput>,
) -> AppResult<(StatusCode, Json<StoreProductResponse>)> {
    let access = merchant_access(&context)?;
    // The store must be one this operator can reach; a store-scoped manager
    // attaches to their own store.
    let mut conn = app.db_pool.acquire().await?;
    let store = marketplace::stores::store_by_id(&mut conn, input.store_id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or_else(|| AppError::NotFound("store not found".into()))?;

    let created = catalog::create_store_product(
        &mut conn,
        store.merchant_id,
        catalog::NewStoreProduct {
            store_id: store.id,
            product_id: input.product_id,
            price: input.price,
            stock: input.stock,
            is_available: input.is_available.unwrap_or(true),
            sku: merge_text(input.sku, None),
        },
    )
    .await?;

    let view = catalog::store_products_for_merchant_scoped(
        &mut conn,
        store.merchant_id,
        Some(&[store.id]),
        // Internal row reload, not a page: a wide enough window to find
        // the row just created.
        200,
        0,
    )
    .await?
    .into_iter()
    .find(|view| view.id == created.id)
    .ok_or_else(|| AppError::Internal("attached product disappeared".into()))?;
    Ok((
        StatusCode::CREATED,
        Json(StoreProductResponse::from_view(
            view,
            &app.storage.public_base_url,
        )),
    ))
}

#[utoipa::path(
    get,
    path = "/v1/merchant/store-products",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "The assortment across reachable stores, oldest first, one page", body = Vec<StoreProductResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List store products", skip_all)]
pub async fn list_store_products(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<StoreProductResponse>>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let mut items = Vec::new();
    for grant in &access.grants {
        let scoped = grant.store_ids.as_deref();
        items.extend(
            catalog::store_products_for_merchant_scoped(
                &mut conn,
                grant.merchant_id,
                scoped,
                page.limit(),
                page.offset(),
            )
            .await?,
        );
    }
    Ok(Json(
        items
            .into_iter()
            .map(|v| StoreProductResponse::from_view(v, &app.storage.public_base_url))
            .collect(),
    ))
}

/// Distinguish the three stock cases on PATCH: absent (keep), `null`
/// (switch back to untracked), and a number (set). Plain serde collapses
/// absent and `null`, so stock is deserialized by hand.
fn deserialize_stock<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StockVisitor;
    impl<'de> serde::de::Visitor<'de> for StockVisitor {
        type Value = Option<Option<i64>>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a non-negative integer or null")
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Some(None)) // explicit null — switch back to untracked
        }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(Some(None))
        }
        fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            Ok(Some(Some(i64::deserialize(d)?))) // a number — track it
        }
    }
    deserializer.deserialize_option(StockVisitor)
}

/// PATCH on the sell configuration: price, stock (a number sets it, explicit
/// `null` switches back to untracked), availability, SKU. Absent fields
/// keep their value.
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateStoreProductInput {
    #[validate(range(min = 0, message = "price must be 0 or more"))]
    pub price: Option<i64>,
    /// Absent = keep, `null` = untracked, a number = the on-hand count.
    #[serde(default, deserialize_with = "deserialize_stock")]
    pub stock: Option<Option<i64>>,
    pub is_available: Option<bool>,
    #[validate(length(max = 64, message = "sku must be at most 64 characters"))]
    pub sku: Option<String>,
}

#[utoipa::path(
    patch,
    path = "/v1/merchant/store-products/{id}",
    params(("id" = Uuid, Path, description = "Store product id")),
    request_body = UpdateStoreProductInput,
    responses(
        (status = 200, description = "The updated store product", body = StoreProductResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or store out of scope"),
        (status = 404, description = "Not one of this business's store products"),
        (status = 409, description = "Changed by someone else since the read — reload and retry"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Update store product", skip_all)]
pub async fn update_store_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateStoreProductInput>,
) -> AppResult<Json<StoreProductResponse>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let current = catalog::store_product_by_id(&mut conn, id)
        .await?
        .filter(|item| access.can_access_store(item.merchant_id, item.store_id))
        .ok_or(AppError::NotFound(StoreProductError::NotFound.to_string()))?;

    if let Some(Some(stock)) = input.stock
        && stock < 0
    {
        return Err(AppError::BadRequest("stock must be 0 or more".into()));
    }
    let changes = StoreProductChanges {
        price: input.price.unwrap_or(current.price),
        stock: match input.stock {
            Some(stock) => stock,  // a number sets it, null clears it
            None => current.stock, // absent keeps
        },
        is_available: input.is_available.unwrap_or(current.is_available),
        sku: merge_text(input.sku, current.sku),
    };
    // The row we just merged against is the precondition: if a second
    // writer moved it in between, the domain refuses — no silent
    // last-write-wins. (0 rows on a row we just read is that race, so
    // NotFound here would lie; map it to the conflict.)
    catalog::update_store_product(&mut conn, id, current.updated_at, changes)
        .await
        .map_err(|error| match error {
            StoreProductError::NotFound => AppError::Conflict(StoreProductError::Stale.to_string()),
            other => other.into(),
        })?;

    let view = catalog::store_products_for_merchant_scoped(
        &mut conn,
        current.merchant_id,
        Some(&[current.store_id]),
        // Internal row reload, not a page: a wide enough window to find
        // the row just updated.
        200,
        0,
    )
    .await?
    .into_iter()
    .find(|view| view.id == id)
    .ok_or_else(|| AppError::Internal("updated product disappeared".into()))?;
    Ok(Json(StoreProductResponse::from_view(
        view,
        &app.storage.public_base_url,
    )))
}

/// Detach one product from one store. The catalog identity stays — the
/// product can be attached (differently priced) somewhere else.
#[utoipa::path(
    delete,
    path = "/v1/merchant/store-products/{id}",
    params(("id" = Uuid, Path, description = "Store product id")),
    responses(
        (status = 204, description = "Product detached from the store"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership, or store out of scope"),
        (status = 404, description = "Not one of this business's store products"),
        (status = 409, description = "The store product has order history — mark it unavailable instead"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete store product", skip_all)]
pub async fn delete_store_product(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    catalog::store_product_by_id(&mut conn, id)
        .await?
        .filter(|item| access.can_access_store(item.merchant_id, item.store_id))
        .ok_or(AppError::NotFound(StoreProductError::NotFound.to_string()))?;

    catalog::delete_store_product(&mut conn, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
