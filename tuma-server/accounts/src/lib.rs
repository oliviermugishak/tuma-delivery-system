//! Identity domain: accounts, authorization profiles, merchant businesses
//! and memberships, JWT sessions, OTP, and the account manager that owns
//! creation + password work.
//!
//! Pure domain crate — no HTTP concerns. Handlers in `tuma-server` call
//! the [`AccountManager`] (and the lookup helpers) with a database
//! connection and translate the typed errors.

pub mod admins;
pub mod customers;
pub mod jwt;
pub mod manager;
pub mod memberships;
pub mod merchants;
pub mod otp;
pub mod refresh_tokens;
pub mod riders;
pub mod users;

pub use manager::{AccountManager, ChangePasswordError, CreateAccountError, PasswordError};
pub use memberships::{Membership, MembershipRole, MembershipView};
pub use merchants::{Merchant, MerchantStatus};
pub use riders::Rider;
pub use users::Account;

/// Everything the middleware needs to authorize an authenticated account,
/// resolved fresh per request. Profiles and memberships never live in the
/// token, so a revoked membership or a suspended business locks the account
/// out on its very next request.
#[derive(Debug, Clone, Default)]
pub struct AuthorizationContext {
    pub customer: Option<customers::Customer>,
    pub admin: Option<admins::AdminProfile>,
    pub rider: Option<riders::Rider>,
    pub memberships: Vec<memberships::MembershipView>,
}

/// Load the authorization context for an account: four indexed lookups on
/// one connection, priced once per authenticated request.
pub async fn authorization_for(
    conn: &mut sqlx::PgConnection,
    account_id: uuid::Uuid,
) -> Result<AuthorizationContext, sqlx::Error> {
    let customer = customers::by_user_id(&mut *conn, account_id).await?;
    let admin = admins::by_user_id(&mut *conn, account_id).await?;
    let rider = riders::by_user_id(&mut *conn, account_id).await?;
    let memberships = memberships::views_for_user(&mut *conn, account_id).await?;
    Ok(AuthorizationContext {
        customer,
        admin,
        rider,
        memberships,
    })
}
