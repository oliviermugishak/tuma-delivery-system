//! The cookie transport: email + password login, silent refresh, logout
//! revocation, and password changes.

mod common;

use accounts::jwt;
use common::{
    MIGRATOR, TestClient, cookie_value, login, seed_customer, seed_merchant, set_cookie_lines,
    spawn_app, token_for,
};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tuma_server::middleware::{AUTH_COOKIE, REFRESH_COOKIE};

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_sets_session_cookies_and_grants_access(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    // Mixed-case email on purpose: login must be case-insensitive.
    let response = login(&client, "Merchant@Example.com", "Password123").await;
    assert_eq!(response.status(), 204);

    let auth = cookie_value(&response, AUTH_COOKIE).expect("access cookie set");
    let refresh = cookie_value(&response, REFRESH_COOKIE).expect("refresh cookie set");
    assert!(!auth.is_empty());
    assert!(!refresh.is_empty());

    for name in [AUTH_COOKIE, REFRESH_COOKIE] {
        let line = set_cookie_lines(&response)
            .into_iter()
            .find(|line| line.starts_with(&format!("{name}=")))
            .expect("both cookies announced");
        assert!(line.contains("Path=/api"), "cookie scoped to /api: {line}");
        assert!(line.contains("HttpOnly"), "cookie is httpOnly: {line}");
        assert!(line.contains("SameSite=Lax"), "cookie is Lax: {line}");
    }

    // The cookie store picked the session up — no bearer header anywhere.
    let response = client.get("/v1/me").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["id"], seeded.account.id.to_string());
    assert_eq!(body["email"], "merchant@example.com");
    let memberships = body["merchant_memberships"].as_array().unwrap();
    assert_eq!(memberships[0]["merchant_name"], "Aline's Kitchen");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_rejects_bad_credentials_with_one_generic_error(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    // Wrong password and unknown email must be indistinguishable.
    for (email, password) in [
        ("merchant@example.com", "WrongPassword1"),
        ("nobody@example.com", "Password123"),
    ] {
        let response = login(&client, email, password).await;
        assert_eq!(response.status(), 401, "rejected: {email}");
        assert!(
            cookie_value(&response, AUTH_COOKIE).is_none(),
            "no session cookie on failure"
        );
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["message"], "Invalid email or password");
    }

    // And nothing was left behind in the client's cookie store.
    let response = client.get("/v1/me").send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_rejects_a_password_account_without_platform_authorization(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    // A bare password account — no admin profile, no merchant membership —
    // must not enter the platform even with the right password.
    let mut conn = app.pool.acquire().await.unwrap();
    let bare = accounts::AccountManager::new(1)
        .create_password_account(&mut conn, "bare@example.com", "Password123")
        .await
        .unwrap();
    drop(conn);

    let response = login(&client, "bare@example.com", "Password123").await;
    assert_eq!(response.status(), 401);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Invalid email or password");
    let _ = bare;
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_rejects_deactivated_accounts(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    sqlx::query("UPDATE accounts.users SET is_active = false WHERE id = $1")
        .bind(seeded.account.id)
        .execute(&app.pool)
        .await
        .unwrap();

    // Same generic 401 — deactivation must not be distinguishable from
    // bad credentials.
    let response = login(&client, "merchant@example.com", "Password123").await;
    assert_eq!(response.status(), 401);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Invalid email or password");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn logout_revokes_the_refresh_session(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    let response = login(&client, "merchant@example.com", "Password123").await;
    let refresh = cookie_value(&response, REFRESH_COOKIE).expect("refresh cookie set");
    assert_eq!(client.get("/v1/me").send().await.unwrap().status(), 200);

    let response = client.post("/v1/auth/logout").send().await.unwrap();
    assert_eq!(response.status(), 204);
    assert!(
        cookie_value(&response, AUTH_COOKIE).is_some_and(|v| v.is_empty()),
        "access cookie cleared"
    );
    assert!(
        cookie_value(&response, REFRESH_COOKIE).is_some_and(|v| v.is_empty()),
        "refresh cookie cleared"
    );

    // The session is dead even for a client that kept the old refresh
    // cookie: it was revoked server-side, so no new access cookie is minted.
    let fresh = TestClient::new(&app.address);
    let response = fresh
        .get("/v1/me")
        .header("Cookie", format!("{REFRESH_COOKIE}={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert!(cookie_value(&response, AUTH_COOKIE).is_none());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn silent_refresh_remints_an_expired_token_cookie(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let seeded = seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    let response = login(
        &TestClient::new(&app.address),
        "merchant@example.com",
        "Password123",
    )
    .await;
    let refresh = cookie_value(&response, REFRESH_COOKIE).expect("refresh cookie set");

    // Expired an hour ago — well beyond the 60s verification leeway.
    let mut claims = jwt::Claims::new(seeded.account.id, 3600);
    claims.exp = claims.iat - 3600;
    let expired = jwt::generate(
        &claims,
        app.config.secret.jwt_signing_key.expose_secret().as_bytes(),
    )
    .unwrap();

    // Expired access cookie + live refresh cookie → request succeeds and
    // the response carries a fresh access cookie.
    let client = TestClient::new(&app.address);
    let response = client
        .get("/v1/me")
        .header(
            "Cookie",
            format!("{AUTH_COOKIE}={expired}; {REFRESH_COOKIE}={refresh}"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let reminted = cookie_value(&response, AUTH_COOKIE).expect("access cookie reminted");
    assert!(!reminted.is_empty());
    assert_ne!(reminted, expired);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["id"], seeded.account.id.to_string());

    // Same story when the access cookie is gone entirely.
    let response = client
        .get("/v1/me")
        .header("Cookie", format!("{REFRESH_COOKIE}={refresh}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(cookie_value(&response, AUTH_COOKIE).is_some());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn change_password_swaps_the_password(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;
    assert_eq!(
        login(&client, "merchant@example.com", "Password123")
            .await
            .status(),
        204
    );

    let response = client
        .post_json(
            "/v1/auth/password",
            json!({ "current_password": "Password123", "new_password": "NewPassword456" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "merchant@example.com",
            "Password123"
        )
        .await
        .status(),
        401,
        "old password is dead"
    );
    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "merchant@example.com",
            "NewPassword456"
        )
        .await
        .status(),
        204,
        "new password works"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn change_password_rejects_a_wrong_current_password(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;
    assert_eq!(
        login(&client, "merchant@example.com", "Password123")
            .await
            .status(),
        204
    );

    let response = client
        .post_json(
            "/v1/auth/password",
            json!({ "current_password": "WrongCurrent1", "new_password": "NewPassword456" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    // The real password is untouched.
    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "merchant@example.com",
            "Password123"
        )
        .await
        .status(),
        204
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn change_password_rejects_accounts_without_a_password(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780001001").await;
    let token = token_for(&app, seeded.account.id, 3600);

    // Customers sign in with OTP — there is no password to change.
    let response = client
        .post_json(
            "/v1/auth/password",
            json!({ "current_password": "Anything123", "new_password": "NewPassword456" }),
        )
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}
