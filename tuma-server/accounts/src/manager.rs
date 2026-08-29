//! Account lifecycle: creation and everything that touches a password.
//!
//! One entry point for all account creation and password work (the
//! kanombe-sda AccountManager pattern). Hashing is deliberately expensive —
//! that is the point — so hash/verify run on the blocking thread pool, gated
//! by a semaphore so a flood of login attempts cannot pin every worker
//! thread. Callers (handlers, tests, bins) never touch argon2 directly.

use crate::users::{self, Account};
use argon2::Argon2;
use password_hash::phc::PasswordHash;
use password_hash::{PasswordHasher, PasswordVerifier};
use sqlx::PgConnection;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Precomputed valid hash that sign-in verification runs against when an
/// account doesn't exist (or has no password), so response timing doesn't
/// reveal whether an email is registered. Never matches a real password.
const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$mk6ChPxzLWLbavr3YWz+zA$gbs9j8ObTGfLbt9M37G0gIcj72oywfl+AG3oHc0QPZM";

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password hashing failed: {0}")]
    Hash(String),
}

/// Account creation failed with the conflicting identity identified, so
/// clients can show the real reason instead of a guess.
#[derive(Debug, thiserror::Error)]
pub enum CreateAccountError {
    #[error("a user with this email already exists")]
    EmailTaken,
    #[error("a user with this phone number already exists")]
    PhoneTaken,
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ChangePasswordError {
    #[error("this account signs in with OTP — there is no password to change")]
    NoPassword,
    #[error("current password is incorrect")]
    WrongCurrentPassword,
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug)]
pub struct AccountManager {
    hashing_semaphore: Arc<Semaphore>,
}

impl AccountManager {
    pub fn new(max_concurrent_hashes: usize) -> Self {
        Self {
            hashing_semaphore: Arc::new(Semaphore::new(max_concurrent_hashes)),
        }
    }

    /// Hash a plaintext password into a PHC string safe to store.
    /// The salt is generated internally — unique per password.
    async fn hash_password(&self, password: String) -> Result<String, PasswordError> {
        let _permit = self.acquire_hash_permit().await;
        tokio::task::spawn_blocking(move || {
            Argon2::default()
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|e| PasswordError::Hash(e.to_string()))
        })
        .await
        .expect("hashing task panicked")
    }

    /// Check a plaintext password against a stored PHC hash.
    /// Returns `false` for a malformed hash rather than erroring, so sign-in
    /// endpoints can answer with one generic 401.
    async fn verify_password(&self, password: String, stored_hash: String) -> bool {
        let _permit = self.acquire_hash_permit().await;
        tokio::task::spawn_blocking(move || {
            let Ok(parsed) = PasswordHash::new(&stored_hash) else {
                return false;
            };
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .await
        .expect("hashing task panicked")
    }

    async fn acquire_hash_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.hashing_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("BUG: hashing semaphore should not be closed")
    }

    /// Create a phone-anchored account (the customer OTP path). The customer
    /// profile is ensured by the caller's flow — see [`crate::customers`].
    pub async fn create_phone_account(
        &self,
        conn: &mut PgConnection,
        phone: &str,
    ) -> Result<Account, CreateAccountError> {
        users::create_with_phone(conn, phone)
            .await
            .map_err(unique_violation)
    }

    /// Create an email + password account (merchant staff, admins), hashing
    /// the password here — callers pass plaintext, never hashes.
    pub async fn create_password_account(
        &self,
        conn: &mut PgConnection,
        email: &str,
        password: &str,
    ) -> Result<Account, CreateAccountError> {
        let password_hash = self.hash_password(password.to_string()).await?;
        users::create_with_password(conn, email, &password_hash)
            .await
            .map_err(unique_violation)
    }

    /// Timing-equalized password check for sign-in: accounts that cannot
    /// match (unknown email, account without a password) still burn an argon2
    /// verify against a dummy hash so timing reveals nothing. Returns whether
    /// the password matched the account's stored hash.
    pub async fn verify_login_password(&self, account: Option<&Account>, password: &str) -> bool {
        match account.and_then(|account| account.password_hash.clone()) {
            Some(hash) => self.verify_password(password.to_string(), hash).await,
            None => {
                self.verify_password(password.to_string(), DUMMY_HASH.to_string())
                    .await;
                false
            }
        }
    }

    /// Self-service password change: verifies the current password, then
    /// stores the new hash. OTP accounts have no password to change.
    pub async fn change_password(
        &self,
        conn: &mut PgConnection,
        account: &Account,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), ChangePasswordError> {
        let Some(hash) = account.password_hash.clone() else {
            return Err(ChangePasswordError::NoPassword);
        };
        if !self
            .verify_password(current_password.to_string(), hash)
            .await
        {
            return Err(ChangePasswordError::WrongCurrentPassword);
        }
        let new_hash = self.hash_password(new_password.to_string()).await?;
        users::set_password(conn, account.id, &new_hash).await?;
        Ok(())
    }
}

/// Resolve a Postgres unique violation (23505) to the identity that actually
/// collided, using the constraint name. Anything else passes through as a
/// plain database error. Shared by account creation and admin identity
/// edits, which can collide with taken emails/phones the same way.
pub(crate) fn unique_violation(error: sqlx::Error) -> CreateAccountError {
    if let sqlx::Error::Database(db_error) = &error
        && db_error.code() == Some(Cow::Borrowed("23505"))
    {
        return match db_error.constraint() {
            Some("users_email_key") => CreateAccountError::EmailTaken,
            Some("users_phone_key") => CreateAccountError::PhoneTaken,
            _ => CreateAccountError::Database(error),
        };
    }
    CreateAccountError::Database(error)
}
