//! One-time codes for passwordless customer auth.
//!
//! Rules (Tuma_Auth_and_RBAC_Architecture.md §6): 6 digits, 5-minute TTL,
//! 60-second resend cooldown, 5 attempts per code, codes hashed at rest,
//! and errors that never reveal whether a phone number is known.

use crate::User;
use crate::manager::{AccountManager, CreateAccountError};
use crate::users;
use rand::RngExt;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

pub const CODE_TTL_SECS: i64 = 5 * 60;
pub const RESEND_COOLDOWN_SECS: i64 = 60;
pub const MAX_ATTEMPTS: i32 = 5;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// A code was issued less than `RESEND_COOLDOWN_SECS` ago. Handlers
    /// swallow this: the response must look identical either way.
    #[error("otp requested too recently")]
    TooSoon,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// One generic failure for: unknown phone, expired code, dead code,
    /// wrong code. Callers must not distinguish these externally.
    #[error("invalid or expired code")]
    InvalidCode,
    /// The phone was registered between code issue and verify (two devices
    /// racing on the same number). Retrying signs into the existing account.
    #[error("a user with this phone number already exists")]
    PhoneTaken,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

fn hash_code(code: &str) -> String {
    hex::encode(Sha256::digest(code.as_bytes()))
}

fn random_code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

/// Issue a code for `phone`, replacing any previous code. When
/// `dev_fixed_code` is set (local config only), it is issued instead of a
/// random one. Respects the resend cooldown.
pub async fn request(
    conn: &mut PgConnection,
    phone: &str,
    dev_fixed_code: Option<&str>,
) -> Result<(), RequestError> {
    let recent = sqlx::query!(
        r#"
        SELECT created_at FROM tuma.auth_otps
        WHERE phone = $1 AND created_at > now() - make_interval(secs => $2)
        "#,
        phone,
        RESEND_COOLDOWN_SECS as f64,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if recent.is_some() {
        return Err(RequestError::TooSoon);
    }

    let code = dev_fixed_code
        .map(str::to_string)
        .unwrap_or_else(random_code);
    let expires_at = OffsetDateTime::now_utc() + Duration::seconds(CODE_TTL_SECS);

    sqlx::query!(
        r#"
        INSERT INTO tuma.auth_otps (phone, code_hash, expires_at)
        VALUES ($1, $2, $3)
        ON CONFLICT (phone) DO UPDATE
        SET code_hash = EXCLUDED.code_hash,
            expires_at = EXCLUDED.expires_at,
            attempts = 0,
            created_at = now()
        "#,
        phone,
        hash_code(&code),
        expires_at,
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

/// Verify `code` for `phone`. On success the code is consumed and the
/// customer is fetched-or-created (register and login are one flow). If
/// `name` is given and the user has none yet, it is set.
pub async fn verify(
    accounts: &AccountManager,
    conn: &mut PgConnection,
    phone: &str,
    code: &str,
    name: Option<&str>,
) -> Result<User, VerifyError> {
    let row = sqlx::query!(
        r#"
        SELECT code_hash, expires_at, attempts FROM tuma.auth_otps
        WHERE phone = $1
        "#,
        phone,
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(VerifyError::InvalidCode)?;

    let expired = row.expires_at < OffsetDateTime::now_utc();
    if expired || row.attempts >= MAX_ATTEMPTS || row.code_hash != hash_code(code) {
        if !expired {
            // Burn an attempt even on a dead code; errors stay generic.
            sqlx::query!(
                r#"
                UPDATE tuma.auth_otps SET attempts = attempts + 1
                WHERE phone = $1
                "#,
                phone,
            )
            .execute(&mut *conn)
            .await?;
        }
        return Err(VerifyError::InvalidCode);
    }

    sqlx::query!(r#"DELETE FROM tuma.auth_otps WHERE phone = $1"#, phone)
        .execute(&mut *conn)
        .await?;

    let user = match users::by_phone(conn, phone).await? {
        Some(user) => user,
        None => {
            accounts
                .create_customer(conn, phone, name)
                .await
                .map_err(|error| match error {
                    CreateAccountError::PhoneTaken => VerifyError::PhoneTaken,
                    // A customer insert touches only the phone unique
                    // constraint and hashes nothing — no other arm can fire.
                    CreateAccountError::EmailTaken | CreateAccountError::Password(_) => {
                        unreachable!("customer creation cannot conflict on email or password")
                    }
                    CreateAccountError::Database(error) => VerifyError::Database(error),
                })?
        }
    };

    // A returning customer who never gave a name can still provide one.
    if user.name.is_none()
        && let Some(name) = name
    {
        return Ok(users::set_name(conn, user.id, name).await?);
    }

    Ok(user)
}
