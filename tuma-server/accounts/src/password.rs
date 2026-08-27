//! Password hashing (argon2) for merchant and admin accounts.
//!
//! Hashing is deliberately expensive — that is the point — so both
//! operations run on the blocking thread pool, gated by a semaphore so a
//! flood of login attempts cannot pin every worker thread.

use argon2::Argon2;
use password_hash::phc::PasswordHash;
use password_hash::{PasswordHasher, PasswordVerifier};
use tokio::sync::Semaphore;

static HASH_CONCURRENCY: Semaphore = Semaphore::const_new(4);

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password hashing failed: {0}")]
    Hash(String),
}

/// Hash a plaintext password into a PHC string safe to store.
/// The salt is generated internally — unique per password.
pub async fn hash(password: &str) -> Result<String, PasswordError> {
    let password = password.to_string();
    let _permit = HASH_CONCURRENCY
        .acquire()
        .await
        .expect("hashing semaphore closed");
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
/// Returns `false` for a malformed hash rather than erroring, so login
/// endpoints can answer with one generic 401.
pub async fn verify(password: &str, stored_hash: &str) -> bool {
    let password = password.to_string();
    let stored_hash = stored_hash.to_string();
    let _permit = HASH_CONCURRENCY
        .acquire()
        .await
        .expect("hashing semaphore closed");
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
