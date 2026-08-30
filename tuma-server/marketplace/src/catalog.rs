//! Catalog: the two-level product model (marketplace schema).
//!
//! `products` is the merchant's catalog — the identity of a thing they
//! sell, defined once per business. `store_products` is what a store
//! actually sells it for: per-store price, stock, availability, and SKU.
//! The customer buys a store_product; the order snapshot therefore cites
//! both ids. Ownership resolves through the store / merchant the same way
//! stores do — foreign resources look missing.

use sqlx::Connection;
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `marketplace.products` — the merchant-level catalog item.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Full values for a catalog product write (PATCH merge discipline as
/// everywhere else).
pub struct ProductChanges {
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProductError {
    /// Missing, or belongs to another merchant — the same response either
    /// way, so merchants cannot probe for other businesses' catalog.
    #[error("product not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Create a catalog product for the business. Attaching it to stores is a
/// separate decision — a catalog product can exist before any store sells
/// it.
pub async fn create_product(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    name: String,
    description: Option<String>,
    image_url: Option<String>,
) -> Result<Product, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        INSERT INTO marketplace.products (merchant_id, name, description, image_url)
        VALUES ($1, $2, $3, $4)
        RETURNING id, merchant_id, name, description, image_url, created_at, updated_at
        "#,
        merchant_id,
        name,
        description,
        image_url,
    )
    .fetch_one(&mut *conn)
    .await
}

/// The business's catalog, oldest first, with each product's gallery
/// cover resolved (the lowest-position gallery image; `cover_key` is the
/// object-storage key — the response layer composes the URL).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductWithCover {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub cover_key: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub async fn products_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<ProductWithCover>, sqlx::Error> {
    sqlx::query_as!(
        ProductWithCover,
        r#"
        SELECT p.id, p.merchant_id, p.name, p.description, p.image_url,
               (SELECT pi.storage_key FROM marketplace.product_images pi
                WHERE pi.product_id = p.id ORDER BY pi.position, pi.id LIMIT 1) AS cover_key,
               p.created_at, p.updated_at
        FROM marketplace.products p
        WHERE p.merchant_id = $1
        ORDER BY p.created_at
        "#,
        merchant_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One of the merchant's catalog products; foreign ids are NotFound.
pub async fn product_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    product_id: Uuid,
) -> Result<Product, ProductError> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, merchant_id, name, description, image_url, created_at, updated_at
        FROM marketplace.products
        WHERE id = $1 AND merchant_id = $2
        "#,
        product_id,
        merchant_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ProductError::NotFound)
}

/// Overwrite a catalog product with full values (ownership pre-checked).
pub async fn update_product(
    conn: &mut PgConnection,
    product_id: Uuid,
    changes: ProductChanges,
) -> Result<Product, ProductError> {
    sqlx::query_as!(
        Product,
        r#"
        UPDATE marketplace.products
        SET name = $2, description = $3, image_url = $4
        WHERE id = $1
        RETURNING id, merchant_id, name, description, image_url, created_at, updated_at
        "#,
        product_id,
        changes.name,
        changes.description,
        changes.image_url,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ProductError::NotFound)
}

/// Delete a catalog product. Every store_product selling it follows via
/// ON DELETE CASCADE — it leaves all assortments at once.
pub async fn delete_product(
    conn: &mut PgConnection,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM marketplace.products
        WHERE id = $1
        "#,
        product_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// A row of `marketplace.store_products` — the sellable item.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreProduct {
    pub id: Uuid,
    pub store_id: Uuid,
    pub product_id: Uuid,
    pub price: i64,
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A store_product joined with its catalog identity and its store — what
/// the merchant's assortment list renders.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreProductView {
    pub id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub product_id: Uuid,
    pub product_name: String,
    pub product_description: Option<String>,
    pub image_url: Option<String>,
    pub cover_key: Option<String>,
    pub price: i64,
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreProductError {
    /// Missing, or belongs to another merchant's store — the same response
    /// either way.
    #[error("store product not found")]
    NotFound,
    /// The catalog product is missing or belongs to another merchant.
    #[error("product not found")]
    ProductNotFound,
    /// This store already sells this catalog product — patch it instead.
    #[error("this store already sells this product")]
    AlreadyAttached,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Attach a catalog product to a store with its sell configuration. Both
/// the store and the product must belong to the same merchant — the domain
/// re-checks the product so handlers cannot be talked into cross-merchant
/// attachments.
pub struct NewStoreProduct {
    pub store_id: Uuid,
    pub product_id: Uuid,
    pub price: i64,
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
}

pub async fn create_store_product(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    new: NewStoreProduct,
) -> Result<StoreProduct, StoreProductError> {
    product_for_merchant(&mut *conn, merchant_id, new.product_id)
        .await
        .map_err(|error| match error {
            ProductError::NotFound => StoreProductError::ProductNotFound,
            ProductError::Database(error) => StoreProductError::Database(error),
        })?;

    let created = sqlx::query_as!(
        StoreProduct,
        r#"
        INSERT INTO marketplace.store_products
            (store_id, product_id, price, stock, is_available, sku)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, store_id, product_id, price, stock, is_available, sku,
                  created_at, updated_at
        "#,
        new.store_id,
        new.product_id,
        new.price,
        new.stock,
        new.is_available,
        new.sku,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db_error)
            if db_error.constraint() == Some("store_products_store_product_key") =>
        {
            StoreProductError::AlreadyAttached
        }
        other => StoreProductError::Database(other),
    })?;
    Ok(created)
}

/// The assortment across one business's stores, optionally scoped to a set
/// of stores (a store-scoped manager), oldest first.
pub async fn store_products_for_merchant_scoped(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_ids: Option<&[Uuid]>,
) -> Result<Vec<StoreProductView>, sqlx::Error> {
    sqlx::query_as!(
        StoreProductView,
        r#"
        SELECT sp.id, sp.store_id, s.name AS store_name, sp.product_id,
               p.name AS product_name, p.description AS product_description,
               p.image_url,
               (SELECT pi.storage_key FROM marketplace.product_images pi
                WHERE pi.product_id = sp.product_id ORDER BY pi.position, pi.id LIMIT 1) AS cover_key,
               sp.price, sp.stock, sp.is_available, sp.sku,
               sp.created_at
        FROM marketplace.store_products sp
        JOIN marketplace.products p ON p.id = sp.product_id
        JOIN marketplace.stores s ON s.id = sp.store_id
        WHERE s.merchant_id = $1
          AND ($2::uuid[] IS NULL OR cardinality($2::uuid[]) = 0 OR sp.store_id = ANY($2::uuid[]))
        ORDER BY sp.created_at
        "#,
        merchant_id,
        store_ids,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One store_product with the store's merchant attached, for ownership
/// resolution: the caller filters through its grant.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreProductWithMerchant {
    pub id: Uuid,
    pub store_id: Uuid,
    pub merchant_id: Uuid,
    pub product_id: Uuid,
    pub price: i64,
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
}

pub async fn store_product_by_id(
    conn: &mut PgConnection,
    store_product_id: Uuid,
) -> Result<Option<StoreProductWithMerchant>, sqlx::Error> {
    sqlx::query_as!(
        StoreProductWithMerchant,
        r#"
        SELECT sp.id, sp.store_id, s.merchant_id, sp.product_id,
               sp.price, sp.stock, sp.is_available, sp.sku
        FROM marketplace.store_products sp
        JOIN marketplace.stores s ON s.id = sp.store_id
        WHERE sp.id = $1
        "#,
        store_product_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Full values for a store_product write.
pub struct StoreProductChanges {
    pub price: i64,
    pub stock: Option<i64>,
    pub is_available: bool,
    pub sku: Option<String>,
}

/// Overwrite a store_product with full values (ownership pre-checked).
pub async fn update_store_product(
    conn: &mut PgConnection,
    store_product_id: Uuid,
    changes: StoreProductChanges,
) -> Result<StoreProduct, StoreProductError> {
    sqlx::query_as!(
        StoreProduct,
        r#"
        UPDATE marketplace.store_products
        SET price = $2, stock = $3, is_available = $4, sku = $5
        WHERE id = $1
        RETURNING id, store_id, product_id, price, stock, is_available, sku,
                  created_at, updated_at
        "#,
        store_product_id,
        changes.price,
        changes.stock,
        changes.is_available,
        changes.sku,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StoreProductError::NotFound)
}

/// Detach one product from one store. The catalog identity stays.
pub async fn delete_store_product(
    conn: &mut PgConnection,
    store_product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM marketplace.store_products
        WHERE id = $1
        "#,
        store_product_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// A store's available menu, oldest first — the customer view (available
/// items only; the open-store rule is enforced by the caller). Each item
/// carries its full gallery (`images`, cover first) so the client never
/// needs a second fetch to show a product.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MenuItem {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub cover_key: Option<String>,
    pub images: Vec<String>,
    pub is_available: bool,
}

pub async fn menu_for_store(
    conn: &mut PgConnection,
    store_id: Uuid,
) -> Result<Vec<MenuItem>, sqlx::Error> {
    sqlx::query_as!(
        MenuItem,
        r#"
        SELECT sp.id, p.name, p.description, sp.price, p.image_url,
               (SELECT pi.storage_key FROM marketplace.product_images pi
                WHERE pi.product_id = sp.product_id ORDER BY pi.position, pi.id LIMIT 1) AS cover_key,
               COALESCE(
                   (SELECT array_agg(pi.storage_key ORDER BY pi.position, pi.id)
                    FROM marketplace.product_images pi
                    WHERE pi.product_id = sp.product_id),
                   ARRAY[]::text[]
               ) AS "images!",
               sp.is_available
        FROM marketplace.store_products sp
        JOIN marketplace.products p ON p.id = sp.product_id
        WHERE sp.store_id = $1 AND sp.is_available
        ORDER BY sp.created_at
        "#,
        store_id,
    )
    .fetch_all(&mut *conn)
    .await
}

// ---------------------------------------------------------------------------
// Product images (slice U1) — the gallery. Bytes live in object storage;
// these rows hold storage keys + gallery order only. The lowest position
// is the cover; every customer view joins it by that rule.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductImage {
    pub id: Uuid,
    pub product_id: Uuid,
    pub storage_key: String,
    pub position: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum ProductImageError {
    /// Missing, or belongs to another merchant's product — the same
    /// response either way.
    #[error("product image not found")]
    NotFound,
    /// [`MAX_IMAGES_PER_PRODUCT`] reached — a genuine conflict with the
    /// product's current gallery state.
    #[error("this product's gallery is full — 8 images max")]
    GalleryFull,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Hard cap on one product's gallery — the menu response carries every
/// URL, so the cap is also a mobile-data budget.
pub const MAX_IMAGES_PER_PRODUCT: i64 = 8;

/// Append a gallery image at the next position, enforcing the gallery cap.
/// The product row is locked `FOR UPDATE` for the check-and-insert, so two
/// concurrent uploads cannot both slip past the cap. The caller has
/// already uploaded the bytes and established ownership.
pub async fn insert_product_image(
    conn: &mut PgConnection,
    product_id: Uuid,
    storage_key: &str,
) -> Result<ProductImage, ProductImageError> {
    let mut tx = conn.begin().await?;
    sqlx::query!(
        r#"
        SELECT id FROM marketplace.products WHERE id = $1 FOR UPDATE
        "#,
        product_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ProductImageError::NotFound)?;
    let count = sqlx::query!(
        r#"
        SELECT COUNT(*) AS "count!: i64"
        FROM marketplace.product_images
        WHERE product_id = $1
        "#,
        product_id,
    )
    .fetch_one(&mut *tx)
    .await?
    .count;
    if count >= MAX_IMAGES_PER_PRODUCT {
        return Err(ProductImageError::GalleryFull);
    }
    let position = sqlx::query!(
        r#"
        SELECT COALESCE(MAX(position), -1) AS "max!: i32"
        FROM marketplace.product_images
        WHERE product_id = $1
        "#,
        product_id,
    )
    .fetch_one(&mut *tx)
    .await?
    .max + 1;
    let image = sqlx::query_as!(
        ProductImage,
        r#"
        INSERT INTO marketplace.product_images (product_id, storage_key, position)
        VALUES ($1, $2, $3)
        RETURNING id, product_id, storage_key, position, created_at, updated_at
        "#,
        product_id,
        storage_key,
        position,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(image)
}

/// One product's gallery, cover first.
pub async fn images_for_product(
    conn: &mut PgConnection,
    product_id: Uuid,
) -> Result<Vec<ProductImage>, sqlx::Error> {
    sqlx::query_as!(
        ProductImage,
        r#"
        SELECT id, product_id, storage_key, position, created_at, updated_at
        FROM marketplace.product_images
        WHERE product_id = $1
        ORDER BY position, id
        "#,
        product_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One gallery image with its product's merchant attached, for ownership
/// resolution: the caller filters through its grant.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductImageWithMerchant {
    pub id: Uuid,
    pub product_id: Uuid,
    pub merchant_id: Uuid,
    pub storage_key: String,
    pub position: i32,
}

pub async fn product_image_by_id(
    conn: &mut PgConnection,
    image_id: Uuid,
) -> Result<Option<ProductImageWithMerchant>, sqlx::Error> {
    sqlx::query_as!(
        ProductImageWithMerchant,
        r#"
        SELECT pi.id, pi.product_id, p.merchant_id, pi.storage_key, pi.position
        FROM marketplace.product_images pi
        JOIN marketplace.products p ON p.id = pi.product_id
        WHERE pi.id = $1
        "#,
        image_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Delete a gallery image. The object in storage is cleaned up by the
/// caller (best-effort) after the row is gone.
pub async fn delete_product_image(
    conn: &mut PgConnection,
    image_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM marketplace.product_images
        WHERE id = $1
        "#,
        image_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Make one image the cover: swap positions with the current cover in one
/// transaction. A no-op when the image already is the cover. Positions are
/// deliberately unconstrained so the two-row swap cannot trip a unique
/// index mid-flight.
pub async fn set_product_cover(
    conn: &mut PgConnection,
    image_id: Uuid,
) -> Result<(), ProductImageError> {
    let mut tx = conn.begin().await?;
    let target = sqlx::query!(
        r#"
        SELECT id, product_id, position
        FROM marketplace.product_images
        WHERE id = $1
        "#,
        image_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ProductImageError::NotFound)?;
    let cover = sqlx::query!(
        r#"
        SELECT id, position
        FROM marketplace.product_images
        WHERE product_id = $1
        ORDER BY position, id
        LIMIT 1
        "#,
        target.product_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(cover) = cover.filter(|cover| cover.id != target.id) {
        sqlx::query!(
            r#"
            UPDATE marketplace.product_images SET position = $2 WHERE id = $1
            "#,
            target.id,
            cover.position,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            r#"
            UPDATE marketplace.product_images SET position = $2 WHERE id = $1
            "#,
            cover.id,
            target.position,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
