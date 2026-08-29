//! Platform-admin profiles: authorization to run the platform. An admin
//! authenticates like anyone else (email + password); the profile row is
//! what makes them an admin. Created only by `seed_admin`.

use sqlx::PgConnection;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of `accounts.admins`. One per account, UNIQUE.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AdminProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Fetch the account's admin profile, creating it if absent — `seed_admin`
/// provisions the first platform admin this way.
pub async fn ensure_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: Option<&str>,
) -> Result<AdminProfile, sqlx::Error> {
    sqlx::query_as!(
        AdminProfile,
        r#"
        INSERT INTO accounts.admins (user_id, name)
        VALUES ($1, $2)
        ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
        RETURNING id, user_id, name, created_at, updated_at
        "#,
        user_id,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn by_user_id(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<AdminProfile>, sqlx::Error> {
    sqlx::query_as!(
        AdminProfile,
        r#"
        SELECT id, user_id, name, created_at, updated_at
        FROM accounts.admins
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
) -> Result<AdminProfile, sqlx::Error> {
    sqlx::query_as!(
        AdminProfile,
        r#"
        UPDATE accounts.admins SET name = $2
        WHERE user_id = $1
        RETURNING id, user_id, name, created_at, updated_at
        "#,
        user_id,
        name,
    )
    .fetch_one(&mut *conn)
    .await
}
