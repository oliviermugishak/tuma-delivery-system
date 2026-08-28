//! Stores and products — the commerce core (S9, multi-store since S10b).
//!
//! Ownership is resolved server-side: a store belongs to exactly one
//! merchant (`stores.merchant_id`), a merchant can have several stores,
//! and product access joins through the store. Clients never claim
//! ownership — the signed-in user id decides. Money is integer RWF
//! (`BIGINT`), enforced non-negative at the schema.

use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `tuma.stores`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Store {
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
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A row of `tuma.products`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Product {
    pub id: Uuid,
    pub store_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub is_available: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Full values for a store write. PATCH handlers merge the partial input
/// onto the current row and hand the domain the complete picture.
pub struct StoreChanges {
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub address_text: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub delivery_fee: i64,
    pub is_open: bool,
}

/// Fields for a new product, including which of the merchant's stores it
/// joins — the merchant picks the store, the server checks ownership.
pub struct NewProduct {
    pub store_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub is_available: bool,
}

/// Full values for a product write (same merge discipline as stores).
pub struct ProductChanges {
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub is_available: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ProductError {
    /// The `store_id` a product was created against is missing or belongs
    /// to another merchant — the same response either way, so merchants
    /// cannot probe for other stores.
    #[error("store not found")]
    StoreNotFound,
    /// Missing, or belongs to another merchant's store — the same response
    /// either way, so merchants cannot probe for other stores' products.
    #[error("product not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Create a store for the signed-in merchant. Merchants can have several
/// stores; new stores start closed — opening is a deliberate PATCH.
pub async fn create_store(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    changes: StoreChanges,
) -> Result<Store, StoreError> {
    let store = sqlx::query_as!(
        Store,
        r#"
        INSERT INTO tuma.stores (
            merchant_id, name, description, image_url, address_text,
            lat, lng, delivery_fee, is_open
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, merchant_id, name, description, image_url, address_text,
                  lat, lng, delivery_fee, is_open, created_at, updated_at
        "#,
        merchant_id,
        changes.name,
        changes.description,
        changes.image_url,
        changes.address_text,
        changes.lat,
        changes.lng,
        changes.delivery_fee,
        changes.is_open,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(store)
}

/// All of the signed-in merchant's stores, oldest first (empty until they
/// create one).
pub async fn stores_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, address_text,
               lat, lng, delivery_fee, is_open, created_at, updated_at
        FROM tuma.stores
        WHERE merchant_id = $1
        ORDER BY created_at
        "#,
        merchant_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One of the signed-in merchant's stores. Another merchant's store is
/// indistinguishable from a missing one.
pub async fn store_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_id: Uuid,
) -> Result<Store, StoreError> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, address_text,
               lat, lng, delivery_fee, is_open, created_at, updated_at
        FROM tuma.stores
        WHERE id = $1 AND merchant_id = $2
        "#,
        store_id,
        merchant_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StoreError::NotFound)
}

/// Overwrite one of the merchant's stores with full values. The
/// `updated_at` trigger only fires when the row actually changes.
pub async fn update_store(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_id: Uuid,
    changes: StoreChanges,
) -> Result<Store, StoreError> {
    sqlx::query_as!(
        Store,
        r#"
        UPDATE tuma.stores
        SET name = $3, description = $4, image_url = $5, address_text = $6,
            lat = $7, lng = $8, delivery_fee = $9, is_open = $10
        WHERE id = $1 AND merchant_id = $2
        RETURNING id, merchant_id, name, description, image_url, address_text,
                  lat, lng, delivery_fee, is_open, created_at, updated_at
        "#,
        store_id,
        merchant_id,
        changes.name,
        changes.description,
        changes.image_url,
        changes.address_text,
        changes.lat,
        changes.lng,
        changes.delivery_fee,
        changes.is_open,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StoreError::NotFound)
}

/// A store by id, regardless of owner or open state. Callers apply policy
/// (customers only ever see open stores).
pub async fn store_by_id(
    conn: &mut PgConnection,
    store_id: Uuid,
) -> Result<Option<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, address_text,
               lat, lng, delivery_fee, is_open, created_at, updated_at
        FROM tuma.stores
        WHERE id = $1
        "#,
        store_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Stores currently accepting orders, oldest first.
pub async fn open_stores(conn: &mut PgConnection) -> Result<Vec<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, address_text,
               lat, lng, delivery_fee, is_open, created_at, updated_at
        FROM tuma.stores
        WHERE is_open
        ORDER BY created_at
        "#,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Products of one store. `available_only` filters out off-menu items
/// (customer view); merchants pass `false` to see everything.
pub async fn products_for_store(
    conn: &mut PgConnection,
    store_id: Uuid,
    available_only: bool,
) -> Result<Vec<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT id, store_id, name, description, price, image_url,
               is_available, created_at, updated_at
        FROM tuma.products
        WHERE store_id = $1 AND (NOT $2 OR is_available)
        ORDER BY created_at
        "#,
        store_id,
        available_only,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Everything on the signed-in merchant's menu across all of their stores
/// (empty until a store has products), oldest first.
pub async fn products_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<Product>, sqlx::Error> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT p.id, p.store_id, p.name, p.description, p.price, p.image_url,
               p.is_available, p.created_at, p.updated_at
        FROM tuma.products p
        JOIN tuma.stores s ON s.id = p.store_id
        WHERE s.merchant_id = $1
        ORDER BY p.created_at
        "#,
        merchant_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One of a merchant's stores with how many products it has — the admin's
/// read-only view (no location or image until something displays them).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreSummary {
    pub id: Uuid,
    pub name: String,
    pub address_text: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    pub created_at: OffsetDateTime,
    pub product_count: i64,
}

/// A merchant's stores with product counts, oldest first (admin view).
pub async fn store_summaries_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<StoreSummary>, sqlx::Error> {
    sqlx::query_as!(
        StoreSummary,
        r#"
        SELECT s.id, s.name, s.address_text, s.delivery_fee, s.is_open,
               s.created_at, COUNT(p.id) as "product_count!"
        FROM tuma.stores s
        LEFT JOIN tuma.products p ON p.store_id = s.id
        WHERE s.merchant_id = $1
        GROUP BY s.id
        ORDER BY s.created_at
        "#,
        merchant_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Platform-wide catalog counts (admin summary).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CatalogCounts {
    pub stores: i64,
    pub open_stores: i64,
    pub products: i64,
}

pub async fn catalog_counts(conn: &mut PgConnection) -> Result<CatalogCounts, sqlx::Error> {
    sqlx::query_as!(
        CatalogCounts,
        r#"
        SELECT (SELECT COUNT(*) FROM tuma.stores) as "stores!",
               (SELECT COUNT(*) FROM tuma.stores WHERE is_open) as "open_stores!",
               (SELECT COUNT(*) FROM tuma.products) as "products!"
        "#,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Add a product to one of the signed-in merchant's stores. A `store_id`
/// that is missing or belongs to another merchant is
/// [`ProductError::StoreNotFound`].
pub async fn create_product(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    product: NewProduct,
) -> Result<Product, ProductError> {
    store_for_merchant(conn, merchant_id, product.store_id)
        .await
        .map_err(|error| match error {
            StoreError::NotFound => ProductError::StoreNotFound,
            StoreError::Database(error) => ProductError::Database(error),
        })?;

    let created = sqlx::query_as!(
        Product,
        r#"
        INSERT INTO tuma.products (store_id, name, description, price, image_url, is_available)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, store_id, name, description, price, image_url,
                  is_available, created_at, updated_at
        "#,
        product.store_id,
        product.name,
        product.description,
        product.price,
        product.image_url,
        product.is_available,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(created)
}

/// One of the signed-in merchant's products. Other merchants' products are
/// indistinguishable from missing ones (see [`ProductError::NotFound`]).
pub async fn product_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    product_id: Uuid,
) -> Result<Product, ProductError> {
    sqlx::query_as!(
        Product,
        r#"
        SELECT p.id, p.store_id, p.name, p.description, p.price, p.image_url,
               p.is_available, p.created_at, p.updated_at
        FROM tuma.products p
        JOIN tuma.stores s ON s.id = p.store_id
        WHERE p.id = $1 AND s.merchant_id = $2
        "#,
        product_id,
        merchant_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ProductError::NotFound)
}

/// Overwrite one of the merchant's products with full values.
pub async fn update_product(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    product_id: Uuid,
    changes: ProductChanges,
) -> Result<Product, ProductError> {
    sqlx::query_as!(
        Product,
        r#"
        UPDATE tuma.products p
        SET name = $3, description = $4, price = $5, image_url = $6, is_available = $7
        FROM tuma.stores s
        WHERE p.id = $1 AND s.id = p.store_id AND s.merchant_id = $2
        RETURNING p.id, p.store_id, p.name, p.description, p.price, p.image_url,
                  p.is_available, p.created_at, p.updated_at
        "#,
        product_id,
        merchant_id,
        changes.name,
        changes.description,
        changes.price,
        changes.image_url,
        changes.is_available,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(ProductError::NotFound)
}

/// Delete one of the merchant's stores. Its products follow via ON DELETE
/// CASCADE. Unknown ids and another merchant's store are the same NotFound.
pub async fn delete_store(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_id: Uuid,
) -> Result<(), StoreError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM tuma.stores
        WHERE id = $1 AND merchant_id = $2
        "#,
        store_id,
        merchant_id,
    )
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// Delete one of the merchant's products (ownership resolved through the
/// store). Unknown ids and another merchant's product are the same NotFound.
pub async fn delete_product(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM tuma.products p
        USING tuma.stores s
        WHERE p.id = $1 AND p.store_id = s.id AND s.merchant_id = $2
        "#,
        product_id,
        merchant_id,
    )
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(ProductError::NotFound);
    }
    Ok(())
}
