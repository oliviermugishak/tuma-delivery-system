use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `accounts.users` — the central ACCOUNT. It authenticates; it is
/// never a business role. Customer and admin profiles, and merchant
/// memberships, are separate rows. `password_hash` lives here so the
/// domain can check it; HTTP layers must never serialize it (responses
/// use their own types).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub is_active: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Create a phone-anchored account (the customer OTP path). The customer
/// profile is ensured separately — see [`crate::customers::ensure_for_user`].
/// Low-level insert — go through [`crate::AccountManager`], which resolves
/// duplicate identities into typed errors.
pub async fn create_with_phone(
    conn: &mut PgConnection,
    phone: &str,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        INSERT INTO accounts.users (phone)
        VALUES ($1)
        RETURNING id, phone, email, password_hash, is_active, created_at, updated_at
        "#,
        phone,
    )
    .fetch_one(&mut *conn)
    .await
}

/// Create an email + password account (merchant staff and admins). Low-level
/// insert taking a ready hash — go through [`crate::AccountManager`], which
/// hashes and resolves duplicate identities.
pub async fn create_with_password(
    conn: &mut PgConnection,
    email: &str,
    password_hash: &str,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        INSERT INTO accounts.users (email, password_hash)
        VALUES ($1, $2)
        RETURNING id, phone, email, password_hash, is_active, created_at, updated_at
        "#,
        email,
        password_hash,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Account>, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        SELECT id, phone, email, password_hash, is_active, created_at, updated_at
        FROM accounts.users
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn by_phone(
    conn: &mut PgConnection,
    phone: &str,
) -> Result<Option<Account>, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        SELECT id, phone, email, password_hash, is_active, created_at, updated_at
        FROM accounts.users
        WHERE phone = $1
        "#,
        phone
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<Account>, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        SELECT id, phone, email, password_hash, is_active, created_at, updated_at
        FROM accounts.users
        WHERE email = $1
        "#,
        email
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn set_password(
    conn: &mut PgConnection,
    id: Uuid,
    password_hash: &str,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        UPDATE accounts.users SET password_hash = $2
        WHERE id = $1
        RETURNING id, phone, email, password_hash, is_active, created_at, updated_at
        "#,
        id,
        password_hash,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn set_active(
    conn: &mut PgConnection,
    id: Uuid,
    is_active: bool,
) -> Result<Option<Account>, sqlx::Error> {
    sqlx::query_as!(
        Account,
        r#"
        UPDATE accounts.users SET is_active = $2
        WHERE id = $1
        RETURNING id, phone, email, password_hash, is_active, created_at, updated_at
        "#,
        id,
        is_active,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Overwrite an account's phone with a ready final value — the caller
/// resolves "absent keeps its current value" against the row first. A taken
/// phone resolves to the typed conflict via the constraint name.
pub async fn update_phone(
    conn: &mut PgConnection,
    id: Uuid,
    phone: &str,
) -> Result<Account, crate::CreateAccountError> {
    sqlx::query_as!(
        Account,
        r#"
        UPDATE accounts.users SET phone = $2
        WHERE id = $1
        RETURNING id, phone, email, password_hash, is_active, created_at, updated_at
        "#,
        id,
        phone,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(crate::manager::unique_violation)
}

/// Hard-delete an account. The customer/admin profile and refresh tokens
/// follow via ON DELETE CASCADE; order groups too (03_commerce.sql).
/// Callers scope the deletion to the audience they manage — an admin id
/// must never disappear through the customer endpoint.
pub async fn delete_account(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM accounts.users
        WHERE id = $1
        "#,
        id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}
