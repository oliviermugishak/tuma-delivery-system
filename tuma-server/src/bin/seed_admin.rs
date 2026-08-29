//! Seeds the first platform admin — admins never come into existence via
//! the API (the profile row is what makes an account an admin).
//!
//! Usage:
//!   TUMA_ADMIN_PASSWORD=... cargo run --bin seed_admin
//!   TUMA_ADMIN_EMAIL=boss@tuma.rw TUMA_ADMIN_PASSWORD=... cargo run --bin seed_admin
//!
//! Runs pending migrations first, so seeding a fresh database is one
//! command. An existing admin is left untouched.

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tuma_server::config::get_configuration;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = get_configuration().expect("Failed to get configuration");
    let pool = PgPoolOptions::new()
        .connect_with(config.database_with_db())
        .await?;
    MIGRATOR.run(&pool).await?;

    let email = std::env::var("TUMA_ADMIN_EMAIL").unwrap_or_else(|_| "admin@tuma.rw".into());
    let password = std::env::var("TUMA_ADMIN_PASSWORD")
        .map_err(|_| anyhow::anyhow!("TUMA_ADMIN_PASSWORD is required"))?;

    let mut conn = pool.acquire().await?;
    if let Some(existing) = accounts::users::by_email(&mut conn, &email).await?
        && accounts::admins::by_user_id(&mut conn, existing.id)
            .await?
            .is_some()
    {
        println!("admin {email} already exists — leaving it alone");
        return Ok(());
    }

    let accounts_manager = accounts::AccountManager::new(1);
    let account = match accounts::users::by_email(&mut conn, &email).await? {
        Some(existing) => existing,
        None => {
            accounts_manager
                .create_password_account(&mut conn, &email, &password)
                .await?
        }
    };
    accounts::admins::ensure_for_user(&mut conn, account.id, Some("Tuma Admin")).await?;

    println!("seeded admin {email} ({})", account.id);
    Ok(())
}
