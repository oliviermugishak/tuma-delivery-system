//! One-time codes for passwordless customer auth.
//!
//! Rules (Tuma_Auth_and_RBAC_Architecture.md §6): 6 digits, 5-minute TTL,
//! 60-second resend cooldown, 5 attempts per code, codes hashed at rest,
//! and errors that never reveal whether a phone number is known.

use crate::customers;
use crate::manager::{AccountManager, CreateAccountError};
use crate::riders;
use crate::users::{self, Account};
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
        SELECT created_at FROM accounts.auth_otps
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
        INSERT INTO accounts.auth_otps (phone, code_hash, expires_at)
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

/// What a successful verification yields: the authenticated account and its
/// customer profile (register and login are one flow).
#[derive(Debug, Clone)]
pub struct OtpSignIn {
    pub account: Account,
    /// The customer profile for a customer sign-in. `None` when the account
    /// is a rider — riders get no customer profile; rider mode is their
    /// surface (the admin created the account with a name already).
    pub customer: Option<customers::Customer>,
    pub rider: Option<riders::Rider>,
}

/// Verify `code` for `phone`. On success the code is consumed and the
/// account is fetched-or-created. Customer accounts get their customer
/// profile ensured (register and login are one flow) — unless the account
/// is a rider, whose profile already exists and who gets no customer one.
/// If `name` is given and the customer profile has none yet, it is set.
pub async fn verify(
    accounts: &AccountManager,
    conn: &mut PgConnection,
    phone: &str,
    code: &str,
    name: Option<&str>,
) -> Result<OtpSignIn, VerifyError> {
    // The validation and the attempt burn are ONE statement: two parallel
    // verifies used to read the same `attempts` count and both pass the
    // cap (a soft cap under brute force). Now Postgres serializes the
    // row update — every attempt, right or wrong, counts exactly once.
    let matched = sqlx::query!(
        r#"
        UPDATE accounts.auth_otps
        SET attempts = attempts + 1
        WHERE phone = $1 AND code_hash = $2
          AND expires_at > now() AND attempts < $3
        RETURNING phone
        "#,
        phone,
        hash_code(code),
        MAX_ATTEMPTS,
    )
    .fetch_optional(&mut *conn)
    .await?;

    if matched.is_none() {
        // Attribute the failure for the burn rule: an expired code burned
        // nothing (it's dead anyway); a wrong code or a capped-out phone
        // burns one more attempt — bounded at MAX + the burns that fit.
        let row = sqlx::query!(
            r#"
            SELECT expires_at, attempts FROM accounts.auth_otps
            WHERE phone = $1
            "#,
            phone,
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(row) = row
            && row.expires_at >= OffsetDateTime::now_utc()
            && row.attempts < MAX_ATTEMPTS
        {
            // Wrong code, still under the cap — burn it.
            sqlx::query!(
                r#"
                UPDATE accounts.auth_otps SET attempts = attempts + 1
                WHERE phone = $1
                "#,
                phone,
            )
            .execute(&mut *conn)
            .await?;
        }
        return Err(VerifyError::InvalidCode);
    }

    sqlx::query!(r#"DELETE FROM accounts.auth_otps WHERE phone = $1"#, phone)
        .execute(&mut *conn)
        .await?;

    let account = match users::by_phone(conn, phone).await? {
        Some(account) => account,
        None => accounts
            .create_phone_account(conn, phone)
            .await
            .map_err(|error| match error {
                // A customer insert touches only the phone unique
                // constraint and hashes nothing — no other arm can fire.
                CreateAccountError::PhoneTaken => VerifyError::PhoneTaken,
                CreateAccountError::EmailTaken | CreateAccountError::Password(_) => {
                    unreachable!("customer creation cannot conflict on email or password")
                }
                CreateAccountError::Database(error) => VerifyError::Database(error),
            })?,
    };

    // Role-routing: an admin-created rider account signs in with the same
    // OTP flow but already carries its rider profile — no customer profile
    // is ensured for it, so require_customer correctly 403s and the app
    // routes it to rider mode.
    if let Some(rider) = riders::by_user_id(conn, account.id).await? {
        return Ok(OtpSignIn {
            account,
            customer: None,
            rider: Some(rider),
        });
    }

    let mut customer = customers::ensure_for_user(conn, account.id).await?;

    // A returning customer who never gave a name can still provide one.
    if customer.name.is_none()
        && let Some(name) = name
    {
        customer = customers::set_name(conn, account.id, name).await?;
    }

    Ok(OtpSignIn {
        account,
        customer: Some(customer),
        rider: None,
    })
}
