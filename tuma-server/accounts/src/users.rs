use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// The three V1 roles. Adding one is an enum value here plus a value in the
/// Postgres type `tuma.user_role` (see Tuma_Auth_and_RBAC_Architecture.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "tuma.user_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Customer,
    Merchant,
    Admin,
}

/// A row of `tuma.users`. `password_hash` lives here so the domain can check
/// it; HTTP layers must never serialize it (responses use their own types).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub role: UserRole,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Create a customer account. Customers are identified by phone; the row
/// appears the first time an OTP is verified (register and login are one
/// flow). A duplicate phone surfaces as `sqlx::Error::Database` (unique
/// violation) for the caller to resolve.
pub async fn create_customer(
    conn: &mut PgConnection,
    phone: &str,
    name: Option<&str>,
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        INSERT INTO tuma.users (role, phone, name)
        VALUES ($1, $2, $3)
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        UserRole::Customer as UserRole,
        phone,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Create a merchant or admin account (email + password identity). Both are
/// provisioned by the platform — customers never come through here.
pub async fn create_staff(
    conn: &mut PgConnection,
    role: UserRole,
    name: Option<&str>,
    email: &str,
    password_hash: &str,
) -> Result<User, sqlx::Error> {
    debug_assert!(
        role == UserRole::Merchant || role == UserRole::Admin,
        "create_staff is only for merchant/admin roles"
    );
    sqlx::query_as!(
        User,
        r#"
        INSERT INTO tuma.users (role, name, email, password_hash)
        VALUES ($1, $2, $3, $4)
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        role as UserRole,
        name,
        email,
        password_hash,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: UserRole", name, phone, email, password_hash,
               is_active, created_at, updated_at
        FROM tuma.users
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn by_phone(conn: &mut PgConnection, phone: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: UserRole", name, phone, email, password_hash,
               is_active, created_at, updated_at
        FROM tuma.users
        WHERE phone = $1
        "#,
        phone
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn by_email(conn: &mut PgConnection, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: UserRole", name, phone, email, password_hash,
               is_active, created_at, updated_at
        FROM tuma.users
        WHERE email = $1
        "#,
        email
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn set_name(conn: &mut PgConnection, id: Uuid, name: &str) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET name = $2
        WHERE id = $1
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}
