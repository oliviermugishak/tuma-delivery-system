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

use time::OffsetDateTime;
use uuid::Uuid;

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

/// One combined row: the account plus whichever profiles exist. The
/// profiles ride LEFT JOINs, so every joined column is nullable — and a
/// profile row's own columns are non-null exactly when its id is.
#[derive(Debug)]
struct ProfileRow {
    u_id: Uuid,
    u_phone: Option<String>,
    u_email: Option<String>,
    u_password_hash: Option<String>,
    u_is_active: bool,
    u_created_at: OffsetDateTime,
    u_updated_at: OffsetDateTime,
    c_id: Option<Uuid>,
    c_name: Option<String>,
    c_created_at: Option<OffsetDateTime>,
    c_updated_at: Option<OffsetDateTime>,
    a_id: Option<Uuid>,
    a_name: Option<String>,
    a_created_at: Option<OffsetDateTime>,
    a_updated_at: Option<OffsetDateTime>,
    r_id: Option<Uuid>,
    r_account_id: Option<Uuid>,
    r_rider_number: Option<i64>,
    r_name: Option<String>,
    r_phone: Option<String>,
    r_is_active: Option<bool>,
    r_created_at: Option<OffsetDateTime>,
    r_updated_at: Option<OffsetDateTime>,
}

/// Load the authorization context for an account: two indexed queries on
/// one connection, priced once per authenticated request — the account row
/// rides the combined profile lookup. Returns `None` for a deleted account.
pub async fn authorization_for(
    conn: &mut sqlx::PgConnection,
    account_id: Uuid,
) -> Result<Option<(Account, AuthorizationContext)>, sqlx::Error> {
    let row = sqlx::query_as!(
        ProfileRow,
        r#"
        SELECT u.id AS "u_id!",
               u.phone AS "u_phone?",
               u.email AS "u_email?",
               u.password_hash AS "u_password_hash?",
               u.is_active AS "u_is_active!",
               u.created_at AS "u_created_at!",
               u.updated_at AS "u_updated_at!",
               c.id AS "c_id?",
               c.name AS "c_name?",
               c.created_at AS "c_created_at?",
               c.updated_at AS "c_updated_at?",
               a.id AS "a_id?",
               a.name AS "a_name?",
               a.created_at AS "a_created_at?",
               a.updated_at AS "a_updated_at?",
               r.id AS "r_id?",
               r.account_id AS "r_account_id?",
               r.rider_number AS "r_rider_number?",
               r.name AS "r_name?",
               r.phone AS "r_phone?",
               r.is_active AS "r_is_active?",
               r.created_at AS "r_created_at?",
               r.updated_at AS "r_updated_at?"
        FROM accounts.users u
        LEFT JOIN accounts.customers c ON c.user_id = u.id
        LEFT JOIN accounts.admins a ON a.user_id = u.id
        LEFT JOIN commerce.riders r ON r.account_id = u.id
        WHERE u.id = $1
        "#,
        account_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let memberships = memberships::views_for_user(&mut *conn, account_id).await?;

    let user = Account {
        id: row.u_id,
        phone: row.u_phone,
        email: row.u_email,
        password_hash: row.u_password_hash,
        is_active: row.u_is_active,
        created_at: row.u_created_at,
        updated_at: row.u_updated_at,
    };
    let customer = match (row.c_id, row.c_created_at, row.c_updated_at) {
        (Some(id), Some(created_at), Some(updated_at)) => Some(customers::Customer {
            id,
            user_id: account_id,
            name: row.c_name,
            created_at,
            updated_at,
        }),
        _ => None,
    };
    let admin = match (row.a_id, row.a_created_at, row.a_updated_at) {
        (Some(id), Some(created_at), Some(updated_at)) => Some(admins::AdminProfile {
            id,
            user_id: account_id,
            name: row.a_name,
            created_at,
            updated_at,
        }),
        _ => None,
    };
    let rider = match (
        row.r_id,
        row.r_account_id,
        row.r_rider_number,
        row.r_name,
        row.r_phone,
        row.r_is_active,
        row.r_created_at,
        row.r_updated_at,
    ) {
        (
            Some(id),
            Some(account),
            Some(rider_number),
            Some(name),
            Some(phone),
            Some(is_active),
            Some(created_at),
            Some(updated_at),
        ) => Some(riders::Rider {
            id,
            account_id: account,
            rider_number,
            name,
            phone,
            is_active,
            created_at,
            updated_at,
        }),
        _ => None,
    };

    Ok(Some((
        user,
        AuthorizationContext {
            customer,
            admin,
            rider,
            memberships,
        },
    )))
}
