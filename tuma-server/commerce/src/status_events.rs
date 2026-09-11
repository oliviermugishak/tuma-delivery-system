//! Order status events — the system's memory. Every status transition
//! appends one row inside the same transaction as the change itself: what
//! happened, when, by whom. The table is append-only (no UPDATE, no
//! DELETE, no updated_at — the `delivery_locations` precedent), so history
//! can never be rewritten, only extended.

use sqlx::PgExecutor;
use uuid::Uuid;

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
