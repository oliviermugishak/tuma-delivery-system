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
/// flow). Low-level insert — go through [`crate::AccountManager`], which
/// resolves duplicate identities into typed errors.
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
/// provisioned by the platform — customers never come through here. Phone
/// numbers are customer identity only, so staff accounts never carry one.
/// Low-level insert taking a ready hash — go through
/// [`crate::AccountManager`], which hashes and resolves duplicate identities.
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

pub async fn set_password(
    conn: &mut PgConnection,
    id: Uuid,
    password_hash: &str,
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET password_hash = $2
        WHERE id = $1
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        password_hash,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Toggle a merchant's active flag. Merchants only — `None` means the id
/// doesn't exist or belongs to another role, and handlers say "not found"
/// either way (this path must never toggle an admin or a customer).
pub async fn set_merchant_active(
    conn: &mut PgConnection,
    id: Uuid,
    is_active: bool,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET is_active = $2
        WHERE id = $1 AND role = 'merchant'
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        is_active,
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn list_merchants(conn: &mut PgConnection) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: UserRole", name, phone, email, password_hash,
               is_active, created_at, updated_at
        FROM tuma.users
        WHERE role = 'merchant'
        ORDER BY created_at
        "#,
    )
    .fetch_all(&mut *conn)
    .await
}

pub async fn list_customers(conn: &mut PgConnection) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: UserRole", name, phone, email, password_hash,
               is_active, created_at, updated_at
        FROM tuma.users
        WHERE role = 'customer'
        ORDER BY created_at
        "#,
    )
    .fetch_all(&mut *conn)
    .await
}

/// Toggle a customer's active flag. Customers only — `None` means the id
/// doesn't exist or belongs to another role, and handlers say "not found"
/// either way (this path must never toggle an admin or a merchant).
pub async fn set_customer_active(
    conn: &mut PgConnection,
    id: Uuid,
    is_active: bool,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET is_active = $2
        WHERE id = $1 AND role = 'customer'
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        is_active,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Overwrite a merchant's name and email with ready final values — the
/// caller resolves "absent keeps its current value" against the row first.
/// A taken email resolves to the typed conflict via the constraint name.
pub async fn update_merchant(
    conn: &mut PgConnection,
    id: Uuid,
    name: Option<&str>,
    email: &str,
) -> Result<User, crate::CreateAccountError> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET name = $2, email = $3
        WHERE id = $1 AND role = 'merchant'
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        name,
        email,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(crate::manager::unique_violation)
}

/// Overwrite a customer's name and phone with ready final values — the
/// caller resolves "absent keeps its current value" against the row first.
/// A taken phone resolves to the typed conflict via the constraint name.
pub async fn update_customer(
    conn: &mut PgConnection,
    id: Uuid,
    name: Option<&str>,
    phone: &str,
) -> Result<User, crate::CreateAccountError> {
    sqlx::query_as!(
        User,
        r#"
        UPDATE tuma.users SET name = $2, phone = $3
        WHERE id = $1 AND role = 'customer'
        RETURNING id, role as "role: UserRole", name, phone, email, password_hash,
                  is_active, created_at, updated_at
        "#,
        id,
        name,
        phone,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(crate::manager::unique_violation)
}

/// Hard-delete a user, scoped by role: the row only disappears if it exists
/// AND holds that role. Returns whether anything was deleted. Stores,
/// products and refresh tokens follow via ON DELETE CASCADE.
pub async fn delete_user(
    conn: &mut PgConnection,
    id: Uuid,
    role: UserRole,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM tuma.users
        WHERE id = $1 AND role = $2
        "#,
        id,
        role as UserRole,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// How many users hold a role (admin summary).
pub async fn count_by_role(conn: &mut PgConnection, role: UserRole) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM tuma.users
        WHERE role = $1
        "#,
        role as UserRole
    )
    .fetch_one(&mut *conn)
    .await
}
