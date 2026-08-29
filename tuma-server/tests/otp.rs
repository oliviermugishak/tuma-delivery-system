mod common;

use common::{MIGRATOR, TestClient, spawn_app};
use serde_json::{Value, json};

const DEV_CODE: &str = "123456"; // configuration/local.yml auth.dev_otp_code
const PHONE: &str = "+250780111001";

async fn request_code(client: &TestClient, phone: &str) -> reqwest::Response {
    client
        .post_json("/v1/auth/otp/request", json!({ "phone": phone }))
        .send()
        .await
        .unwrap()
}

async fn verify_code(client: &TestClient, phone: &str, code: &str) -> reqwest::Response {
    client
        .post_json(
            "/v1/auth/otp/verify",
            json!({ "phone": phone, "code": code }),
        )
        .send()
        .await
        .unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn request_then_verify_creates_a_customer(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = request_code(&client, PHONE).await;
    assert_eq!(response.status(), 200);

    let response = verify_code(&client, PHONE, DEV_CODE).await;
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["user"]["phone"], PHONE);
    assert!(
        body["user"]["customer"]["id"].is_string(),
        "a customer profile exists"
    );
    let token = body["token"].as_str().unwrap().to_string();

    // The issued token must work against /me.
    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let me: Value = response.json().await.unwrap();
    assert_eq!(me["id"], body["user"]["id"]);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn verify_with_name_sets_the_name(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    let response = client
        .post_json(
            "/v1/auth/otp/verify",
            json!({ "phone": PHONE, "code": DEV_CODE, "name": "Aline" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["user"]["customer"]["name"], "Aline");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn verify_with_a_wrong_code_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    let response = verify_code(&client, PHONE, "000000").await;
    assert_eq!(response.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn verify_without_a_request_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = verify_code(&client, PHONE, DEV_CODE).await;
    assert_eq!(response.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn five_wrong_attempts_kill_the_code(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    for _ in 0..5 {
        assert_eq!(verify_code(&client, PHONE, "000000").await.status(), 400);
    }
    // Even the correct code is dead now.
    assert_eq!(verify_code(&client, PHONE, DEV_CODE).await.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_returning_customer_logs_back_in(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    let first: Value = verify_code(&client, PHONE, DEV_CODE)
        .await
        .json()
        .await
        .unwrap();

    // Second session, some time later: same phone, same account.
    request_code(&client, PHONE).await;
    let second: Value = verify_code(&client, PHONE, DEV_CODE)
        .await
        .json()
        .await
        .unwrap();

    assert_eq!(first["user"]["id"], second["user"]["id"]);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn resend_cooldown_keeps_the_first_code(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    let (created_at,): (time::OffsetDateTime,) =
        sqlx::query_as("SELECT created_at FROM accounts.auth_otps WHERE phone = $1")
            .bind(PHONE)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    // Second request inside the cooldown: same 200 shape, code untouched.
    let response = request_code(&client, PHONE).await;
    assert_eq!(response.status(), 200);

    let (created_at_after,): (time::OffsetDateTime,) =
        sqlx::query_as("SELECT created_at FROM accounts.auth_otps WHERE phone = $1")
            .bind(PHONE)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    assert_eq!(created_at, created_at_after);
    // And the original code still verifies.
    assert_eq!(verify_code(&client, PHONE, DEV_CODE).await.status(), 200);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_consumed_code_cannot_be_reused(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    request_code(&client, PHONE).await;
    assert_eq!(verify_code(&client, PHONE, DEV_CODE).await.status(), 200);
    assert_eq!(verify_code(&client, PHONE, DEV_CODE).await.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn logout_requires_auth_and_returns_204(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    assert_eq!(
        client
            .post("/v1/auth/logout")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    request_code(&client, PHONE).await;
    let body: Value = verify_code(&client, PHONE, DEV_CODE)
        .await
        .json()
        .await
        .unwrap();
    let token = body["token"].as_str().unwrap().to_string();

    let response = client
        .post("/v1/auth/logout")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn otp_request_rejects_a_disallowed_origin(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    // TUMA_CORS_ORIGIN is unset in tests, so any browser Origin is rejected.
    let response = client
        .post_json("/v1/auth/otp/request", json!({ "phone": PHONE }))
        .header("Origin", "http://evil.example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn invalid_phone_numbers_fail_validation(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    for phone in ["12345", "not-a-phone", "+250780111000123456"] {
        let response = request_code(&client, phone).await;
        assert_eq!(response.status(), 422, "phone {phone} should be rejected");
    }
}
