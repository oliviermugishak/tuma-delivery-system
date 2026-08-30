#![allow(dead_code)]
use accounts::jwt;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use tuma_server::app::AppState;
use tuma_server::app::build_app_with_state;
use tuma_server::config::Config;
use tuma_server::config::get_configuration;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
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

    pub fn patch_json(&self, path: &str, body: serde_json::Value) -> reqwest::RequestBuilder {
        self.client.patch(self.url(path)).json(&body)
    }

    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.delete(self.url(path))
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
    let mut config = get_configuration().expect("Failed to get configuration");
    // Tests are hermetic: the in-memory backend, always — uploads never
    // touch the developer's disk and vanish with the test.
    config.storage.backend = tuma_server::config::StorageBackend::Memory;

    let listener = tokio::net::TcpListener::bind(format!("{}:0", config.application.host))
        .await
        .expect("Failed to bind listener");
    let port = listener.local_addr().unwrap().port();
    let address = format!("{}:{}", config.application.host, port);

    let state = AppState::new(
        pool.clone(),
        std::sync::Arc::new(accounts::AccountManager::new(4)),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
        config.application.cookie_secure,
        storage::build_service(&config.storage).expect("Failed to build the storage backend"),
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

/// A customer account + its profile, seeded directly — tests own the
/// database, no OTP needed.
pub struct CustomerSeed {
    pub account: accounts::Account,
    pub customer: accounts::customers::Customer,
}

pub async fn seed_customer(pool: &PgPool, phone: &str) -> CustomerSeed {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    let account = accounts::AccountManager::new(1)
        .create_phone_account(&mut conn, phone)
        .await
        .expect("failed to seed customer account");
    let customer = accounts::customers::ensure_for_user(&mut conn, account.id)
        .await
        .expect("failed to seed customer profile");
    CustomerSeed { account, customer }
}

/// A platform admin: account + admin profile, known password "Password123".
pub struct AdminSeed {
    pub account: accounts::Account,
    pub admin: accounts::admins::AdminProfile,
}

pub async fn seed_admin(pool: &PgPool, email: &str) -> AdminSeed {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    let account = accounts::AccountManager::new(1)
        .create_password_account(&mut conn, email, "Password123")
        .await
        .expect("failed to seed admin account");
    let admin = accounts::admins::ensure_for_user(&mut conn, account.id, None)
        .await
        .expect("failed to seed admin profile");
    AdminSeed { account, admin }
}

/// A merchant business + its owner's account + the owner membership.
pub struct MerchantSeed {
    pub account: accounts::Account,
    pub merchant: accounts::merchants::Merchant,
    pub membership: accounts::memberships::Membership,
}

pub async fn seed_merchant(pool: &PgPool, owner_email: &str, business_name: &str) -> MerchantSeed {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    let merchant = accounts::merchants::create(&mut conn, business_name, None, None)
        .await
        .expect("failed to seed merchant business");
    let account = accounts::AccountManager::new(1)
        .create_password_account(&mut conn, owner_email, "Password123")
        .await
        .expect("failed to seed owner account");
    let membership = accounts::memberships::create(
        &mut conn,
        account.id,
        merchant.id,
        accounts::MembershipRole::Owner,
        None,
    )
    .await
    .expect("failed to seed owner membership");
    MerchantSeed {
        account,
        merchant,
        membership,
    }
}

/// A store-scoped manager: an account whose membership covers exactly one
/// store of one business.
pub async fn seed_store_manager(
    pool: &PgPool,
    email: &str,
    merchant_id: uuid::Uuid,
    store_id: uuid::Uuid,
) -> accounts::Account {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    let account = accounts::AccountManager::new(1)
        .create_password_account(&mut conn, email, "Password123")
        .await
        .expect("failed to seed manager account");
    accounts::memberships::create(
        &mut conn,
        account.id,
        merchant_id,
        accounts::MembershipRole::Manager,
        Some(store_id),
    )
    .await
    .expect("failed to seed manager membership");
    account
}

/// Seed a store (closed by default) for a business.
pub async fn seed_store(
    pool: &PgPool,
    merchant_id: uuid::Uuid,
    name: &str,
    is_open: bool,
) -> marketplace::stores::Store {
    let mut conn = pool.acquire().await.expect("failed to acquire connection");
    marketplace::stores::create_store(
        &mut conn,
        merchant_id,
        marketplace::stores::StoreChanges {
            name: name.to_string(),
            description: None,
            image_url: None,
            address_text: None,
            lat: None,
            lng: None,
            category: None,
            delivery_fee: 0,
            is_open,
        },
    )
    .await
    .expect("failed to seed store")
}

/// Mint a real signed token the way the server would — the account id is
/// all the token carries.
pub fn token_for(app: &TestApp, account_id: uuid::Uuid, ttl_secs: usize) -> String {
    let claims = jwt::Claims::new(account_id, ttl_secs);
    jwt::generate(
        &claims,
        app.config.secret.jwt_signing_key.expose_secret().as_bytes(),
    )
    .expect("failed to mint token")
}

/// Extract a cookie's value from a response's `Set-Cookie` headers.
/// Returns `Some("")` for clearing cookies, `None` if the cookie is absent.
pub fn cookie_value(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|line| {
            let value = line.strip_prefix(&format!("{name}="))?;
            Some(value.split(';').next().unwrap_or_default().to_string())
        })
}

/// All raw `Set-Cookie` header lines, for asserting cookie attributes.
pub fn set_cookie_lines(response: &reqwest::Response) -> Vec<&str> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect()
}

/// Email + password login over the cookie transport.
pub async fn login(client: &TestClient, email: &str, password: &str) -> reqwest::Response {
    client
        .post_json(
            "/v1/auth/login",
            serde_json::json!({ "email": email, "password": password }),
        )
        .send()
        .await
        .expect("failed to call /auth/login")
}
