mod common;

use common::{MIGRATOR, TestClient, spawn_app};

#[sqlx::test(migrator = "MIGRATOR")]
async fn health_check_returns_ok(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(app.address);

    let response = client
        .get("/health")
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");
}
