#![allow(dead_code)]
use accounts::UserRole;
use accounts::jwt;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use tuma_server::app::AppState;
use tuma_server::app::build_app_with_state;
use tuma_server::config::Config;
use tuma_server::config::get_configuration;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub struct TestClient {
    base_url: String,
    client: reqwest::Client,
}

impl TestClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .expect("failed to build test client"),
        }
    }

    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(self.url(path))
    }

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(self.url(path))
    }

    pub fn post_json(&self, path: &str, body: serde_json::Value) -> reqwest::RequestBuilder {
        self.client.post(self.url(path)).json(&body)
    }

    pub fn url(&self, path: &str) -> String {
        assert!(path.starts_with("/"), "missing / for url path");
        format!("http://{}/api{}", self.base_url, path)
    }
}

pub struct TestApp {
    pub pool: PgPool,
    pub address: String,
    pub config: Config,
}

pub async fn spawn_app(pool: PgPool) -> TestApp {
    let config = get_configuration().expect("Failed to get configuration");

    let listener = tokio::net::TcpListener::bind(format!("{}:0", config.application.host))
        .await
        .expect("Failed to bind listener");
    let port = listener.local_addr().unwrap().port();
    let address = format!("{}:{}", config.application.host, port);

    let state = AppState::new(
        pool.clone(),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
    );
    let app = build_app_with_state(state);

    std::mem::forget(tokio::spawn(async {
        let _ = axum::serve(listener, app).await;
    }));

    TestApp {
        pool,
        address,
        config,
    }
}

/// Seed a customer row directly — tests own the database, no OTP needed.
pub async fn seed_customer(pool: &PgPool, phone: &str) -> accounts::User {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    accounts::users::create_customer(&mut conn, phone, None)
        .await
        .expect("failed to seed customer")
}

/// Seed a merchant/admin row with a known password ("Password123").
pub async fn seed_staff(pool: &PgPool, role: UserRole, email: &str) -> accounts::User {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    let password_hash = accounts::password::hash("Password123")
        .await
        .expect("failed to hash password");
    accounts::users::create_staff(&mut conn, role, None, email, &password_hash)
        .await
        .expect("failed to seed staff user")
}

/// Mint a real signed token the way the server would.
pub fn token_for(app: &TestApp, user: &accounts::User, ttl_secs: usize) -> String {
    let claims = jwt::Claims::new(user.id, user.role, ttl_secs);
    jwt::generate(
        &claims,
        app.config.secret.jwt_signing_key.expose_secret().as_bytes(),
    )
    .expect("failed to mint token")
}
