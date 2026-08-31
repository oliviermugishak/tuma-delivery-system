//! Commerce: checkouts, order groups, store orders, deliveries, payments.
//!
//! The domain rules (Tuma_V1_Brief.md, marketplace re-architecture):
//! - One checkout = one `order_group`; every participating store gets its
//!   own `store_order` — the fulfillment boundary. Items hang off store
//!   orders, never off the group.
//! - Placement is one transaction: validate everything, reserve stock,
//!   insert the group, its store orders, item snapshots, one delivery per
//!   store order, the payment, and its allocations — all or nothing.
//! - The server computes every number. Clients send store_product ids and
//!   quantities only.
//! - Status (the six-value machine) lives at store-order level; the
//!   group's state is derived from its children, never stored.
//! - One payment per checkout; `payment_allocations` is the explicit
//!   ledger of which slice of it belongs to which store order.

use serde::{Deserialize, Serialize};
use sqlx::{Acquire, PgConnection};
use std::collections::BTreeMap;
use time::OffsetDateTime;
use uuid::Uuid;

/// The six store-order statuses. The PostgreSQL enum and the JSON wire
/// value share the same snake_case labels, so there is one spelling of
/// each state across the whole system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "commerce.order_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Placed,
    Accepted,
    Preparing,
    PickedUp,
    Delivered,
    Cancelled,
}

impl OrderStatus {
    /// Whether moving from `self` to `next` is legal. Forward-only along
    /// the happy path; cancellation reachable while the order is still on
    /// the premises (placed, accepted, preparing). Terminal states accept
    /// no moves.
    pub fn can_transition_to(self, next: OrderStatus) -> bool {
        use OrderStatus as S;
        matches!(
            (self, next),
            (S::Placed, S::Accepted)
                | (S::Placed, S::Cancelled)
                | (S::Accepted, S::Preparing)
                | (S::Accepted, S::Cancelled)
                | (S::Preparing, S::PickedUp)
                | (S::Preparing, S::Cancelled)
                | (S::PickedUp, S::Delivered)
        )
    }

    /// The canonical snake_case label — the PostgreSQL enum value, the
    /// JSON wire value, and this string are one spelling.
    pub fn label(self) -> &'static str {
        match self {
            OrderStatus::Placed => "placed",
            OrderStatus::Accepted => "accepted",
            OrderStatus::Preparing => "preparing",
            OrderStatus::PickedUp => "picked_up",
            OrderStatus::Delivered => "delivered",
            OrderStatus::Cancelled => "cancelled",
        }
    }

    /// Convert a snake_case label back to the enum variant. Returns `None`
    /// for unknown labels so callers can reject input cleanly.
    pub fn from_label(label: &str) -> Option<OrderStatus> {
        match label {
            "placed" => Some(OrderStatus::Placed),
            "accepted" => Some(OrderStatus::Accepted),
            "preparing" => Some(OrderStatus::Preparing),
            "picked_up" => Some(OrderStatus::PickedUp),
            "delivered" => Some(OrderStatus::Delivered),
            "cancelled" => Some(OrderStatus::Cancelled),
            _ => None,
        }
    }
}

/// The order group's overall state, derived from its store orders — never
/// a second state machine to keep truthful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupStatus {
    InProgress,
    PartiallyFulfilled,
    Completed,
    Cancelled,
}

impl GroupStatus {
    pub fn label(self) -> &'static str {
        match self {
            GroupStatus::InProgress => "in_progress",
            GroupStatus::PartiallyFulfilled => "partially_fulfilled",
            GroupStatus::Completed => "completed",
            GroupStatus::Cancelled => "cancelled",
        }
    }
}

/// Derive the group state from its children's statuses:
/// all cancelled → cancelled · all delivered → completed ·
/// any in-flight child → in_progress (partially_fulfilled once something
/// has landed) · a delivered/cancelled mix with nothing in flight →
/// partially_fulfilled. A single-order group mirrors its child exactly.
pub fn derive_group_status(statuses: &[OrderStatus]) -> GroupStatus {
    if statuses.is_empty() {
        return GroupStatus::InProgress;
    }
    let in_flight = |s: &OrderStatus| {
        matches!(
            s,
            OrderStatus::Placed
                | OrderStatus::Accepted
                | OrderStatus::Preparing
                | OrderStatus::PickedUp
        )
    };
    if statuses.iter().all(|s| *s == OrderStatus::Cancelled) {
        return GroupStatus::Cancelled;
    }
    if statuses.iter().all(|s| *s == OrderStatus::Delivered) {
        return GroupStatus::Completed;
    }
    if statuses.iter().any(in_flight) {
        let landed = statuses
            .iter()
            .filter(|s| matches!(s, OrderStatus::Delivered | OrderStatus::Cancelled))
            .count();
        return if landed > 0 {
            GroupStatus::PartiallyFulfilled
        } else {
            GroupStatus::InProgress
        };
    }
    GroupStatus::PartiallyFulfilled
}

/// A row of `commerce.order_groups`. No status column — see
/// [`derive_group_status`].
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderGroup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub number: i64,
    pub address_text: String,
    pub address_lat: Option<f64>,
    pub address_lng: Option<f64>,
    pub subtotal: i64,
    pub delivery_total: i64,
    pub grand_total: i64,
    pub idempotency_key: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A row of `commerce.store_orders` — what a store fulfills.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreOrder {
    pub id: Uuid,
    pub order_group_id: Uuid,
    pub merchant_id: Uuid,
    pub store_id: Uuid,
    pub number: i64,
    pub status: OrderStatus,
    pub subtotal: i64,
    pub delivery_fee: i64,
    pub total: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A row of `commerce.order_items`. The name + price snapshot is frozen at
/// order time — later catalog edits never rewrite history.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub store_order_id: Uuid,
    pub store_product_id: Uuid,
    pub product_id: Uuid,
    pub product_name_snapshot: String,
    pub unit_price: i64,
    pub quantity: i32,
}

/// A row of `commerce.payments`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub order_group_id: Uuid,
    pub provider: PaymentProvider,
    pub amount: i64,
    pub currency: String,
    pub status: PaymentStatus,
    pub provider_reference: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "commerce.payment_provider", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentProvider {
    CashOnDelivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "commerce.payment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Collected,
    Refunded,
}

impl PaymentStatus {
    pub fn label(self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Collected => "collected",
            PaymentStatus::Refunded => "refunded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "commerce.allocation_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatus {
    Pending,
    Settled,
    Refunded,
}

/// A row of `commerce.payment_allocations` — the ledger slice of the one
/// customer payment that belongs to a store order.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentAllocation {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub store_order_id: Uuid,
    pub merchant_id: Uuid,
    pub store_id: Uuid,
    pub amount: i64,
    pub status: AllocationStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// What the checkout sends: where to deliver, an optional idempotency key,
/// and store_product id + quantity per line. Everything else — grouping,
/// prices, stock, totals, the split — is the server's job.
#[derive(Debug, Clone)]
pub struct NewCheckout {
    pub address_text: String,
    pub address_lat: Option<f64>,
    pub address_lng: Option<f64>,
    pub idempotency_key: Option<String>,
    pub items: Vec<NewCheckoutItem>,
}

#[derive(Debug, Clone)]
pub struct NewCheckoutItem {
    pub store_product_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Clone)]
pub struct StockShort {
    pub product_name: String,
    pub available: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum CheckoutError {
    #[error("the cart is empty")]
    EmptyCart,
    /// The same store_product twice in one checkout — a client bug, not a
    /// world-change to recover from.
    #[error("duplicate cart items")]
    DuplicateItems,
    /// These store_products are missing, paused, or sold by a store that is
    /// closed — the same answer for the customer either way.
    #[error("one or more items are no longer available")]
    Unavailable { store_product_ids: Vec<Uuid> },
    #[error(
        "these stores are not accepting orders right now: {}",
        stores.join(", ")
    )]
    StoreClosed { stores: Vec<String> },
    #[error(
        "not enough stock for: {}",
        items
            .iter()
            .map(|short| short.product_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )]
    InsufficientStock { items: Vec<StockShort> },
    /// A retried checkout whose idempotency key already created a group —
    /// not a failure; the handler returns the existing group.
    #[error("checkout already placed")]
    AlreadyPlaced(Box<OrderGroup>),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("order not found")]
    OrderNotFound,
    #[error("an order cannot move from {from} to {to}")]
    Illegal {
        from: &'static str,
        to: &'static str,
    },
    #[error("a rider picks the order up — use the handoff action with their rider number")]
    HandoffRequired,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What the checkout created, fully loaded for the response.
pub struct CheckoutCreated {
    pub group: OrderGroup,
    pub store_orders: Vec<StoreOrderCreated>,
    pub payment: Payment,
}

/// One store's slice of a fresh checkout: its order, its display name, and
/// its frozen items.
pub struct StoreOrderCreated {
    pub order: StoreOrder,
    pub store_name: String,
    pub items: Vec<OrderItem>,
}

/// Place a checkout. Atomic and idempotent: with a key the client already
/// used, the existing group comes back instead of a duplicate.
pub async fn create_checkout(
    conn: &mut PgConnection,
    user_id: Uuid,
    checkout: NewCheckout,
) -> Result<CheckoutCreated, CheckoutError> {
    if checkout.items.is_empty() {
        return Err(CheckoutError::EmptyCart);
    }
    let mut seen = std::collections::HashSet::new();
    for item in &checkout.items {
        if item.quantity <= 0 {
            return Err(CheckoutError::Unavailable {
                store_product_ids: vec![item.store_product_id],
            });
        }
        if !seen.insert(item.store_product_id) {
            return Err(CheckoutError::DuplicateItems);
        }
    }

    // Idempotent retry: the same key on the same account returns the
    // checkout it already created.
    if let Some(key) = &checkout.idempotency_key
        && let Some(existing) = group_by_idempotency_key(&mut *conn, user_id, key).await?
    {
        return Err(CheckoutError::AlreadyPlaced(Box::new(existing)));
    }

    let ids: Vec<Uuid> = checkout
        .items
        .iter()
        .map(|item| item.store_product_id)
        .collect();
    let rows = sqlx::query!(
        r#"
        SELECT sp.id, sp.price, sp.stock, sp.is_available,
               s.id AS store_id, s.name AS store_name, s.is_open AS store_is_open,
               s.delivery_fee, s.merchant_id,
               m.status::text AS merchant_status,
               sp.product_id,
               p.name AS product_name
        FROM marketplace.store_products sp
        JOIN marketplace.stores s ON s.id = sp.store_id
        JOIN marketplace.merchants m ON m.id = s.merchant_id
        JOIN marketplace.products p ON p.id = sp.product_id
        WHERE sp.id = ANY($1)
        "#,
        &ids[..],
    )
    .fetch_all(&mut *conn)
    .await?;

    // Every id must resolve; missing/paused ones are named back.
    let mut missing: Vec<Uuid> = ids
        .iter()
        .filter(|id| !rows.iter().any(|row| &row.id == *id))
        .copied()
        .collect();

    struct Line {
        store_product_id: Uuid,
        store_id: Uuid,
        store_name: String,
        merchant_id: Uuid,
        delivery_fee: i64,
        product_id: Uuid,
        product_name: String,
        unit_price: i64,
        stock: Option<i64>,
        quantity: i32,
    }
    let mut lines: Vec<Line> = Vec::with_capacity(rows.len());
    let mut closed_stores: Vec<String> = Vec::new();
    for row in &rows {
        let merchant_active = row.merchant_status.as_deref() == Some("active");
        let quantity = checkout
            .items
            .iter()
            .find(|item| item.store_product_id == row.id)
            .map(|item| item.quantity)
            .unwrap_or_default();
        if !row.is_available || !merchant_active {
            missing.push(row.id);
            continue;
        }
        if !row.store_is_open && !closed_stores.contains(&row.store_name) {
            closed_stores.push(row.store_name.clone());
        }
        lines.push(Line {
            store_product_id: row.id,
            store_id: row.store_id,
            store_name: row.store_name.clone(),
            merchant_id: row.merchant_id,
            delivery_fee: row.delivery_fee,
            product_id: row.product_id,
            product_name: row.product_name.clone(),
            unit_price: row.price,
            stock: row.stock,
            quantity,
        });
    }
    if !missing.is_empty() {
        return Err(CheckoutError::Unavailable {
            store_product_ids: missing,
        });
    }
    if !closed_stores.is_empty() {
        return Err(CheckoutError::StoreClosed {
            stores: closed_stores,
        });
    }

    // Group the lines by store, preserving first-seen order (BTreeMap over
    // store ids would reorder; the customer's grouping order is friendlier).
    let mut order_by_store: Vec<Uuid> = Vec::new();
    let mut per_store: BTreeMap<Uuid, Vec<&Line>> = BTreeMap::new();
    for line in &lines {
        if !per_store.contains_key(&line.store_id) {
            order_by_store.push(line.store_id);
        }
        per_store.entry(line.store_id).or_default().push(line);
    }

    // Stock reservation first — a conditional decrement that fails loudly
    // when the shelf is empty. Untracked rows (stock NULL) skip this.
    let mut shorts: Vec<StockShort> = Vec::new();
    for line in &lines {
        if let Some(stock) = line.stock
            && stock < i64::from(line.quantity)
        {
            shorts.push(StockShort {
                product_name: line.product_name.clone(),
                available: stock,
            });
        }
    }
    if !shorts.is_empty() {
        return Err(CheckoutError::InsufficientStock { items: shorts });
    }
    for line in &lines {
        if line.stock.is_some() {
            let result = sqlx::query!(
                r#"
                UPDATE marketplace.store_products
                SET stock = stock - $2
                WHERE id = $1 AND stock >= $2
                "#,
                line.store_product_id,
                i64::from(line.quantity),
            )
            .execute(&mut *conn)
            .await?;
            if result.rows_affected() == 0 {
                // Lost a race between the check and the decrement; the
                // transaction aborts, nothing was placed.
                return Err(CheckoutError::InsufficientStock {
                    items: vec![StockShort {
                        product_name: line.product_name.clone(),
                        available: 0,
                    }],
                });
            }
        }
    }

    let mut tx = conn.begin().await?;

    let group_subtotal: i64 = per_store
        .values()
        .map(|store_lines| {
            store_lines
                .iter()
                .map(|line| line.unit_price * i64::from(line.quantity))
                .sum::<i64>()
        })
        .sum();
    let delivery_total: i64 = per_store
        .values()
        .map(|store_lines| store_lines.first().map(|l| l.delivery_fee).unwrap_or(0))
        .sum();
    let grand_total = group_subtotal + delivery_total;

    let group = sqlx::query_as!(
        OrderGroup,
        r#"
        INSERT INTO commerce.order_groups
            (user_id, address_text, address_lat, address_lng, subtotal,
             delivery_total, grand_total, idempotency_key)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, number, address_text, address_lat, address_lng,
                  subtotal, delivery_total, grand_total, idempotency_key,
                  created_at, updated_at
        "#,
        user_id,
        checkout.address_text,
        checkout.address_lat,
        checkout.address_lng,
        group_subtotal,
        delivery_total,
        grand_total,
        checkout.idempotency_key,
    )
    .fetch_one(&mut *tx)
    .await?;

    let payment = sqlx::query_as!(
        Payment,
        r#"
        INSERT INTO commerce.payments (order_group_id, amount)
        VALUES ($1, $2)
        RETURNING id, order_group_id,
                  provider AS "provider: PaymentProvider", amount, currency,
                  status AS "status: PaymentStatus", provider_reference,
                  created_at, updated_at
        "#,
        group.id,
        grand_total,
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut store_orders: Vec<StoreOrderCreated> = Vec::with_capacity(per_store.len());
    for store_id in order_by_store {
        let store_lines = &per_store[&store_id];
        let first = store_lines[0];
        let store_subtotal: i64 = store_lines
            .iter()
            .map(|line| line.unit_price * i64::from(line.quantity))
            .sum();
        let store_total = store_subtotal + first.delivery_fee;

        let store_order = sqlx::query_as!(
            StoreOrder,
            r#"
            INSERT INTO commerce.store_orders
                (order_group_id, merchant_id, store_id, subtotal, delivery_fee, total)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, order_group_id, merchant_id, store_id, number,
                      status AS "status: OrderStatus", subtotal, delivery_fee, total,
                      created_at, updated_at
            "#,
            group.id,
            first.merchant_id,
            first.store_id,
            store_subtotal,
            first.delivery_fee,
            store_total,
        )
        .fetch_one(&mut *tx)
        .await?;

        let mut items: Vec<OrderItem> = Vec::with_capacity(store_lines.len());
        for line in store_lines {
            let item = sqlx::query_as!(
                OrderItem,
                r#"
                INSERT INTO commerce.order_items
                    (store_order_id, store_product_id, product_id,
                     product_name_snapshot, unit_price, quantity)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, store_order_id, store_product_id, product_id,
                          product_name_snapshot, unit_price, quantity
                "#,
                store_order.id,
                line.store_product_id,
                line.product_id,
                line.product_name,
                line.unit_price,
                line.quantity,
            )
            .fetch_one(&mut *tx)
            .await?;
            items.push(item);
        }

        sqlx::query!(
            r#"INSERT INTO commerce.deliveries (store_order_id) VALUES ($1)"#,
            store_order.id,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO commerce.payment_allocations
                (payment_id, store_order_id, merchant_id, store_id, amount)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            payment.id,
            store_order.id,
            first.merchant_id,
            first.store_id,
            store_total,
        )
        .execute(&mut *tx)
        .await?;

        store_orders.push(StoreOrderCreated {
            order: store_order,
            store_name: first.store_name.clone(),
            items,
        });
    }

    tx.commit().await?;
    Ok(CheckoutCreated {
        group,
        store_orders,
        payment,
    })
}

/// The customer's order groups, newest first, one page at a time. Statuses
/// ride along so the client (and [`derive_group_status`]) can show overall
/// state.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupSummaryRow {
    pub id: Uuid,
    pub number: i64,
    pub grand_total: i64,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupStoreStatusRow {
    pub order_group_id: Uuid,
    pub store_name: String,
    pub status: OrderStatus,
}

pub async fn group_summaries_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<(GroupSummaryRow, Vec<GroupStoreStatusRow>)>, sqlx::Error> {
    let groups = sqlx::query_as!(
        GroupSummaryRow,
        r#"
        SELECT id, number, grand_total, created_at
        FROM commerce.order_groups
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit,
        offset,
    )
    .fetch_all(&mut *conn)
    .await?;
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let group_ids: Vec<Uuid> = groups.iter().map(|group| group.id).collect();
    let statuses = sqlx::query!(
        r#"
        SELECT so.order_group_id, s.name AS store_name,
               so.status AS "status: OrderStatus"
        FROM commerce.store_orders so
        JOIN marketplace.stores s ON s.id = so.store_id
        WHERE so.order_group_id = ANY($1)
        ORDER BY so.created_at
        "#,
        &group_ids[..],
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut by_group: std::collections::HashMap<Uuid, Vec<GroupStoreStatusRow>> =
        std::collections::HashMap::new();
    for row in &statuses {
        by_group
            .entry(row.order_group_id)
            .or_default()
            .push(GroupStoreStatusRow {
                order_group_id: row.order_group_id,
                store_name: row.store_name.clone(),
                status: row.status,
            });
    }
    Ok(groups
        .into_iter()
        .map(|group| {
            let rows = by_group.remove(&group.id).unwrap_or_default();
            (group, rows)
        })
        .collect())
}

/// One of the customer's groups, fully loaded: store orders with their
/// item snapshots and the payment state. Another customer's group is
/// indistinguishable from a missing one.
pub struct GroupDetail {
    pub group: OrderGroup,
    pub store_orders: Vec<(StoreOrder, String, Vec<OrderItem>)>,
    pub payment: Payment,
}

pub async fn group_detail_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    group_id: Uuid,
) -> Result<Option<GroupDetail>, sqlx::Error> {
    let Some(group) = sqlx::query_as!(
        OrderGroup,
        r#"
        SELECT id, user_id, number, address_text, address_lat, address_lng,
               subtotal, delivery_total, grand_total, idempotency_key,
               created_at, updated_at
        FROM commerce.order_groups
        WHERE id = $1 AND user_id = $2
        "#,
        group_id,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };

    let payment = sqlx::query_as!(
        Payment,
        r#"
        SELECT id, order_group_id, provider AS "provider: PaymentProvider",
               amount, currency, status AS "status: PaymentStatus",
               provider_reference, created_at, updated_at
        FROM commerce.payments
        WHERE order_group_id = $1
        "#,
        group.id,
    )
    .fetch_one(&mut *conn)
    .await?;

    let orders = sqlx::query!(
        r#"
        SELECT so.id, so.order_group_id, so.merchant_id, so.store_id, so.number,
               so.status AS "status: OrderStatus", so.subtotal, so.delivery_fee,
               so.total, so.created_at, so.updated_at,
               s.name AS store_name
        FROM commerce.store_orders so
        JOIN marketplace.stores s ON s.id = so.store_id
        WHERE so.order_group_id = $1
        ORDER BY so.created_at
        "#,
        group.id,
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut store_orders = Vec::with_capacity(orders.len());
    for order in orders {
        let items = items_for_store_order(&mut *conn, order.id).await?;
        store_orders.push((
            StoreOrder {
                id: order.id,
                order_group_id: order.order_group_id,
                merchant_id: order.merchant_id,
                store_id: order.store_id,
                number: order.number,
                status: order.status,
                subtotal: order.subtotal,
                delivery_fee: order.delivery_fee,
                total: order.total,
                created_at: order.created_at,
                updated_at: order.updated_at,
            },
            order.store_name,
            items,
        ));
    }
    Ok(Some(GroupDetail {
        group,
        store_orders,
        payment,
    }))
}

/// The items of a store order whose access the caller already established.
pub async fn items_for_store_order(
    conn: &mut PgConnection,
    store_order_id: Uuid,
) -> Result<Vec<OrderItem>, sqlx::Error> {
    sqlx::query_as!(
        OrderItem,
        r#"
        SELECT id, store_order_id, store_product_id, product_id,
               product_name_snapshot, unit_price, quantity
        FROM commerce.order_items
        WHERE store_order_id = $1
        ORDER BY id
        "#,
        store_order_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Which business and store a store order belongs to — the fact ownership
/// checks need, before any mutation.
pub async fn store_order_scope(
    conn: &mut PgConnection,
    store_order_id: Uuid,
) -> Result<Option<(Uuid, Uuid, Uuid)>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT so.id, so.merchant_id, so.store_id, so.order_group_id
        FROM commerce.store_orders so
        WHERE so.id = $1
        "#,
        store_order_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|row| (row.id, row.merchant_id, row.store_id)))
}

/// Advance a store order along the six-value machine. The caller has
/// already established ownership; this function owns the legality of the
/// transition itself.
pub async fn advance_store_order_status(
    conn: &mut PgConnection,
    store_order_id: Uuid,
    next: OrderStatus,
) -> Result<StoreOrder, TransitionError> {
    let mut tx = conn.begin().await?;
    let current = sqlx::query!(
        r#"
        SELECT status AS "status: OrderStatus" FROM commerce.store_orders
        WHERE id = $1
        "#,
        store_order_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TransitionError::OrderNotFound)?;

    if !current.status.can_transition_to(next) {
        return Err(TransitionError::Illegal {
            from: current.status.label(),
            to: next.label(),
        });
    }
    // `picked_up` is the handoff's event, never a bare advance: the
    // transition without a rider attached would strand the delivery —
    // nobody could push locations or mark it delivered. The handoff
    // endpoint (which sets rider_id, stamps handoff_at, caches the route)
    // is the ONLY door into `picked_up`; re-assignment reuses it.
    if next == OrderStatus::PickedUp {
        return Err(TransitionError::HandoffRequired);
    }

    let updated = sqlx::query_as!(
        StoreOrder,
        r#"
        UPDATE commerce.store_orders SET status = $2
        WHERE id = $1
        RETURNING id, order_group_id, merchant_id, store_id, number,
                  status AS "status: OrderStatus", subtotal, delivery_fee, total,
                  created_at, updated_at
        "#,
        store_order_id,
        next as OrderStatus,
    )
    .fetch_one(&mut *tx)
    .await?;
    // The cash state (tracking doc §5): delivery = payment for cash-on-
    // delivery, whichever real actor drives the advance — the rider's
    // Delivered action and the merchant's PATCH are the same event to the
    // ledger. Inside this transaction so status and money move atomically.
    if next == OrderStatus::Delivered {
        crate::deliveries::settle_delivery_cash(&mut tx, store_order_id).await?;
    }
    tx.commit().await?;
    Ok(updated)
}

/// The customer cancels one store order of their own group — allowed while
/// the order is still on the premises (placed, accepted, preparing).
#[derive(Debug, thiserror::Error)]
pub enum CancelError {
    #[error("order not found")]
    NotFound,
    #[error(transparent)]
    Illegal(#[from] TransitionError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn cancel_own_store_order(
    conn: &mut PgConnection,
    user_id: Uuid,
    group_id: Uuid,
    store_order_id: Uuid,
) -> Result<StoreOrder, CancelError> {
    // The group must belong to the customer, and the store order must sit
    // inside it — a foreign id is a plain NotFound either way.
    let owned = sqlx::query!(
        r#"
        SELECT so.id
        FROM commerce.store_orders so
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        WHERE so.id = $1 AND og.id = $2 AND og.user_id = $3
        "#,
        store_order_id,
        group_id,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(CancelError::NotFound)?;

    advance_store_order_status(&mut *conn, owned.id, OrderStatus::Cancelled)
        .await
        .map_err(CancelError::Illegal)
}

/// Store orders still moving — the platform's live-ops count (summary).
pub async fn count_in_progress(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM commerce.store_orders
        WHERE status NOT IN ('delivered', 'cancelled')
        "#,
    )
    .fetch_one(&mut *conn)
    .await
}

/// A merchant operator's incoming store orders, newest first, one page.
/// Scoped like every merchant read: per-merchant, optionally per-store.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MerchantStoreOrderRow {
    pub id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub number: i64,
    pub status: OrderStatus,
    pub total: i64,
    pub address_text: String,
    pub created_at: OffsetDateTime,
}

pub async fn store_orders_for_merchant_scoped(
    conn: &mut PgConnection,
    merchant_id: Uuid,
    store_ids: Option<&[Uuid]>,
    limit: i64,
    offset: i64,
) -> Result<Vec<MerchantStoreOrderRow>, sqlx::Error> {
    sqlx::query_as!(
        MerchantStoreOrderRow,
        r#"
        SELECT so.id, so.store_id, s.name AS store_name, so.number,
               so.status AS "status: OrderStatus", so.total,
               og.address_text, so.created_at
        FROM commerce.store_orders so
        JOIN marketplace.stores s ON s.id = so.store_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        WHERE s.merchant_id = $1
          AND ($2::uuid[] IS NULL OR cardinality($2::uuid[]) = 0 OR so.store_id = ANY($2::uuid[]))
        ORDER BY so.created_at DESC
        LIMIT $3 OFFSET $4
        "#,
        merchant_id,
        store_ids,
        limit,
        offset,
    )
    .fetch_all(&mut *conn)
    .await
}

/// The merchant's view of one store order: what to prepare, where to
/// deliver it, and who to contact — the customer's profile name and the
/// account's phone (the delivery contact; local reality is that riders and
/// merchants call). Ownership is the handler's job via membership grants.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoreOrderDetail {
    pub id: Uuid,
    pub order_group_id: Uuid,
    pub merchant_id: Uuid,
    pub store_id: Uuid,
    pub store_name: String,
    pub number: i64,
    pub status: OrderStatus,
    pub subtotal: i64,
    pub delivery_fee: i64,
    pub total: i64,
    pub address_text: String,
    pub address_lat: Option<f64>,
    pub address_lng: Option<f64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub payment_status: PaymentStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Load one store order's full detail. `None` = unknown id; the handler
/// layers the membership grant on top.
pub async fn store_order_detail(
    conn: &mut PgConnection,
    store_order_id: Uuid,
) -> Result<Option<StoreOrderDetail>, sqlx::Error> {
    sqlx::query_as!(
        StoreOrderDetail,
        r#"
        SELECT so.id, so.order_group_id, so.merchant_id, so.store_id,
               s.name AS store_name, so.number,
               so.status AS "status: OrderStatus", so.subtotal, so.delivery_fee,
               so.total, og.address_text, og.address_lat, og.address_lng,
               c.name AS customer_name, u.phone AS customer_phone,
               p.status AS "payment_status: PaymentStatus",
               so.created_at, so.updated_at
        FROM commerce.store_orders so
        JOIN marketplace.stores s ON s.id = so.store_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        LEFT JOIN accounts.customers c ON c.user_id = og.user_id
        LEFT JOIN accounts.users u ON u.id = og.user_id
        JOIN commerce.payments p ON p.order_group_id = og.id
        WHERE so.id = $1
        "#,
        store_order_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The payment of one group, for rendering the customer's group detail.
pub async fn payment_for_group(
    conn: &mut PgConnection,
    group_id: Uuid,
) -> Result<Option<Payment>, sqlx::Error> {
    sqlx::query_as!(
        Payment,
        r#"
        SELECT id, order_group_id, provider AS "provider: PaymentProvider",
               amount, currency, status AS "status: PaymentStatus",
               provider_reference, created_at, updated_at
        FROM commerce.payments
        WHERE order_group_id = $1
        "#,
        group_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn group_by_idempotency_key(
    conn: &mut PgConnection,
    user_id: Uuid,
    key: &str,
) -> Result<Option<OrderGroup>, sqlx::Error> {
    sqlx::query_as!(
        OrderGroup,
        r#"
        SELECT id, user_id, number, address_text, address_lat, address_lng,
               subtotal, delivery_total, grand_total, idempotency_key,
               created_at, updated_at
        FROM commerce.order_groups
        WHERE user_id = $1 AND idempotency_key = $2
        "#,
        user_id,
        key,
    )
    .fetch_optional(&mut *conn)
    .await
}
