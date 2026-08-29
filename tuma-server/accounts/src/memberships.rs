//! Merchant memberships: how an account is authorized to act for a
//! merchant business. An owner covers the whole business; a manager is
//! either merchant-wide (`store_id` NULL) or scoped to one store.

use super::merchants::MerchantStatus;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "marketplace.membership_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MembershipRole {
    Owner,
    Manager,
}

impl MembershipRole {
    pub fn label(self) -> &'static str {
        match self {
            MembershipRole::Owner => "owner",
            MembershipRole::Manager => "manager",
        }
    }
}

/// A row of `marketplace.merchant_memberships`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Membership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub merchant_id: Uuid,
    pub role: MembershipRole,
    pub store_id: Option<Uuid>,
    pub status: MerchantStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// A membership joined with its business — what `/me` and the merchant wing
/// need to render and to scope. `merchant_status` rides along so a suspended
/// business can be refused without a second lookup.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipView {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub merchant_name: String,
    pub merchant_status: MerchantStatus,
    pub role: MembershipRole,
    pub store_id: Option<Uuid>,
}

/// Attach an account to a business. The admin's merchant-creation flow
/// grants the owner membership; staff invitations (a later slice) grant
/// managers.
pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    merchant_id: Uuid,
    role: MembershipRole,
    store_id: Option<Uuid>,
) -> Result<Membership, sqlx::Error> {
    sqlx::query_as!(
        Membership,
        r#"
        INSERT INTO marketplace.merchant_memberships (user_id, merchant_id, role, store_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, merchant_id, role AS "role: MembershipRole",
                  store_id, status AS "status: MerchantStatus",
                  created_at, updated_at
        "#,
        user_id,
        merchant_id,
        role as MembershipRole,
        store_id,
    )
    .fetch_one(&mut *conn)
    .await
}

/// The account's memberships joined with their businesses, oldest first.
pub async fn views_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<MembershipView>, sqlx::Error> {
    sqlx::query_as!(
        MembershipView,
        r#"
        SELECT m.id, m.merchant_id, b.name AS merchant_name,
               b.status AS "merchant_status: MerchantStatus",
               m.role AS "role: MembershipRole", m.store_id
        FROM marketplace.merchant_memberships m
        JOIN marketplace.merchants b ON b.id = m.merchant_id
        WHERE m.user_id = $1
        ORDER BY m.created_at
        "#,
        user_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// All memberships of one business (admin detail).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipListRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub role: MembershipRole,
    pub store_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

pub async fn list_for_merchant(
    conn: &mut PgConnection,
    merchant_id: Uuid,
) -> Result<Vec<MembershipListRow>, sqlx::Error> {
    sqlx::query_as!(
        MembershipListRow,
        r#"
        SELECT m.id, m.user_id, u.email,
               m.role AS "role: MembershipRole", m.store_id, m.created_at
        FROM marketplace.merchant_memberships m
        JOIN accounts.users u ON u.id = m.user_id
        WHERE m.merchant_id = $1
        ORDER BY m.created_at
        "#,
        merchant_id,
    )
    .fetch_all(&mut *conn)
    .await
}
