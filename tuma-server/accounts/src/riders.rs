//! Riders: Tuma-owned delivery drivers. A rider is an account (phone + OTP,
//! admin-created — never self-signup) plus this profile row carrying the
//! unique rider number the merchant types at handoff. The table lives in
//! the commerce schema beside the deliveries it serves; this crate owns it
//! because rider identity resolves through [`crate::authorization_for`],
//! exactly like the customer and admin profiles.

use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RiderError {
    #[error("rider not found")]
    NotFound,
    #[error("this rider has delivery history — deactivate instead")]
    HasDeliveries,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A row of `commerce.riders`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Rider {
    pub id: Uuid,
    pub account_id: Uuid,
    pub rider_number: i64,
    pub name: String,
    pub phone: String,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Create the rider profile for an account that already exists. The rider
/// number comes from the identity column — never supplied, never edited.
pub async fn create(
    conn: &mut PgConnection,
    account_id: Uuid,
    name: &str,
    phone: &str,
) -> Result<Rider, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        INSERT INTO commerce.riders (account_id, name, phone)
        VALUES ($1, $2, $3)
        RETURNING id, account_id, rider_number, name, phone, is_active,
                  created_at, updated_at
        "#,
        account_id,
        name,
        phone,
    )
    .fetch_one(&mut *conn)
    .await
}

/// The authorization lookup: the account's rider profile, if it is one.
pub async fn by_user_id(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<Rider>, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        SELECT id, account_id, rider_number, name, phone, is_active,
               created_at, updated_at
        FROM commerce.riders
        WHERE account_id = $1
        "#,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Rider>, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        SELECT id, account_id, rider_number, name, phone, is_active,
               created_at, updated_at
        FROM commerce.riders
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The rider a merchant is handing an order to, by the number they typed —
/// active riders only: the number is the whole assignment interface, and a
/// deactivated rider is not assignable. Unknown and inactive are the same
/// answer ("no active rider with that number").
pub async fn active_by_number(
    conn: &mut PgConnection,
    rider_number: i64,
) -> Result<Option<Rider>, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        SELECT id, account_id, rider_number, name, phone, is_active,
               created_at, updated_at
        FROM commerce.riders
        WHERE rider_number = $1 AND is_active
        "#,
        rider_number,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// All riders, oldest first (admin list — the merchant needs to read the
/// numbers to enter them at handoff).
pub async fn list(conn: &mut PgConnection) -> Result<Vec<Rider>, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        SELECT id, account_id, rider_number, name, phone, is_active,
               created_at, updated_at
        FROM commerce.riders
        ORDER BY created_at
        "#,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Overwrite the rider's editable fields with ready final values — the
/// caller resolves "absent keeps its current value" against the row first.
pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    phone: &str,
    is_active: bool,
) -> Result<Option<Rider>, sqlx::Error> {
    sqlx::query_as!(
        Rider,
        r#"
        UPDATE commerce.riders
        SET name = $2, phone = $3, is_active = $4
        WHERE id = $1
        RETURNING id, account_id, rider_number, name, phone, is_active,
                  created_at, updated_at
        "#,
        id,
        name,
        phone,
        is_active,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Hard-delete the profile row; the account delete cascades from it. Only
/// allowed while the rider has never been assigned a delivery — assignment
/// history is operationally real, so the check happens in [`delete_account`].
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM commerce.riders
        WHERE id = $1
        "#,
        id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Hard-delete a rider's account (the profile cascades) — the remedy for an
/// admin-typo'd phone. Scoped by the profile's existence so only rider
/// accounts can disappear through it, and blocked when any delivery ever
/// referenced the rider (assignment history must survive).
pub async fn delete_account(
    conn: &mut PgConnection,
    account_id: Uuid,
) -> Result<Result<bool, RiderError>, sqlx::Error> {
    let (referenced,): (bool,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM commerce.deliveries d
            JOIN commerce.riders r ON r.id = d.rider_id
            WHERE r.account_id = $1
        )
        "#,
    )
    .bind(account_id)
    .fetch_one(&mut *conn)
    .await?;
    if referenced {
        return Ok(Err(RiderError::HasDeliveries));
    }
    let result = sqlx::query!(
        r#"
        DELETE FROM accounts.users u
        USING commerce.riders r
        WHERE u.id = r.account_id AND r.account_id = $1
        "#,
        account_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(Ok(result.rows_affected() == 1))
}
