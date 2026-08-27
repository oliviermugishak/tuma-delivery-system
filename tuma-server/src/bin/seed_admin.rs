//! Seeds the first admin account — admins never come into existence via the
//! API (see Tuma_Auth_and_RBAC_Architecture.md §2).
//!
//! Usage:
//!   TUMA_ADMIN_PASSWORD=... cargo run --bin seed_admin
//!   TUMA_ADMIN_EMAIL=boss@tuma.rw TUMA_ADMIN_PASSWORD=... cargo run --bin seed_admin
//!
//! Runs pending migrations first, so seeding a fresh database is one
//! command. An existing admin is left untouched.

use accounts::UserRole;
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
    if accounts::users::by_email(&mut conn, &email)
        .await?
        .is_some()
    {
        println!("admin {email} already exists — leaving it alone");
        return Ok(());
    }

    let password_hash = accounts::password::hash(&password).await?;
    let user = accounts::users::create_staff(
        &mut conn,
        UserRole::Admin,
        Some("Tuma Admin"),
        &email,
        &password_hash,
    )
    .await?;

    println!("seeded admin {email} ({})", user.id);
    Ok(())
}
