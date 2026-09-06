use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tuma_server::app::AppState;
use tuma_server::app::allowed_origins;
use tuma_server::app::build_app_with_state;
use tuma_server::config::Environment;
use tuma_server::config::get_configuration;
use tuma_server::telemetry::get_subscriber;
use tuma_server::telemetry::init_subscriber;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[tokio::main]
async fn main() {
    let subscriber = get_subscriber(
        "tuma-server".into(),
        "info,tuma_server=debug,tower_http=debug,sqlx=warn".into(),
        std::io::stdout,
    );
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to get configuration");

    if allowed_origins().is_empty() {
        tracing::warn!("TUMA_CORS_ORIGIN is not set — browser mutations will be rejected");
    }

    let connection_pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .acquire_timeout(std::time::Duration::from_secs(
            config.database.acquire_timeout_secs,
        ))
        .connect_lazy_with(config.database_with_db());

    if Environment::current() == Environment::Production {
        MIGRATOR
            .run(&connection_pool)
            .await
            .expect("Failed to run database migrations");
    }

    // The maintenance prune — the system's first background job. Hourly;
    // the first tick fires immediately, so a restart prunes on startup.
    // A failed pass is logged and retried an hour later — a prune is
    // housekeeping, never a crash.
    if config.jobs.prune_enabled {
        let pool = connection_pool.clone();
        let retention = config.jobs.prune_retention_days;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
            loop {
                interval.tick().await;
                let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(retention);
                match async {
                    let mut conn = pool.acquire().await?;
                    let locations =
                        commerce::deliveries::prune_old_locations(&mut conn, cutoff).await?;
                    let tokens =
                        accounts::refresh_tokens::prune_dead_tokens(&mut conn, cutoff).await?;
                    Ok::<_, sqlx::Error>((locations, tokens))
                }
                .await
                {
                    Ok((locations, tokens)) => {
                        tracing::info!(
                            deleted_locations = locations,
                            deleted_tokens = tokens,
                            retention_days = retention,
                            "maintenance prune complete"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(%error, "maintenance prune failed — retrying next hour");
                    }
                }
            }
        });
    }

    let app_state = AppState::new(
        connection_pool,
        std::sync::Arc::new(accounts::AccountManager::new(4)),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
        config.application.cookie_secure,
        storage::build_service(&config.storage).expect("Failed to build the storage backend"),
        routing::build_service(&config.routing).expect("Failed to build the routing backend"),
        geocoding::build_service(&config.geocoding).expect("Failed to build the geocoding backend"),
        config.rate_limit.clone(),
    );
    let app = build_app_with_state(app_state);
    let address = format!("{}:{}", config.application.host, config.application.port);

    tracing::info!(address = %address, "server listening");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Failed to bind listener");
    let _ = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await;
}
