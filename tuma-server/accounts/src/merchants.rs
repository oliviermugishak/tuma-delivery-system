//! Merchants: the business entity. A merchant is never a login — people
//! act for a business through merchant memberships (see
//! [`crate::memberships`]). Stores, catalog products, and memberships all
//! cascade from this row.

use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// The PostgreSQL enum and the JSON wire value share the same snake_case
/// labels, so there is one spelling across the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "marketplace.merchant_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MerchantStatus {
    Active,
    Suspended,
}

impl MerchantStatus {
    pub fn label(self) -> &'static str {
        match self {
            MerchantStatus::Active => "active",
            MerchantStatus::Suspended => "suspended",
        }
    }
}

/// A row of `marketplace.merchants`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Merchant {
    pub id: Uuid,
    pub name: String,
    pub business_email: Option<String>,
    pub business_phone: Option<String>,
    pub status: MerchantStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Create the business. Admin-provisioned only — merchants never self-signup.
pub async fn create(
    conn: &mut PgConnection,
    name: &str,
    business_email: Option<&str>,
    business_phone: Option<&str>,
) -> Result<Merchant, sqlx::Error> {
    sqlx::query_as!(
        Merchant,
        r#"
        INSERT INTO marketplace.merchants (name, business_email, business_phone)
        VALUES ($1, $2, $3)
        RETURNING id, name, business_email, business_phone,
                  status AS "status: MerchantStatus", created_at, updated_at
        "#,
        name,
        business_email,
        business_phone,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Merchant>, sqlx::Error> {
    sqlx::query_as!(
        Merchant,
        r#"
        SELECT id, name, business_email, business_phone,
               status AS "status: MerchantStatus", created_at, updated_at
        FROM marketplace.merchants
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// All merchant businesses, oldest first (admin list).
pub async fn list(conn: &mut PgConnection) -> Result<Vec<Merchant>, sqlx::Error> {
    sqlx::query_as!(
        Merchant,
        r#"
        SELECT id, name, business_email, business_phone,
               status AS "status: MerchantStatus", created_at, updated_at
        FROM marketplace.merchants
        ORDER BY created_at
        "#,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Overwrite a business's editable fields with ready final values — the
/// caller resolves "absent keeps its current value" against the row first.
pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    business_email: Option<&str>,
    business_phone: Option<&str>,
    status: MerchantStatus,
) -> Result<Option<Merchant>, sqlx::Error> {
    sqlx::query_as!(
        Merchant,
        r#"
        UPDATE marketplace.merchants
        SET name = $2, business_email = $3, business_phone = $4, status = $5
        WHERE id = $1
        RETURNING id, name, business_email, business_phone,
                  status AS "status: MerchantStatus", created_at, updated_at
        "#,
        id,
        name,
        business_email,
        business_phone,
        status as MerchantStatus,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Hard-delete the business. Memberships, stores (and their store_products),
/// and catalog products follow via ON DELETE CASCADE. Member ACCOUNTS are
/// not deleted — they are identities, not parts of the business.
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM marketplace.merchants
        WHERE id = $1
        "#,
        id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// How many businesses exist (admin summary).
pub async fn count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM marketplace.merchants
        "#,
    )
    .fetch_one(&mut *conn)
    .await
}
