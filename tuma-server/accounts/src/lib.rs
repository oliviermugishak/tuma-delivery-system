//! Identity domain: users, roles, JWT sessions, OTP, and the account
//! manager that owns creation + password work.
//!
//! Pure domain crate — no HTTP concerns. Handlers in `tuma-server` call
//! the [`AccountManager`] (and the lookup helpers) with a database
//! connection and translate the typed errors.

pub mod jwt;
pub mod manager;
pub mod otp;
pub mod refresh_tokens;
pub mod users;

pub use manager::{AccountManager, ChangePasswordError, CreateAccountError, PasswordError};
pub use users::{User, UserRole};
