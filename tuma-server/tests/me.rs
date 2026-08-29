mod common;

use accounts::jwt;
use common::{MIGRATOR, TestClient, seed_customer, seed_merchant, spawn_app, token_for};
use secrecy::ExposeSecret;
use serde_json::{Value, json};

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_without_credentials_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = client.get("/v1/me").send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_with_garbage_token_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = client
        .get("/v1/me")
        .bearer_auth("not-a-jwt")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_with_expired_token_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000002").await;

    // Expired an hour ago — well beyond the 60s verification leeway.
    let mut claims = jwt::Claims::new(seeded.account.id, 3600);
    claims.exp = claims.iat - 3600;
    let token = jwt::generate(
        &claims,
        app.config.secret.jwt_signing_key.expose_secret().as_bytes(),
    )
    .unwrap();

    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_with_valid_bearer_token_returns_the_account_and_profile(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000003").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["id"], seeded.account.id.to_string());
    assert_eq!(body["phone"], "+250780000003");
    assert_eq!(body["customer"]["id"], seeded.customer.id.to_string());
    // The account is roleless: no role field exists to mistake for one.
    assert!(body.get("role").is_none());
    // The password hash must never leak into responses.
    assert!(body.get("password_hash").is_none());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_shows_a_merchant_operators_business_and_role(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert!(
        body["customer"].is_null(),
        "operators have no customer profile"
    );
    let memberships = body["merchant_memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 1);
    assert_eq!(
        memberships[0]["merchant_id"],
        seeded.merchant.id.to_string()
    );
    assert_eq!(memberships[0]["merchant_name"], "Aline's Kitchen");
    assert_eq!(memberships[0]["role"], "owner");
    assert!(
        memberships[0]["store_id"].is_null(),
        "owners are not store-scoped"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_works_through_the_cookie_transport_too(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000004").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .get("/v1/me")
        .header("Cookie", format!("tuma-auth-token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["id"], seeded.account.id.to_string());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn me_is_rejected_for_a_deactivated_account(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000005").await;
    let token = token_for(&app, seeded.account.id, 3600);

    sqlx::query("UPDATE accounts.users SET is_active = false WHERE id = $1")
        .bind(seeded.account.id)
        .execute(&app.pool)
        .await
        .unwrap();

    // The token is still cryptographically valid — the fresh account lookup
    // in the auth middleware must still turn it away.
    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_updates_the_customers_name(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000006").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .patch_json("/v1/me", json!({ "name": "  Aline  " }))
        .bearer_auth(token.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["customer"]["name"], "Aline"); // stored trimmed

    // And the change is persisted, not just echoed.
    let response = client
        .get("/v1/me")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["customer"]["name"], "Aline");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_updates_an_admins_name(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = common::seed_admin(&app.pool, "admin@example.com").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .patch_json("/v1/me", json!({ "name": "Platform Boss" }))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["admin"]["name"], "Platform Boss");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_without_credentials_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = client
        .patch_json("/v1/me", json!({ "name": "Aline" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_rejects_a_whitespace_only_name(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000007").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .patch_json("/v1/me", json!({ "name": "   " }))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_rejects_a_missing_name(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000008").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .patch_json("/v1/me", json!({}))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn patch_me_rejects_an_operator_without_an_editable_profile(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let token = token_for(&app, seeded.account.id, 3600);

    // Merchant operators have no editable name in V1 — their wing identity
    // is the business.
    let response = client
        .patch_json("/v1/me", json!({ "name": "Someone" }))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}
