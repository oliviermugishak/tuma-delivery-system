use secrecy::ExposeSecret;
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

    let connection_pool = PgPoolOptions::new().connect_lazy_with(config.database_with_db());

    if Environment::current() == Environment::Production {
        MIGRATOR
            .run(&connection_pool)
            .await
            .expect("Failed to run database migrations");
    }

    let app_state = AppState::new(
        connection_pool,
        std::sync::Arc::new(accounts::AccountManager::new(4)),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
        config.application.cookie_secure,
        storage::build_service(&config.storage).expect("Failed to build the storage backend"),
        routing::build_service(&config.routing).expect("Failed to build the routing backend"),
        tuma_server::app::GoogleKeys {
            geocoding: config
                .routing
                .geo_api_key
                .as_ref()
                .map(|key| key.expose_secret().to_owned())
                .filter(|key| !key.is_empty()),
        },
    );
    let app = build_app_with_state(app_state);
    let address = format!("{}:{}", config.application.host, config.application.port);

    tracing::info!(address = %address, "server listening");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Failed to bind listener");
    let _ = axum::serve(listener, app).await;
}
