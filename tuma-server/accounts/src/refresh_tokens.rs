//! Opaque refresh tokens for web sessions.
//!
//! The browser holds two cookies: a short-lived JWT (access) and a
//! long-lived opaque token (refresh). Only the refresh token's SHA-256
//! hash is stored — a database leak does not leak working sessions.
//! See tuma-docs/Tuma_Auth_and_RBAC_Architecture.md §5.

use crate::jwt::WEB_REFRESH_TTL_SECS;
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// 256 random bits, hex-encoded — enough that brute force is a non-concept.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    hex::encode(bytes)
}

/// Issue a fresh refresh token for `user_id`. Returns the plaintext token
/// (sent to the browser once, never stored); only its hash hits the DB.
pub async fn issue(conn: &mut PgConnection, user_id: Uuid) -> Result<String, sqlx::Error> {
    let token = generate_token();
    let expires_at = OffsetDateTime::now_utc() + Duration::seconds(WEB_REFRESH_TTL_SECS as i64);
    sqlx::query!(
        r#"
        INSERT INTO tuma.refresh_tokens (token_hash, user_id, expires_at)
        VALUES ($1, $2, $3)
        "#,
        hash_token(&token),
        user_id,
        expires_at,
    )
    .execute(&mut *conn)
    .await?;
    Ok(token)
}

/// Validate a plaintext refresh token. `None` means unknown, expired, or
/// revoked — callers must not distinguish these.
pub async fn validate(conn: &mut PgConnection, token: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT user_id, expires_at, revoked_at
        FROM tuma.refresh_tokens
        WHERE token_hash = $1
        "#,
        hash_token(token),
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(match row {
        Some(row) if row.revoked_at.is_none() && row.expires_at > OffsetDateTime::now_utc() => {
            Some(row.user_id)
        }
        _ => None,
    })
}

/// Revoke a refresh token (logout). Returns whether a live row was found.
pub async fn revoke(conn: &mut PgConnection, token: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE tuma.refresh_tokens
        SET revoked_at = now()
        WHERE token_hash = $1 AND revoked_at IS NULL
        "#,
        hash_token(token),
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() > 0)
}
