//! Customer profiles: the business entity behind a customer account.
//! The account authenticates (phone + OTP); the profile carries the
//! customer-facing data (display name).

use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `accounts.customers`. One per account, UNIQUE.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Customer {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Fetch the account's customer profile, creating it if absent — the OTP
/// flow's register-and-login-are-one guarantee.
pub async fn ensure_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        INSERT INTO accounts.customers (user_id)
        VALUES ($1)
        ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
        RETURNING id, user_id, name, created_at, updated_at
        "#,
        user_id,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn by_user_id(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<Customer>, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        SELECT id, user_id, name, created_at, updated_at
        FROM accounts.customers
        WHERE user_id = $1
        "#,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn set_name(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: &str,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        UPDATE accounts.customers SET name = $2
        WHERE user_id = $1
        RETURNING id, user_id, name, created_at, updated_at
        "#,
        user_id,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}

/// One row of the admin's customers list: the account facts (phone, active
/// flag) joined with the profile (name), oldest account first.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomerListRow {
    pub user_id: Uuid,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
}

/// Customer accounts, oldest first, one page at a time (admin list).
pub async fn list_customers(
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<Vec<CustomerListRow>, sqlx::Error> {
    sqlx::query_as!(
        CustomerListRow,
        r#"
        SELECT c.user_id, c.name, u.phone, u.is_active, u.created_at
        FROM accounts.customers c
        JOIN accounts.users u ON u.id = c.user_id
        ORDER BY u.created_at
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset,
    )
    .fetch_all(&mut *conn)
    .await
}

/// The admin-list row for one account. `None` when the account has no
/// customer profile — which is how admin edits stay scoped to customers.
pub async fn list_row_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<CustomerListRow>, sqlx::Error> {
    sqlx::query_as!(
        CustomerListRow,
        r#"
        SELECT c.user_id, c.name, u.phone, u.is_active, u.created_at
        FROM accounts.customers c
        JOIN accounts.users u ON u.id = c.user_id
        WHERE c.user_id = $1
        "#,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Overwrite the profile name. `None` clears it (an empty-name edit from
/// the admin means "remove the name"), `Some` writes the ready value.
pub async fn update_name(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: Option<&str>,
) -> Result<Customer, sqlx::Error> {
    sqlx::query_as!(
        Customer,
        r#"
        UPDATE accounts.customers SET name = $2
        WHERE user_id = $1
        RETURNING id, user_id, name, created_at, updated_at
        "#,
        user_id,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Hard-delete the account, but only if it is a customer: the delete is
/// scoped by the profile's existence, so an admin's or operator's id can
/// never disappear through the customer endpoint. Profile, refresh tokens,
/// and order history follow via ON DELETE CASCADE.
pub async fn delete_customer_account(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM accounts.users u
        USING accounts.customers c
        WHERE u.id = c.user_id AND c.user_id = $1
        "#,
        user_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// How many customer profiles exist (admin summary).
pub async fn count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM accounts.customers
        "#,
    )
    .fetch_one(&mut *conn)
    .await
}
