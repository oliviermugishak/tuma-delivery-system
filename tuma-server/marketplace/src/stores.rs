//! Stores — the fulfillment boundary (marketplace schema).
//!
//! A store belongs to exactly one merchant business; a merchant can have
//! several. Ownership is resolved server-side from the account's merchant
//! memberships: clients never claim ownership — the signed-in account's
//! scopes decide, and a foreign resource is indistinguishable from a
//! missing one. Money is integer RWF (`BIGINT`), enforced non-negative at
//! the schema.

use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `marketplace.stores`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Store {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    /// Object-storage key of the uploaded banner (slice U1). The public
    /// URL is composed at response time — stored URLs would rot when the
    /// base changes (e.g. a dev machine's LAN IP).
    pub banner_key: Option<String>,
    pub address_text: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub category: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    /// The store's contact surface — the customer's Get-help and
    /// store-info sheets call and email the store directly.
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
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
    pub category: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store not found")]
    NotFound,
    /// The row's updated_at moved between the caller's read and this
    /// write — a second writer won the race. The PATCH is refused instead
    /// of silently overwriting their change.
    #[error("this store was changed by someone else — reload and retry")]
    Stale,
    /// The store appears in commerce.store_orders — order history is
    /// immutable, so the delete is refused with its remedy named.
    #[error("this store has order history — close it instead of deleting")]
    HasOrderHistory,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Create a store for a merchant business. Merchants can have several
/// stores; new stores start closed — opening is a deliberate PATCH.
pub async fn create_store(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    changes: StoreChanges,
) -> Result<Store, StoreError> {
    let store = sqlx::query_as!(
        Store,
        r#"
        INSERT INTO marketplace.stores (
            merchant_id, name, description, image_url, address_text,
            lat, lng, category, delivery_fee, is_open,
            contact_phone, contact_email
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id, merchant_id, name, description, image_url, banner_key, address_text,
                  lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
                  created_at, updated_at
        "#,
        merchant_id,
        changes.name,
        changes.description,
        changes.image_url,
        changes.address_text,
        changes.lat,
        changes.lng,
        changes.category,
        changes.delivery_fee,
        changes.is_open,
        changes.contact_phone,
        changes.contact_email,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(store)
}

/// All stores of one merchant business, oldest first (empty until they
/// create one). Optionally scoped to a set of stores (a store-scoped
/// manager) — `None` means every store of the merchant.
pub async fn stores_for_merchant_scoped(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_ids: Option<&[Uuid]>,
) -> Result<Vec<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, banner_key, address_text,
               lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
               created_at, updated_at
        FROM marketplace.stores
        WHERE merchant_id = $1
          AND ($2::uuid[] IS NULL OR cardinality($2::uuid[]) = 0 OR id = ANY($2::uuid[]))
        ORDER BY created_at
        "#,
        merchant_id,
        store_ids,
    )
    .fetch_all(&mut *conn)
    .await
}

/// The slice of authorization this domain needs: which merchant's rows,
/// and — for a store-scoped manager — which stores. The API layer builds
/// it from its richer membership grant; the domain never depends on HTTP
/// or auth-middleware types.
#[derive(Debug, Clone)]
pub struct StoreScope {
    pub merchant_id: Uuid,
    /// `None` = every store of the merchant; `Some` = only these.
    pub store_ids: Option<Vec<Uuid>>,
}

impl StoreScope {
    pub fn allows(&self, store_id: Uuid) -> bool {
        self.store_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&store_id))
    }
}

/// All stores across the account's accessible merchants with per-merchant
/// scoping applied — the multi-membership case. One scoped query per scope;
/// memberships are few by construction.
pub async fn stores_for_scopes(
    conn: &mut PgConnection,
    scopes: &[StoreScope],
) -> Result<Vec<Store>, sqlx::Error> {
    let mut stores = Vec::new();
    for scope in scopes {
        let scoped = scope.store_ids.as_deref();
        stores.extend(stores_for_merchant_scoped(&mut *conn, scope.merchant_id, scoped).await?);
    }
    Ok(stores)
}

/// One store of one merchant business, scoped to the scope. Another
/// merchant's store — or an out-of-scope store — is indistinguishable from
/// a missing one.
pub async fn store_for_scope(
    conn: &mut PgConnection,
    scope: &StoreScope,
    store_id: Uuid,
) -> Result<Store, StoreError> {
    let store = sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, banner_key, address_text,
               lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
               created_at, updated_at
        FROM marketplace.stores
        WHERE id = $1 AND merchant_id = $2
        "#,
        store_id,
        scope.merchant_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .filter(|store| scope.allows(store.id))
    .ok_or(StoreError::NotFound)?;
    Ok(store)
}

/// Overwrite a store with full values. The caller has already established
/// access and merged the PATCH onto the current row. `expected_updated_at`
/// is the optimistic-concurrency precondition: the caller read it from the
/// row it merged against, and 0 rows here means a second writer moved the
/// row first — refused instead of last-write-wins. The `updated_at`
/// trigger only fires when the row actually changes.
pub async fn update_store(
    conn: &mut PgConnection,
    store_id: Uuid,
    expected_updated_at: OffsetDateTime,
    changes: StoreChanges,
) -> Result<Store, StoreError> {
    sqlx::query_as!(
        Store,
        r#"
        UPDATE marketplace.stores
        SET name = $2, description = $3, image_url = $4, address_text = $5,
            lat = $6, lng = $7, category = $8, delivery_fee = $9, is_open = $10,
            contact_phone = $11, contact_email = $12
        WHERE id = $1 AND updated_at = $13
        RETURNING id, merchant_id, name, description, image_url, banner_key, address_text,
                  lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
                  created_at, updated_at
        "#,
        store_id,
        changes.name,
        changes.description,
        changes.image_url,
        changes.address_text,
        changes.lat,
        changes.lng,
        changes.category,
        changes.delivery_fee,
        changes.is_open,
        changes.contact_phone,
        changes.contact_email,
        expected_updated_at,
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
        SELECT id, merchant_id, name, description, image_url, banner_key, address_text,
               lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
               created_at, updated_at
        FROM marketplace.stores
        WHERE id = $1
        "#,
        store_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Hard cap on the customer-facing store GETs — the hottest reads in the
/// app must never return an unbounded page. Matches the search side's
/// product cap style.
pub const STORE_LIMIT: i64 = 50;

/// Stores currently accepting orders, oldest first, capped at
/// [`STORE_LIMIT`].
pub async fn open_stores(conn: &mut PgConnection) -> Result<Vec<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, banner_key, address_text,
               lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
               created_at, updated_at
        FROM marketplace.stores
        WHERE is_open
        ORDER BY created_at
        LIMIT $1
        "#,
        STORE_LIMIT,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Escapes the LIKE wildcards so a customer searching "100%" finds
/// "100% Juices" rather than everything. The pattern is still wrapped
/// in %...% by the caller for substring matching.
pub fn escape_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Open stores whose name or category matches the search pattern, oldest
/// first, capped at [`STORE_LIMIT`]. A separate query from [open_stores]
/// on purpose: the no-search path stays byte-for-byte identical, and this
/// one stays free of null-branches that would keep the trigram index from
/// being used.
pub async fn search_stores(
    conn: &mut PgConnection,
    pattern: &str,
) -> Result<Vec<Store>, sqlx::Error> {
    sqlx::query_as!(
        Store,
        r#"
        SELECT id, merchant_id, name, description, image_url, banner_key, address_text,
               lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
               created_at, updated_at
        FROM marketplace.stores
        WHERE is_open AND (name ILIKE $1 OR category ILIKE $1)
        ORDER BY created_at
        LIMIT $2
        "#,
        pattern,
        STORE_LIMIT,
    )
    .fetch_all(&mut *conn)
    .await
}

/// A merchant's stores with assortment counts, oldest first (admin view).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreSummary {
    pub id: Uuid,
    pub name: String,
    pub address_text: Option<String>,
    pub category: Option<String>,
    pub delivery_fee: i64,
    pub is_open: bool,
    pub created_at: OffsetDateTime,
    pub product_count: i64,
}

pub async fn store_summaries_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<StoreSummary>, sqlx::Error> {
    sqlx::query_as!(
        StoreSummary,
        r#"
        SELECT s.id, s.name, s.address_text, s.category, s.delivery_fee, s.is_open,
               s.created_at, COUNT(sp.id) as "product_count!"
        FROM marketplace.stores s
        LEFT JOIN marketplace.store_products sp ON sp.store_id = s.id
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
    /// Merchant-level catalog products.
    pub products: i64,
    /// Store-level sellable items.
    pub store_products: i64,
}

pub async fn catalog_counts(conn: &mut PgConnection) -> Result<CatalogCounts, sqlx::Error> {
    sqlx::query_as!(
        CatalogCounts,
        r#"
        SELECT (SELECT COUNT(*) FROM marketplace.stores) as "stores!",
               (SELECT COUNT(*) FROM marketplace.stores WHERE is_open) as "open_stores!",
               (SELECT COUNT(*) FROM marketplace.products) as "products!",
               (SELECT COUNT(*) FROM marketplace.store_products) as "store_products!"
        "#,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Delete a store. Its store_products follow via ON DELETE CASCADE. The
/// caller has already established access; unknown and out-of-scope ids are
/// the same NotFound. A store with order history refuses: the FKs from
/// commerce.store_orders have no ON DELETE, and history must not be
/// rewritten to satisfy a delete.
pub async fn delete_store(conn: &mut PgConnection, store_id: Uuid) -> Result<(), StoreError> {
    // Cheap pre-check: a store that ever appeared in an order is part of
    // that history — closing it is the remedy, not deleting it.
    let (referenced,): (bool,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM commerce.store_orders WHERE store_id = $1
        )
        "#,
    )
    .bind(store_id)
    .fetch_one(&mut *conn)
    .await?;
    if referenced {
        return Err(StoreError::HasOrderHistory);
    }
    let result = sqlx::query!(
        r#"
        DELETE FROM marketplace.stores
        WHERE id = $1
        "#,
        store_id,
    )
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// Set (or clear) the store's banner storage key. Only `banner_key` is
/// stored — the public URL is composed at response time from config, so
/// a base-URL change (CDN cutover, new LAN IP) never rots stored rows.
/// The legacy `image_url` column is left alone: it keeps serving whatever
/// external URL the merchant typed, and the response layer prefers the
/// banner when both exist.
pub async fn set_banner(
    conn: &mut PgConnection,
    store_id: Uuid,
    banner_key: Option<&str>,
) -> Result<Store, StoreError> {
    sqlx::query_as!(
        Store,
        r#"
        UPDATE marketplace.stores
        SET banner_key = $2
        WHERE id = $1
        RETURNING id, merchant_id, name, description, image_url, banner_key, address_text,
                  lat, lng, category, delivery_fee, is_open, contact_phone, contact_email,
                  created_at, updated_at
        "#,
        store_id,
        banner_key,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StoreError::NotFound)
}
