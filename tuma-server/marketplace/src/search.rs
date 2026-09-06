//! Discovery: cross-store product search and the popular shelf.
//!
//! Two customer-facing reads over the same join — a store_product (what a
//! customer actually buys, with its per-store price) dressed by its
//! catalog identity (name, images) and the open store that fulfills it:
//!
//! - [`search_products`] matches catalog product names (the trigram index
//!   from migration 06 keeps the ILIKE indexed at any catalog size).
//! - [`popular_products`] ranks by `commerce.product_popularity` — a
//!   counter kept live by checkout (+1 per order line) and cancel (−1),
//!   backfilled once by migration 14. Deterministic "popular near you",
//!   no simulation, no ML (blueprint §23). An item nobody ordered never
//!   shows up.
//!
//! The commerce-schema read is deliberate and read-only: popularity is a
//! marketplace fact derived from commerce events, and keeping both
//! discovery queries beside each other keeps the handler dumb. The cover
//! subselect mirrors `catalog::menu_for_store` — lowest gallery position
//! is the cover.

use sqlx::PgConnection;
use uuid::Uuid;

/// One buyable thing, as discovery shows it: the store_product (id +
/// per-store price) dressed by its catalog identity and the open store
/// that fulfills it. Distance facts are attached by the handler when the
/// request carries a location, exactly like the store feed.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProductHit {
    pub store_product_id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    pub cover_key: Option<String>,
    /// The fulfilling store's coordinates — the handler turns these into
    /// distance/ETA when the request carries the customer's location.
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
}

/// Available products of open stores whose catalog name matches the
/// pattern, alphabetical. The handler wraps the raw term with
/// `stores::escape_like` before it gets here.
pub async fn search_products(
    conn: &mut PgConnection,
    pattern: &str,
    limit: i64,
) -> Result<Vec<ProductHit>, sqlx::Error> {
    sqlx::query_as!(
        ProductHit,
        r#"
        SELECT sp.id AS "store_product_id!",
               sp.store_id,
               s.name AS "store_name!",
               p.name,
               p.description,
               sp.price,
               p.image_url,
               (SELECT pi.storage_key FROM marketplace.product_images pi
                WHERE pi.product_id = sp.product_id ORDER BY pi.position, pi.id LIMIT 1) AS cover_key,
               s.lat AS store_lat,
               s.lng AS store_lng
        FROM marketplace.store_products sp
        JOIN marketplace.products p ON p.id = sp.product_id
        JOIN marketplace.stores s ON s.id = sp.store_id
        WHERE s.is_open AND sp.is_available AND p.name ILIKE $1
        ORDER BY p.name, s.name
        LIMIT $2
        "#,
        pattern,
        limit,
    )
    .fetch_all(&mut *conn)
    .await
}

/// The most-purchased products across open stores, by real order counts,
/// best first. The counter (migration 14) already excludes cancelled
/// orders — a cancel decrements it — so the read is a plain join; the
/// open-store rule stays a read-side fact. Ties break alphabetically.
pub async fn popular_products(
    conn: &mut PgConnection,
    limit: i64,
) -> Result<Vec<ProductHit>, sqlx::Error> {
    sqlx::query_as!(
        ProductHit,
        r#"
        SELECT sp.id AS "store_product_id!",
               sp.store_id,
               s.name AS "store_name!",
               p.name,
               p.description,
               sp.price,
               p.image_url,
               (SELECT pi.storage_key FROM marketplace.product_images pi
                WHERE pi.product_id = sp.product_id ORDER BY pi.position, pi.id LIMIT 1) AS cover_key,
               s.lat AS store_lat,
               s.lng AS store_lng
        FROM commerce.product_popularity pp
        JOIN marketplace.store_products sp ON sp.id = pp.store_product_id
        JOIN marketplace.products p ON p.id = sp.product_id
        JOIN marketplace.stores s ON s.id = sp.store_id
        WHERE s.is_open AND sp.is_available AND pp.order_lines > 0
        ORDER BY pp.order_lines DESC, p.name
        LIMIT $1
        "#,
        limit,
    )
    .fetch_all(&mut *conn)
    .await
}
