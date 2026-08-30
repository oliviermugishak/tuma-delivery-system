//! Image uploads for merchant operators (slice U1): store banners and
//! product-image galleries. Bytes arrive as multipart (proxied through
//! the API — one path for every backend), are validated by content and
//! normalized once, then written to the configured object store; the DB
//! holds keys and gallery order only.
//!
//! Authorization follows each area's precedent: banners are store
//! territory (any operator who can reach the store), the catalog gallery
//! is owner territory. Foreign resources look missing. Orphaned objects
//! are cleaned up best-effort — never on the response path.

use crate::app::{AppError, AppResult, AppState, UserContext};
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use marketplace::catalog;
use marketplace::stores;
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

fn merchant_access(context: &UserContext) -> AppResult<crate::app::MerchantAccess> {
    context
        .merchant_access()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

/// The multipart schema in the OpenAPI contract — hey-api generates a
/// FormData request type from it.
#[derive(utoipa::ToSchema)]
pub struct ImageUpload {
    /// The image file (JPEG, PNG, or WebP; at most 5 MB).
    #[schema(format = Binary, content_encoding = "binary")]
    pub file: String,
}

/// One gallery image as the API returns it.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProductImageResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    /// The URL the object is served from (composed against the
    /// environment's public storage base).
    pub image_url: String,
    pub position: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl ProductImageResponse {
    pub fn from_domain(image: catalog::ProductImage, base_url: &str) -> Self {
        Self {
            id: image.id,
            product_id: image.product_id,
            image_url: storage::public_url(base_url, &image.storage_key),
            position: image.position,
            created_at: image.created_at,
        }
    }
}

/// Pull the `file` part out of a multipart body. The bytes are NOT
/// trusted: `normalize_image` sniffs the real content and enforces the
/// size cap (413) and format allow-list (415).
async fn read_image_part(multipart: &mut Multipart) -> AppResult<axum::body::Bytes> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("invalid multipart body: {error}")))?
    {
        if field.name() == Some("file") {
            return field.bytes().await.map_err(|error| {
                AppError::BadRequest(format!("could not read the file: {error}"))
            });
        }
    }
    Err(AppError::BadRequest(
        "the multipart body has no 'file' part".into(),
    ))
}

// ---------------------------------------------------------------------------
// Store banner
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/v1/merchant/stores/{id}/banner",
    params(("id" = Uuid, Path, description = "Store id")),
    request_body(content = ImageUpload, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Banner stored; the store's image_url now serves it", body = crate::routes::stores::StoreResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's stores"),
        (status = 413, description = "The image exceeds 5 MB"),
        (status = 415, description = "Not a JPEG, PNG, or WebP image"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Upload store banner", skip_all)]
pub async fn upload_store_banner(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(store_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<crate::routes::stores::StoreResponse>)> {
    let access = merchant_access(&context)?;
    let grant = access.single_grant()?;
    let bytes = read_image_part(&mut multipart).await?;
    let image = storage::normalize_image(bytes)?;

    let mut conn = app.db_pool.acquire().await?;
    // Store territory: anyone who can reach the store can dress it.
    let store = stores::store_by_id(&mut conn, store_id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or_else(|| AppError::NotFound("store not found".into()))?;

    let key = storage::banner_key(grant.merchant_id, store.id);
    storage::put_image(&app.storage.store, &key, image).await?;
    let updated = stores::set_banner(&mut conn, store.id, Some(&key)).await?;

    // The replaced banner is retired off the response path — its key is
    // gone from the DB, so it can never be served again anyway.
    if let Some(old_key) = store.banner_key {
        storage::delete_best_effort(&app.storage.store, &old_key).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(crate::routes::stores::StoreResponse::from_store(
            updated,
            &app.storage.public_base_url,
        )),
    ))
}

#[utoipa::path(
    delete,
    path = "/v1/merchant/stores/{id}/banner",
    params(("id" = Uuid, Path, description = "Store id")),
    responses(
        (status = 204, description = "Banner cleared"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's stores"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete store banner", skip_all)]
pub async fn delete_store_banner(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(store_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let access = merchant_access(&context)?;
    access.single_grant()?;

    let mut conn = app.db_pool.acquire().await?;
    let store = stores::store_by_id(&mut conn, store_id)
        .await?
        .filter(|store| access.can_access_store(store.merchant_id, store.id))
        .ok_or_else(|| AppError::NotFound("store not found".into()))?;

    stores::set_banner(&mut conn, store.id, None).await?;
    if let Some(old_key) = store.banner_key {
        storage::delete_best_effort(&app.storage.store, &old_key).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Product gallery
// ---------------------------------------------------------------------------

/// The catalog is owner territory (catalog precedent) — resolved once per
/// handler, before any I/O.
fn owner_grant(context: &UserContext) -> AppResult<crate::app::MerchantGrant> {
    let access = merchant_access(context)?;
    let grant = access.single_grant()?;
    if !grant.owner {
        return Err(AppError::Forbidden(
            "only the business owner can manage the catalog".into(),
        ));
    }
    Ok(grant.clone())
}

#[utoipa::path(
    get,
    path = "/v1/merchant/products/{id}/images",
    params(("id" = Uuid, Path, description = "Catalog product id")),
    responses(
        (status = 200, description = "The gallery, cover first", body = Vec<ProductImageResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this owner's products"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List product images", skip_all)]
pub async fn list_product_images(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(product_id): Path<Uuid>,
) -> AppResult<Json<Vec<ProductImageResponse>>> {
    let grant = owner_grant(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let product = catalog::product_for_merchant(&mut conn, grant.merchant_id, product_id).await?;
    let images = catalog::images_for_product(&mut conn, product.id).await?;
    let base_url = app.storage.public_base_url.clone();
    Ok(Json(
        images
            .into_iter()
            .map(|image| ProductImageResponse::from_domain(image, &base_url))
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/merchant/products/{id}/images",
    params(("id" = Uuid, Path, description = "Catalog product id")),
    request_body(content = ImageUpload, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Image appended to the gallery", body = ProductImageResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Only the business owner manages the catalog"),
        (status = 404, description = "Not one of this owner's products"),
        (status = 413, description = "The image exceeds 5 MB"),
        (status = 415, description = "Not a JPEG, PNG, or WebP image"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Upload product image", skip_all)]
pub async fn upload_product_image(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(product_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<ProductImageResponse>)> {
    let grant = owner_grant(&context)?;
    let bytes = read_image_part(&mut multipart).await?;
    let image = storage::normalize_image(bytes)?;

    let mut conn = app.db_pool.acquire().await?;
    let product = catalog::product_for_merchant(&mut conn, grant.merchant_id, product_id).await?;

    let key = storage::product_image_key(grant.merchant_id, product.id);
    storage::put_image(&app.storage.store, &key, image).await?;
    // The domain enforces the gallery cap (409 when full) and picks the
    // next position under a product-row lock.
    let row = catalog::insert_product_image(&mut conn, product.id, &key).await?;

    Ok((
        StatusCode::CREATED,
        Json(ProductImageResponse::from_domain(
            row,
            &app.storage.public_base_url,
        )),
    ))
}

/// Resolve nothing here — each handler fetches its own image through
/// `product_for_merchant` (ownership) + `product_image_by_id` filtered on
/// the product id, so a foreign product, a foreign image, or a mismatched
/// pair is the same 404.

#[utoipa::path(
    delete,
    path = "/v1/merchant/products/{product_id}/images/{image_id}",
    params(
        ("product_id" = Uuid, Path, description = "Catalog product id"),
        ("image_id" = Uuid, Path, description = "Gallery image id"),
    ),
    responses(
        (status = 204, description = "Image removed"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Only the business owner manages the catalog"),
        (status = 404, description = "Not one of this owner's images"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Delete product image", skip_all)]
pub async fn delete_product_image(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path((product_id, image_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let grant = owner_grant(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let product = catalog::product_for_merchant(&mut conn, grant.merchant_id, product_id).await?;
    let image = catalog::product_image_by_id(&mut conn, image_id)
        .await?
        .filter(|image| image.product_id == product.id)
        .ok_or_else(|| AppError::NotFound("product image not found".into()))?;

    catalog::delete_product_image(&mut conn, image.id).await?;
    storage::delete_best_effort(&app.storage.store, &image.storage_key).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/v1/merchant/products/{product_id}/images/{image_id}/cover",
    params(
        ("product_id" = Uuid, Path, description = "Catalog product id"),
        ("image_id" = Uuid, Path, description = "Gallery image id"),
    ),
    responses(
        (status = 204, description = "The image is now the cover"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Only the business owner manages the catalog"),
        (status = 404, description = "Not one of this owner's images"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Set product cover", skip_all)]
pub async fn set_product_cover(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path((product_id, image_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let grant = owner_grant(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let product = catalog::product_for_merchant(&mut conn, grant.merchant_id, product_id).await?;
    let image = catalog::product_image_by_id(&mut conn, image_id)
        .await?
        .filter(|image| image.product_id == product.id)
        .ok_or_else(|| AppError::NotFound("product image not found".into()))?;

    catalog::set_product_cover(&mut conn, image.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
