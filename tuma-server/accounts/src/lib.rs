//! Identity domain: users, roles, JWT sessions, and password hashing.
//!
//! Pure domain crate — no HTTP concerns. Handlers in `tuma-server` call
//! these functions with a database connection and translate errors.

pub mod jwt;
pub mod otp;
pub mod password;
pub mod users;

pub use users::{User, UserRole};
