//! Order status events — the system's memory. Every status transition
//! appends one row inside the same transaction as the change itself: what
//! happened, when, by whom. The table is append-only (no UPDATE, no
//! DELETE, no updated_at — the `delivery_locations` precedent), so history
//! can never be rewritten, only extended.

use sqlx::PgExecutor;
use uuid::Uuid;

use crate::orders::OrderStatus;

/// Who performed a transition. `customer` and `merchant` events carry
/// `accounts.users` ids; `rider` events carry `commerce.riders` ids (the
/// rider profile).
#[derive(Debug, Clone, Copy, sqlx::Type)]
#[sqlx(type_name = "commerce.actor_kind", rename_all = "snake_case")]
pub enum ActorKind {
    Customer,
    Merchant,
    Rider,
}

impl ActorKind {
    /// The canonical snake_case label — the PostgreSQL enum value, the
    /// JSON wire value, and this string are one spelling.
    pub fn label(self) -> &'static str {
        match self {
            ActorKind::Customer => "customer",
            ActorKind::Merchant => "merchant",
            ActorKind::Rider => "rider",
        }
    }
}

/// One memory row of the append-only trail, as the readers carry it: what
/// happened, by whom, and when (the history's ordering fact).
#[derive(Debug, Clone, Copy)]
pub struct StoredEvent {
    pub status: OrderStatus,
    pub actor_kind: ActorKind,
    pub created_at: time::OffsetDateTime,
}

/// The actor behind a transition: their kind and their id (`accounts.users`
/// for customer/merchant, `commerce.riders` for rider).
#[derive(Debug, Clone, Copy)]
pub struct Actor {
    pub kind: ActorKind,
    pub id: Uuid,
}

/// Append one status event for `store_order_id`. MUST run inside the same
/// transaction as the status change itself (callers pass `&mut *tx`): the
/// event and the transition commit or roll back together — a status move
/// without its memory is a lie.
pub async fn record_status_event(
    executor: impl PgExecutor<'_>,
    store_order_id: Uuid,
    status: crate::orders::OrderStatus,
    actor: Actor,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO commerce.order_status_events (store_order_id, status, actor_kind, actor_id)
        VALUES ($1, $2, $3, $4)
        "#,
        store_order_id,
        status as crate::orders::OrderStatus,
        actor.kind as ActorKind,
        actor.id,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// The reader side of the append-only trail: every event recorded for the
/// named store orders, flattened as `(store_order_id, event)`. The
/// ordering (created_at, then id as the tiebreaker) IS the history's
/// order — readers join these back by id and keep it. Orders created
/// before migration 15 simply have no rows, and honestly come back with
/// none.
pub async fn for_store_orders(
    conn: &mut sqlx::PgConnection,
    store_order_ids: &[Uuid],
) -> sqlx::Result<Vec<(Uuid, StoredEvent)>> {
    sqlx::query!(
        r#"
        SELECT store_order_id,
               status AS "status: OrderStatus",
               actor_kind AS "actor_kind: ActorKind",
               created_at
        FROM commerce.order_status_events
        WHERE store_order_id = ANY($1)
        ORDER BY created_at, id
        "#,
        store_order_ids,
    )
    .fetch_all(&mut *conn)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|row| {
                (
                    row.store_order_id,
                    StoredEvent {
                        status: row.status,
                        actor_kind: row.actor_kind,
                        created_at: row.created_at,
                    },
                )
            })
            .collect()
    })
}
