//! Discovery — one endpoint, two sections. Empty `q` is the default
//! discovery state (🔥 popular products + all open stores); a present
//! `q` narrows both sections to real matches. The customer is always a
//! `require_customer`-gated session, like the store feed.

use crate::app::{AppResult, AppState, UserContext};
use crate::routes::stores::{
    Located, StoreResponse, attach_distance, attach_distance_and_sort, validated_coords,
    validated_term,
};
use axum::extract::{Query, State};
use axum::{Extension, Json};
use marketplace::search;
use marketplace::stores;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A cap on the products section — the feed is a shelf, not a dump.
const PRODUCT_LIMIT: i64 = 20;

/// One product hit as the API returns it: the store_product (what the
/// customer buys, per-store price) dressed by its catalog identity and
/// the open store that fulfills it. Distance facts ride along exactly
/// like the store feed's — server-computed, never client math.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProductHitResponse {
    pub store_product_id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_min: Option<i64>,
    /// The fulfilling store's coordinates, unserialized — the shared geo
    /// helper places the row from these.
    #[serde(skip)]
    pub store_lat: Option<f64>,
    #[serde(skip)]
    pub store_lng: Option<f64>,
}

impl ProductHitResponse {
    fn from_hit(hit: search::ProductHit, base_url: &str) -> Self {
        Self {
            store_product_id: hit.store_product_id,
            store_id: hit.store_id,
            store_name: hit.store_name,
            name: hit.name,
            description: hit.description,
            price: hit.price,
            image_url: storage::resolve_image_url(
                base_url,
                hit.cover_key.as_deref(),
                hit.image_url.as_deref(),
            ),
            distance_m: None,
            eta_min: None,
            store_lat: hit.store_lat,
            store_lng: hit.store_lng,
        }
    }
}

impl Located for ProductHitResponse {
    fn coords(&self) -> (Option<f64>, Option<f64>) {
        (self.store_lat, self.store_lng)
    }
    fn distance_m(&self) -> Option<i64> {
        self.distance_m
    }
    fn set_distance(&mut self, distance_m: i64, eta_min: i64) {
        self.distance_m = Some(distance_m);
        self.eta_min = Some(eta_min);
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SearchResponse {
    /// Empty `q`: the most-purchased products (real order counts). With
    /// `q`: catalog-name matches, alphabetical. Availability and the
    /// open-store rule are enforced server-side either way.
    pub products: Vec<ProductHitResponse>,
    /// Empty `q`: all open stores. With `q`: name/category matches —
    /// nearest first when the request carries a location.
    pub stores: Vec<StoreResponse>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DiscoveryQuery {
    /// Search term; empty = the default discovery state.
    pub q: Option<String>,
    /// Customer latitude, -90..90.
    pub lat: Option<f64>,
    /// Customer longitude, -180..180.
    pub lng: Option<f64>,
}

#[utoipa::path(
    get,
    path = "/v1/search",
    params(("q" = Option<String>, Query, description = "Search term matched against product names (stores section: name + category); empty = popular products + all open stores"),
           ("lat" = Option<f64>, Query, description = "Customer latitude (-90..90); requires lng"),
           ("lng" = Option<f64>, Query, description = "Customer longitude (-180..180); requires lat")),
    responses(
        (status = 200, description = "Discovery: products and stores sections, both honoring the search term and the open-store rule", body = SearchResponse),
        (status = 400, description = "Only one of lat/lng, a coordinate out of range / not a number, or a search term over 100 characters"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
    ),
    tag = "search"
)]
#[tracing::instrument(name = "Search", skip_all)]
pub async fn search(
    State(app): State<AppState>,
    Extension(_context): Extension<UserContext>,
    Query(query): Query<DiscoveryQuery>,
) -> AppResult<Json<SearchResponse>> {
    let customer = validated_coords(query.lat, query.lng)?;
    let term = validated_term(query.q)?;

    let mut conn = app.db_pool.acquire().await?;
    let base_url = app.storage.public_base_url.clone();

    let (products, stores) = match term.as_deref() {
        Some(raw) => {
            let pattern = format!("%{}%", stores::escape_like(raw));
            (
                search::search_products(&mut conn, &pattern, PRODUCT_LIMIT).await?,
                stores::search_stores(&mut conn, &pattern).await?,
            )
        }
        None => (
            search::popular_products(&mut conn, 10).await?,
            stores::open_stores(&mut conn).await?,
        ),
    };

    let mut product_rows: Vec<ProductHitResponse> = products
        .into_iter()
        .map(|hit| ProductHitResponse::from_hit(hit, &base_url))
        .collect();
    // Products keep their server order (popularity, then name) — the
    // distance rides along for display only.
    attach_distance(&mut product_rows, customer);

    let mut store_rows: Vec<StoreResponse> = stores
        .into_iter()
        .map(|store| StoreResponse::from_store(store, &base_url))
        .collect();
    attach_distance_and_sort(&mut store_rows, customer);

    Ok(Json(SearchResponse {
        products: product_rows,
        stores: store_rows,
    }))
}
