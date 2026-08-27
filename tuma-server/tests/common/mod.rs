#![allow(dead_code)]
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
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
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
    pub pool: Option<PgPool>,
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

    let state = AppState::new(pool.clone());
    let app = build_app_with_state(state);

    std::mem::forget(tokio::spawn(async {
        let _ = axum::serve(listener, app).await;
    }));

    TestApp {
        pool: Some(pool),
        address,
        config,
    }
}
