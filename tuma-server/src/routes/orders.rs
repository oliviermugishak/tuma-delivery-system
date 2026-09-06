//! Customer and merchant order endpoints on the re-architected domain: one
//! checkout → one order group → one store order per store.
//!
//! Customer endpoints:
//! - `POST /v1/orders` — checkout: items from many stores, one address,
//!   one cash payment, split server-side into store orders (201; an
//!   idempotent retry returns the existing group with 200)
//! - `GET /v1/orders` — order groups, newest first (limit/offset paging)
//! - `GET /v1/orders/{id}` — one group with its store orders, item
//!   snapshots, and payment
//! - `POST /v1/orders/{id}/store-orders/{sid}/cancel` — cancel one store
//!   order while it is still on the premises
//!
//! Merchant endpoints:
//! - `GET /v1/merchant/orders` — incoming store orders across authorized
//!   stores (limit/offset paging)
//! - `GET /v1/merchant/store-orders/{id}` — the fulfillment sheet: items,
//!   delivery address, customer contact
//! - `PATCH /v1/merchant/store-orders/{id}` — advance status
//!
//! Order operations are the merchants' monopoly: the admin has no
//! order-mutation endpoints. Payment allocations are created automatically
//! at checkout as passive records — there is no collection action until a
//! real money system exists (then it becomes automatic, e.g. a MoMo
//! webhook).

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use commerce::orders::{
    self, CancelError, CheckoutError, GroupStatus, NewCheckout, NewCheckoutItem, OrderStatus,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

/// One frozen line of a store order.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OrderItemResponse {
    pub store_product_id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub unit_price: i64,
    pub quantity: i32,
}

/// A store order as the customer sees it: one store's slice of the
/// checkout, with its own status and totals.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StoreOrderResponse {
    pub id: Uuid,
    pub number: i64,
    pub store_id: Uuid,
    pub store_name: String,
    #[schema(value_type = String)]
    pub status: OrderStatus,
    pub subtotal: i64,
    pub delivery_fee: i64,
    pub total: i64,
    /// Why the order died — set at the cancel moment (merchant reject or
    /// customer change-of-mind), rendered on the customer's order detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_reason: Option<String>,
    /// The store's phone — the Get help sheet's Call row (absent on the
    /// checkout response; the detail is the contact surface).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_contact_phone: Option<String>,
    /// The store's email — the Get help sheet's Email row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_contact_email: Option<String>,
    pub items: Vec<OrderItemResponse>,
}

/// The customer-facing purchase: one checkout, N store orders, one
/// payment. `status` is derived from the children server-side.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OrderGroupResponse {
    pub id: Uuid,
    pub number: i64,
    pub address_text: String,
    pub address_lat: Option<f64>,
    pub address_lng: Option<f64>,
    pub subtotal: i64,
    pub delivery_total: i64,
    pub grand_total: i64,
    #[schema(value_type = String)]
    pub status: GroupStatus,
    #[schema(value_type = String)]
    pub payment_status: commerce::PaymentStatus,
    /// The checkout's rider note — the rider's Delivering card renders it.
    pub customer_note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub store_orders: Vec<StoreOrderResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GroupSummaryResponse {
    pub id: Uuid,
    pub number: i64,
    pub grand_total: i64,
    #[schema(value_type = String)]
    pub status: GroupStatus,
    /// Which stores are fulfilling this purchase.
    pub stores: Vec<String>,
    /// Total frozen items across the group's store orders — the active
    /// cards' "4 items" line and the history rows' distinguishing fact.
    pub items_count: i64,
    /// The first item's snapshot name — the active card's sub-line.
    pub first_item_name: Option<String>,
    /// The soonest ETA among the group's out-for-delivery deliveries
    /// (null unless something is picked_up).
    #[serde(with = "time::serde::rfc3339::option")]
    pub eta_target: Option<time::OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Shared paging query: a capped limit, offset past the first page. The
/// clamp (1..=200, default 50) is the house pattern — every paged list
/// reuses this struct rather than inventing its own bounds.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl PageQuery {
    pub(crate) fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 200)
    }
    pub(crate) fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// The merchant board's paging plus the status filter: a comma-separated
/// list of order statuses (`placed,accepted,preparing,picked_up` for the
/// Live board, `delivered,cancelled` for History). Absent or empty = no
/// filter — every status comes back, exactly as before this existed.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct MerchantOrdersQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

impl MerchantOrdersQuery {
    pub(crate) fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 200)
    }
    pub(crate) fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
    pub(crate) fn statuses(&self) -> AppResult<Vec<OrderStatus>> {
        match self.status.as_deref().map(str::trim) {
            None | Some("") => Ok(Vec::new()),
            Some(list) => list
                .split(',')
                .map(|label| {
                    OrderStatus::from_label(label.trim()).ok_or_else(|| {
                        AppError::BadRequest(format!("unknown order status: {label}"))
                    })
                })
                .collect(),
        }
    }
}

/// The operator's merchant surface — already proven to exist by the guard.
fn merchant_access(context: &UserContext) -> AppResult<crate::app::MerchantAccess> {
    context
        .merchant_access()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

/// The route cache a handoff stores on the delivery: the adapter's road
/// route when one is available, otherwise the honest stand-in — an ETA
/// from the locked ride speed and NO geometry. A routing-backend failure
/// degrades to the same fallback rather than blocking the handoff; the
/// cache re-fetches on a stray anyway (tracking doc §4).
async fn handoff_route(
    routing: &dyn routing::RoutingProvider,
    from: routing::Coord,
    to: routing::Coord,
) -> commerce::CachedRoute {
    let now = time::OffsetDateTime::now_utc();
    match routing.route(from, to).await {
        Ok(Some(route)) => commerce::CachedRoute {
            polyline: Some(route.polyline),
            eta_target: now + time::Duration::seconds(route.duration_secs),
        },
        other => {
            if let Err(error) = other {
                tracing::warn!(
                    error = %error,
                    "routing backend failed at handoff — falling back to the ride-speed estimate"
                );
            }
            let distance = marketplace::geo::haversine_m(from.lat, from.lng, to.lat, to.lng);
            commerce::CachedRoute {
                polyline: None,
                eta_target: now + time::Duration::minutes(marketplace::geo::ride_minutes(distance)),
            }
        }
    }
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CheckoutInput {
    /// The delivery line is server-owned: with a pin, the text is derived
    /// from it (cached) and whatever the client sent is a fallback at
    /// best — a coordinate pair is never accepted as a name. Only a
    /// pin-less checkout still requires the client's own line.
    #[validate(length(max = 300, message = "address must be at most 300 characters"))]
    pub address_text: Option<String>,
    #[validate(range(min = -90.0, max = 90.0, message = "lat must be between -90 and 90"))]
    pub address_lat: Option<f64>,
    #[validate(range(min = -180.0, max = 180.0, message = "lng must be between -180 and 180"))]
    pub address_lng: Option<f64>,
    /// A client-generated key: retrying the same checkout with the same key
    /// returns the group it already created instead of placing twice.
    #[validate(length(max = 100, message = "idempotency_key must be at most 100 characters"))]
    pub idempotency_key: Option<String>,
    /// The checkout's "Note for rider · optional" — one line the rider
    /// sees on the Delivering card. Trimmed server-side; empty becomes
    /// None (P2: nothing renders for an unknown).
    #[validate(length(max = 140, message = "note must be at most 140 characters"))]
    pub customer_note: Option<String>,
    /// At least one line, at most fifty (review P12: an unbounded cart is
    /// a denial-of-wallet and a denial-of-database — 50 lines is generous
    /// for real baskets). Each carries the store_product id and quantity.
    // `nested` runs each line's own validators — without it the Vec's
    // length check is the only thing that fires.
    #[validate(
        length(min = 1, max = 50, message = "cart is limited to 50 items"),
        nested
    )]
    pub items: Vec<CheckoutLineInput>,
}

#[derive(Debug, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct CheckoutLineInput {
    pub store_product_id: Uuid,
    #[validate(range(min = 1, max = 99, message = "quantity must be between 1 and 99"))]
    pub quantity: i32,
}

#[utoipa::path(
    post,
    path = "/v1/orders",
    request_body = CheckoutInput,
    responses(
        (status = 201, description = "Checkout placed — one group with its store orders", body = OrderGroupResponse),
        (status = 200, description = "Already placed — the retry's idempotency key matched an existing group", body = OrderGroupResponse),
        (status = 400, description = "Empty cart or invalid input"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
        (status = 409, description = "Store closed, item unavailable, or not enough stock — details name the offenders"),
        (status = 422, description = "Invalid input"),
    ),
    tag = "orders"
)]
#[tracing::instrument(name = "Checkout", skip_all)]
pub async fn checkout(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CheckoutInput>,
) -> AppResult<(StatusCode, Json<OrderGroupResponse>)> {
    let user_id = context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    // The client never sends totals — only ids and quantities.
    let mut conn = app.db_pool.acquire().await?;
    // The delivery line is server-owned: with a pin, the place name is
    // derived (a cache hit for a saved address) and the client's line is
    // a fallback at best — a coordinate pair is never a name. Without a
    // pin, the client's own line is the only honest thing available.
    let client_text = input
        .address_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .filter(|text| !geocoding::is_coordinate_pair(text));
    let address_text = match (input.address_lat, input.address_lng) {
        (Some(lat), Some(lng)) => {
            geocoding::place_name(
                &mut conn,
                app.geocoding.as_ref(),
                geocoding::Coord { lat, lng },
                client_text.unwrap_or("Pinned location"),
            )
            .await?
        }
        _ => client_text
            .map(str::to_string)
            .ok_or_else(|| AppError::BadRequest("delivery address must not be empty".into()))?,
    };
    match orders::create_checkout(
        &mut conn,
        user_id,
        NewCheckout {
            address_text,
            address_lat: input.address_lat,
            address_lng: input.address_lng,
            idempotency_key: input.idempotency_key.clone(),
            customer_note: input
                .customer_note
                .as_deref()
                .map(str::trim)
                .filter(|note| !note.is_empty())
                .map(str::to_string),
            items: input
                .items
                .iter()
                .map(|line| NewCheckoutItem {
                    store_product_id: line.store_product_id,
                    quantity: line.quantity,
                })
                .collect(),
        },
    )
    .await
    {
        Ok(created) => Ok((StatusCode::CREATED, Json(group_response(created)))),
        Err(CheckoutError::AlreadyPlaced(group)) => {
            let detail = orders::group_detail_for_user(&mut conn, user_id, group.id)
                .await?
                .ok_or_else(|| AppError::Internal("existing group disappeared".into()))?;
            Ok((StatusCode::OK, Json(group_detail_response(detail))))
        }
        Err(CheckoutError::IdempotencyRace { user_id, key }) => {
            // Lost the concurrent-same-key race: the aborted transaction
            // can't be queried, so re-fetch the winner on a fresh one.
            let group = orders::group_by_idempotency_key(&mut conn, user_id, &key)
                .await?
                .ok_or_else(|| AppError::Internal("idempotency race winner missing".into()))?;
            let detail = orders::group_detail_for_user(&mut conn, user_id, group.id)
                .await?
                .ok_or_else(|| AppError::Internal("existing group disappeared".into()))?;
            Ok((StatusCode::OK, Json(group_detail_response(detail))))
        }
        Err(error) => Err(error.into()),
    }
}

/// Fold a created checkout into the customer-facing group response.
fn group_response(created: orders::CheckoutCreated) -> OrderGroupResponse {
    let statuses: Vec<OrderStatus> = created
        .store_orders
        .iter()
        .map(|slice| slice.order.status)
        .collect();
    OrderGroupResponse {
        id: created.group.id,
        number: created.group.number,
        address_text: created.group.address_text,
        address_lat: created.group.address_lat,
        address_lng: created.group.address_lng,
        subtotal: created.group.subtotal,
        delivery_total: created.group.delivery_total,
        grand_total: created.group.grand_total,
        status: orders::derive_group_status(&statuses),
        payment_status: created.payment.status,
        customer_note: created.group.customer_note,
        created_at: created.group.created_at,
        store_orders: created
            .store_orders
            .into_iter()
            .map(|slice| StoreOrderResponse {
                id: slice.order.id,
                number: slice.order.number,
                store_id: slice.order.store_id,
                store_name: slice.store_name,
                status: slice.order.status,
                subtotal: slice.order.subtotal,
                delivery_fee: slice.order.delivery_fee,
                total: slice.order.total,
                cancel_reason: slice.order.cancel_reason,
                store_contact_phone: None,
                store_contact_email: None,
                items: slice
                    .items
                    .into_iter()
                    .map(|item| OrderItemResponse {
                        store_product_id: item.store_product_id,
                        product_id: item.product_id,
                        product_name: item.product_name_snapshot,
                        unit_price: item.unit_price,
                        quantity: item.quantity,
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Fold a loaded group detail into the customer-facing response.
fn group_detail_response(detail: orders::GroupDetail) -> OrderGroupResponse {
    let statuses: Vec<OrderStatus> = detail
        .store_orders
        .iter()
        .map(|slice| slice.order.status)
        .collect();
    OrderGroupResponse {
        id: detail.group.id,
        number: detail.group.number,
        address_text: detail.group.address_text,
        address_lat: detail.group.address_lat,
        address_lng: detail.group.address_lng,
        subtotal: detail.group.subtotal,
        delivery_total: detail.group.delivery_total,
        grand_total: detail.group.grand_total,
        status: orders::derive_group_status(&statuses),
        payment_status: detail.payment.status,
        customer_note: detail.group.customer_note,
        created_at: detail.group.created_at,
        store_orders: detail
            .store_orders
            .into_iter()
            .map(|slice| StoreOrderResponse {
                id: slice.order.id,
                number: slice.order.number,
                store_id: slice.order.store_id,
                store_name: slice.store_name,
                status: slice.order.status,
                subtotal: slice.order.subtotal,
                delivery_fee: slice.order.delivery_fee,
                total: slice.order.total,
                cancel_reason: slice.order.cancel_reason,
                store_contact_phone: slice.store_contact_phone,
                store_contact_email: slice.store_contact_email,
                items: slice
                    .items
                    .into_iter()
                    .map(|item| OrderItemResponse {
                        store_product_id: item.store_product_id,
                        product_id: item.product_id,
                        product_name: item.product_name_snapshot,
                        unit_price: item.unit_price,
                        quantity: item.quantity,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[utoipa::path(
    get,
    path = "/v1/orders",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip")),
    responses(
        (status = 200, description = "Order groups, newest first", body = Vec<GroupSummaryResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
    ),
    tag = "orders"
)]
#[tracing::instrument(name = "List order groups", skip_all)]
pub async fn list_orders(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(page): Query<PageQuery>,
) -> AppResult<Json<Vec<GroupSummaryResponse>>> {
    let user_id = context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let mut conn = app.db_pool.acquire().await?;
    let rows =
        orders::group_summaries_for_user(&mut conn, user_id, page.limit(), page.offset()).await?;
    Ok(Json(
        rows.into_iter()
            .map(|(group, statuses, extras)| GroupSummaryResponse {
                id: group.id,
                number: group.number,
                grand_total: group.grand_total,
                status: orders::derive_group_status(
                    &statuses.iter().map(|row| row.status).collect::<Vec<_>>(),
                ),
                stores: statuses.iter().map(|row| row.store_name.clone()).collect(),
                items_count: extras.items_count,
                first_item_name: extras.first_item_name,
                eta_target: extras.eta_target,
                created_at: group.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/v1/orders/{id}",
    params(("id" = Uuid, Path, description = "Order group id")),
    responses(
        (status = 200, description = "One order group with its store orders and payment", body = OrderGroupResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
        (status = 404, description = "Group not found (or not yours)"),
    ),
    tag = "orders"
)]
#[tracing::instrument(name = "Get order group", skip_all)]
pub async fn get_order(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OrderGroupResponse>> {
    let user_id = context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let mut conn = app.db_pool.acquire().await?;
    let detail = orders::group_detail_for_user(&mut conn, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    Ok(Json(group_detail_response(detail)))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CancelInput {
    /// Optional client explanation — recorded in a later slice when the
    /// cancellation event log lands; accepted now so the UI can send it.
    #[validate(length(max = 500, message = "reason must be at most 500 characters"))]
    pub reason: Option<String>,
}

#[utoipa::path(
    post,
    path = "/v1/orders/{id}/store-orders/{store_order_id}/cancel",
    request_body = CancelInput,
    params(("id" = Uuid, Path, description = "Order group id"),
           ("store_order_id" = Uuid, Path, description = "Store order id")),
    responses(
        (status = 200, description = "The cancelled store order", body = StoreOrderResponse),
        (status = 400, description = "The order can no longer be cancelled (already out)"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
        (status = 404, description = "Not one of your orders"),
    ),
    tag = "orders"
)]
#[tracing::instrument(name = "Cancel store order", skip_all)]
pub async fn cancel_store_order(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path((id, store_order_id)): Path<(Uuid, Uuid)>,
    ValidatedJson(input): ValidatedJson<CancelInput>,
) -> AppResult<Json<StoreOrderResponse>> {
    let user_id = context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let mut conn = app.db_pool.acquire().await?;
    // The reason the customer typed is no longer discarded — it lands on
    // the row and rides the responses.
    let _cancelled = orders::cancel_own_store_order(
        &mut conn,
        user_id,
        id,
        store_order_id,
        input.reason.as_deref(),
    )
    .await
    .map_err(|error| match error {
        CancelError::NotFound => AppError::NotFound("order not found".into()),
        CancelError::Illegal(transition) => transition.into(),
        CancelError::Database(error) => AppError::Database(error),
    })?;

    // Reload with the store name for the response.
    let detail = orders::group_detail_for_user(&mut conn, user_id, id)
        .await?
        .ok_or_else(|| AppError::Internal("cancelled order's group disappeared".into()))?;
    let slice = detail
        .store_orders
        .into_iter()
        .find(|slice| slice.order.id == store_order_id)
        .ok_or_else(|| AppError::Internal("cancelled order vanished from its group".into()))?;
    Ok(Json(StoreOrderResponse {
        id: slice.order.id,
        number: slice.order.number,
        store_id: slice.order.store_id,
        store_name: slice.store_name,
        status: slice.order.status,
        subtotal: slice.order.subtotal,
        delivery_fee: slice.order.delivery_fee,
        total: slice.order.total,
        cancel_reason: slice.order.cancel_reason,
        store_contact_phone: slice.store_contact_phone,
        store_contact_email: slice.store_contact_email,
        items: slice
            .items
            .into_iter()
            .map(|item| OrderItemResponse {
                store_product_id: item.store_product_id,
                product_id: item.product_id,
                product_name: item.product_name_snapshot,
                unit_price: item.unit_price,
                quantity: item.quantity,
            })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Merchant endpoints
// ---------------------------------------------------------------------------

/// A store order on the merchant's board: what to fulfill, for whom, and
/// where.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantStoreOrderResponse {
    pub id: Uuid,
    pub number: i64,
    pub store_id: Uuid,
    pub store_name: String,
    #[schema(value_type = String)]
    pub status: OrderStatus,
    pub total: i64,
    pub address_text: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[utoipa::path(
    get,
    path = "/v1/merchant/orders",
    params(("limit" = Option<i64>, Query, description = "Page size, 1-200 (default 50)"),
           ("offset" = Option<i64>, Query, description = "Rows to skip"),
           ("status" = Option<String>, Query, description = "Comma-separated status filter (placed,accepted,preparing,picked_up,delivered,cancelled). Absent = every status.")),
    responses(
        (status = 200, description = "Incoming store orders across authorized stores, newest first", body = Vec<MerchantStoreOrderResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "List merchant store orders", skip_all)]
pub async fn list_merchant_orders(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Query(page): Query<MerchantOrdersQuery>,
) -> AppResult<Json<Vec<MerchantStoreOrderResponse>>> {
    let access = merchant_access(&context)?;
    let statuses = page.statuses()?;
    let mut conn = app.db_pool.acquire().await?;
    // Owner grants admit whole merchants; scoped manager grants admit
    // exactly their stores — one paged query covers both (a per-grant
    // loop with a shared limit/offset duplicates and skips pages).
    let mut owner_merchant_ids = Vec::new();
    let mut scoped_store_ids = Vec::new();
    for grant in &access.grants {
        match grant.store_ids.as_deref() {
            None => owner_merchant_ids.push(grant.merchant_id),
            Some(stores) => scoped_store_ids.extend_from_slice(stores),
        }
    }
    let rows = orders::store_orders_for_grants(
        &mut conn,
        &owner_merchant_ids,
        &scoped_store_ids,
        &statuses[..],
        page.limit(),
        page.offset(),
    )
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| MerchantStoreOrderResponse {
                id: row.id,
                number: row.number,
                store_id: row.store_id,
                store_name: row.store_name,
                status: row.status,
                total: row.total,
                address_text: row.address_text,
                created_at: row.created_at,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct AdvanceStatusInput {
    pub status: String,
    /// Why the order is being cancelled — stored only on the cancel
    /// transition, rendered to the customer, the merchant sheet, and the
    /// receipt. Other advances ignore it.
    #[validate(length(max = 500, message = "reason must be at most 500 characters"))]
    pub reason: Option<String>,
}

/// One frozen line, with the line total the customer saw.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantOrderItemResponse {
    pub product_name: String,
    pub unit_price: i64,
    pub quantity: i32,
}

impl From<commerce::OrderItem> for MerchantOrderItemResponse {
    fn from(item: commerce::OrderItem) -> Self {
        Self {
            product_name: item.product_name_snapshot,
            unit_price: item.unit_price,
            quantity: item.quantity,
        }
    }
}

/// The full store-order detail for the operator: what to prepare (items),
/// where it goes (address + coordinates), who to contact (customer name +
/// phone), and the money. This is the fulfillment sheet.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MerchantStoreOrderDetailResponse {
    pub id: Uuid,
    pub number: i64,
    pub store_id: Uuid,
    pub store_name: String,
    #[schema(value_type = String)]
    pub status: OrderStatus,
    pub subtotal: i64,
    pub delivery_fee: i64,
    pub total: i64,
    /// Why the order died — set at the cancel moment, shown on the
    /// fulfillment sheet and the History receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_reason: Option<String>,
    pub address_text: String,
    pub address_lat: Option<f64>,
    pub address_lng: Option<f64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    #[schema(value_type = String)]
    pub payment_status: commerce::PaymentStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub items: Vec<MerchantOrderItemResponse>,
}

/// One store order's detail — the fulfillment sheet: what to prepare, where
/// it goes, and who to call. Store-scoped managers may read only their own
/// store's orders; a foreign one is a plain 404.
#[utoipa::path(
    get,
    path = "/v1/merchant/store-orders/{id}",
    params(("id" = Uuid, Path, description = "Store order id")),
    responses(
        (status = 200, description = "The store order's detail with items and the customer contact", body = MerchantStoreOrderDetailResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's orders"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Get merchant store order", skip_all)]
pub async fn get_merchant_store_order(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<MerchantStoreOrderDetailResponse>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;

    let (_, merchant_id, store_id) = orders::store_order_scope(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    if !access.can_access_store(merchant_id, store_id) {
        return Err(AppError::NotFound("order not found".into()));
    }

    let detail = orders::store_order_detail(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    let items = orders::items_for_store_order(&mut conn, id).await?;

    Ok(Json(MerchantStoreOrderDetailResponse {
        id: detail.id,
        number: detail.number,
        store_id: detail.store_id,
        store_name: detail.store_name,
        status: detail.status,
        subtotal: detail.subtotal,
        delivery_fee: detail.delivery_fee,
        total: detail.total,
        cancel_reason: detail.cancel_reason,
        address_text: detail.address_text,
        address_lat: detail.address_lat,
        address_lng: detail.address_lng,
        customer_name: detail.customer_name,
        customer_phone: detail.customer_phone,
        payment_status: detail.payment_status,
        created_at: detail.created_at,
        items: items
            .into_iter()
            .map(MerchantOrderItemResponse::from)
            .collect(),
    }))
}

/// Advance one of the operator's store orders (accept, prepare, hand off…;
/// or cancel to reject). Store-scoped managers can act only on their own
/// store's orders — a foreign one is a plain 404.
#[utoipa::path(
    patch,
    path = "/v1/merchant/store-orders/{id}",
    params(("id" = Uuid, Path, description = "Store order id")),
    request_body = AdvanceStatusInput,
    responses(
        (status = 200, description = "Store order advanced", body = MerchantStoreOrderResponse),
        (status = 400, description = "Invalid or illegal status transition"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's orders"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Advance store order", skip_all)]
pub async fn advance_store_order(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<AdvanceStatusInput>,
) -> AppResult<Json<MerchantStoreOrderResponse>> {
    let access = merchant_access(&context)?;
    let next = OrderStatus::from_label(&input.status)
        .ok_or_else(|| AppError::BadRequest("invalid status value".into()))?;
    // The reason rides only the cancel — the one transition that ends an
    // order; every other advance ignores it.
    let reason = if next == OrderStatus::Cancelled {
        input.reason.as_deref()
    } else {
        None
    };

    let mut conn = app.db_pool.acquire().await?;
    let (_, merchant_id, store_id) = orders::store_order_scope(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    if !access.can_access_store(merchant_id, store_id) {
        return Err(AppError::NotFound("order not found".into()));
    }

    let order = orders::advance_store_order_status(&mut conn, id, next, reason).await?;
    // (The cash settlement lives inside the domain advance's transaction —
    // delivery = payment, one atomic event, doc §5.)
    let row = orders::merchant_store_order_row(&mut conn, order.id)
        .await?
        .ok_or_else(|| AppError::Internal("advanced order disappeared".into()))?;
    Ok(Json(MerchantStoreOrderResponse {
        id: row.id,
        number: row.number,
        store_id: row.store_id,
        store_name: row.store_name,
        status: row.status,
        total: row.total,
        address_text: row.address_text,
        created_at: row.created_at,
    }))
}

/// The fulfillment sheet's "Handed to rider" action (tracking doc §5): the
/// merchant types the rider's unique number, the server validates an
/// active rider, assigns the delivery, and advances the order to
/// `picked_up`. Runs again while the delivery isn't delivered (the
/// wrong-number remedy); after `delivered` the assignment is frozen.
/// Foreign store orders are a plain 404, like every merchant route.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HandoffInput {
    /// The rider number the store asked for — the whole assignment
    /// interface; there is no directory picker.
    pub rider_number: i64,
}

#[utoipa::path(
    post,
    path = "/v1/merchant/store-orders/{id}/handoff",
    params(("id" = Uuid, Path, description = "Store order id")),
    request_body = HandoffInput,
    responses(
        (status = 200, description = "Rider assigned, order handed over (picked_up)", body = MerchantStoreOrderResponse),
        (status = 400, description = "The order is not in a handable state (preparing, or re-assign while picked_up)"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "No merchant membership"),
        (status = 404, description = "Not one of this operator's orders, or no active rider with that number"),
    ),
    tag = "merchant"
)]
#[tracing::instrument(name = "Hand off to rider", skip_all)]
pub async fn handoff_store_order(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    axum::Json(input): axum::Json<HandoffInput>,
) -> AppResult<Json<MerchantStoreOrderResponse>> {
    let access = merchant_access(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let (_, merchant_id, store_id) = orders::store_order_scope(&mut conn, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    if !access.can_access_store(merchant_id, store_id) {
        return Err(AppError::NotFound("order not found".into()));
    }

    let rider = accounts::riders::active_by_number(&mut conn, input.rider_number)
        .await?
        .ok_or_else(|| AppError::NotFound("no active rider with that number".into()))?;

    // Route cache at handoff (tracking doc §4): the destination never
    // moves mid-delivery, so the road route is fetched once here. With no
    // Directions key configured the adapter answers None and the honest
    // fallback stands in: an ETA from the locked ride speed, no geometry.
    let points = commerce::deliveries::route_context(&mut conn, id)
        .await?
        .filter(|points| {
            points.store_lat.is_some()
                && points.store_lng.is_some()
                && points.destination_lat.is_some()
                && points.destination_lng.is_some()
        });
    let cached = match points {
        Some(points) => Some(
            handoff_route(
                app.routing.as_ref(),
                routing::Coord {
                    lat: points.store_lat.unwrap(),
                    lng: points.store_lng.unwrap(),
                },
                routing::Coord {
                    lat: points.destination_lat.unwrap(),
                    lng: points.destination_lng.unwrap(),
                },
            )
            .await,
        ),
        None => None,
    };

    let order = commerce::deliveries::handoff(&mut conn, id, rider.id, cached).await?;
    let row = orders::merchant_store_order_row(&mut conn, order.id)
        .await?
        .ok_or_else(|| AppError::Internal("handed-over order disappeared".into()))?;
    Ok(Json(MerchantStoreOrderResponse {
        id: row.id,
        number: row.number,
        store_id: row.store_id,
        store_name: row.store_name,
        status: row.status,
        total: row.total,
        address_text: row.address_text,
        created_at: row.created_at,
    }))
}
