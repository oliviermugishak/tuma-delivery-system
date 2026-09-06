//! Per-IP rate limiting on the auth doors. The limiter's own suite: a
//! freshly spawned app (buckets empty, limits enabled) gets hammered —
//! ten wrong passwords are ten 401s, then the door itself answers 429, a
//! valid login included. The OTP door has its own budget, and every
//! non-auth route answers as if the limiter didn't exist.

mod common;

use app_config::GeocodingBackend;
use common::{MIGRATOR, TestApp, TestClient, seed_merchant};
use serde_json::{Value, json};
use tuma_server::config::{Config, StorageBackend, get_configuration};

/// spawn_app's twin, with the rate limit LEFT ON — the whole point here.
/// Storage and geocoding stay hermetic (memory backends) like the shared
/// harness.
async fn spawn_rate_limited_app(pool: sqlx::PgPool) -> TestApp {
    let mut config: Config = get_configuration().expect("Failed to get configuration");
    config.storage.backend = StorageBackend::Memory;
    config.geocoding.backend = GeocodingBackend::Memory;
    assert!(config.rate_limit.enabled, "local config ships the doors on");

    let listener = tokio::net::TcpListener::bind(format!("{}:0", config.application.host))
        .await
        .expect("Failed to bind listener");
    let port = listener.local_addr().unwrap().port();
    let address = format!("{}:{}", config.application.host, port);

    let state = tuma_server::app::AppState::new(
        pool.clone(),
        std::sync::Arc::new(accounts::AccountManager::new(4)),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
        config.application.cookie_secure,
        storage::build_service(&config.storage).expect("storage"),
        routing::build_service(&config.routing).expect("routing"),
        geocoding::build_service(&config.geocoding).expect("geocoding"),
        config.rate_limit.clone(),
    );
    let app = tuma_server::app::build_app_with_state(state);

    std::mem::forget(tokio::spawn(async {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    }));

    TestApp {
        pool,
        address,
        config,
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_auth_doors_rate_limit_by_ip(pool: sqlx::PgPool) {
    let app = spawn_rate_limited_app(pool).await;
    let seeded = seed_merchant(&app.pool, "aline@example.com", "Aline's Kitchen").await;
    let _ = &seeded.merchant;
    let client = TestClient::new(&app.address);

    // Ten failed logins: every one is the generic-credential 401 — the
    // cap has not been hit yet, and the error is never a 429.
    for attempt in 1..=10 {
        let response = client
            .post_json(
                "/v1/auth/login",
                json!({ "email": "aline@example.com", "password": "WrongPassword" }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            401,
            "attempt {attempt} must be the credential rejection"
        );
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"], "unauthorized");
    }

    // The 11th attempt — right password or wrong — meets a shut door.
    let response = client
        .post_json(
            "/v1/auth/login",
            json!({ "email": "aline@example.com", "password": "WrongPassword" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "too_many_requests");

    // A VALID login also 429s: the door counts attempts, not outcomes —
    // the credential never even reaches argon2.
    let response = client
        .post_json(
            "/v1/auth/login",
            json!({ "email": "aline@example.com", "password": "Password123" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429, "the door is shut for the window");

    // The OTP door has its own budget: ten pass, the 11th is the 429.
    for attempt in 1..=10 {
        let response = client
            .post_json("/v1/auth/otp/request", json!({ "phone": "+250783002002" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "otp attempt {attempt} admitted");
    }
    let response = client
        .post_json("/v1/auth/otp/request", json!({ "phone": "+250783002002" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);

    // Only the doors are limited: the rest of the API answers normally
    // throughout — an unauthenticated route is 200, an auth-guarded one
    // is its ordinary 401, never a 429.
    let response = client.get("/health").send().await.unwrap();
    assert_eq!(response.status(), 200, "health ignores the limiter");
    let response = client.get("/v1/openapi.json").send().await.unwrap();
    assert_eq!(response.status(), 200, "non-auth routes pass untouched");
    let response = client.get("/v1/stores").send().await.unwrap();
    assert_eq!(
        response.status(),
        401,
        "an auth-guarded route rejects as itself, not as the limiter"
    );
}
