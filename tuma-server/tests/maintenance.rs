//! The maintenance prune (the system's first background job): GPS
//! breadcrumbs and dead refresh tokens are deleted once they outlive the
//! retention window. The scheduler itself lives in `main.rs` (hourly,
//! startup tick) — these tests prove the two domain functions it calls,
//! including exactly-what-dies semantics at the cutoff.

mod common;

use common::{
    MIGRATOR, TestClient, seed_customer, seed_merchant, seed_rider, spawn_app, token_for,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

struct Operator {
    client: TestClient,
    token: String,
}

async fn owner(app: &common::TestApp, email: &str, business: &str) -> Operator {
    let seeded = seed_merchant(&app.pool, email, business).await;
    Operator {
        client: TestClient::new(&app.address),
        token: token_for(app, seeded.account.id, 3600),
    }
}

async fn customer_session(app: &common::TestApp, phone: &str) -> (TestClient, String) {
    let seeded = seed_customer(&app.pool, phone).await;
    (
        TestClient::new(&app.address),
        token_for(app, seeded.account.id, 3600),
    )
}

async fn create_store(op: &Operator, name: &str) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/stores",
            json!({ "name": name, "delivery_fee": 1500, "lat": -1.9620, "lng": 30.1290 }),
        )
        .bearer_auth(&op.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn create_product(op: &Operator, name: &str) -> Uuid {
    let response = op
        .client
        .post_json("/v1/merchant/products", json!({ "name": name }))
        .bearer_auth(&op.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn attach(op: &Operator, store_id: Uuid, product_id: Uuid, price: i64) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store_id, "price": price, "stock": null }),
        )
        .bearer_auth(&op.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn set_open(pool: &PgPool, store_id: Uuid, open: bool) {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("UPDATE marketplace.stores SET is_open = $2 WHERE id = $1")
        .bind(store_id)
        .bind(open)
        .execute(&mut *conn)
        .await
        .unwrap();
}

fn line(store_product_id: Uuid, quantity: i32) -> Value {
    json!({ "store_product_id": store_product_id, "quantity": quantity })
}

/// The delivery id behind a store order.
async fn delivery_of(pool: &PgPool, store_order_id: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT id FROM commerce.deliveries WHERE store_order_id = $1")
        .bind(store_order_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn pruning_removes_old_breadcrumbs_and_dead_tokens(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com", "Aline's Kitchen").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000).await;
    set_open(&app.pool, store, true).await;

    // A real checkout, advanced to preparing and handed to a real rider —
    // the delivery (and its breadcrumb trail) exists because the flow ran.
    let (chantal, token) = customer_session(&app, "+250780000033").await;
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "address_lat": -1.9499,
                "address_lng": 30.0622,
                "items": [line(rice_sp, 1)]
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let group: Value = response.json().await.unwrap();
    let order_id: Uuid = group["store_orders"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let jean = seed_rider(&app.pool, "Jean", "+250780000034").await;
    // accept → preparing → handoff (picked_up).
    for status in ["accepted", "preparing"] {
        let response = aline
            .client
            .patch_json(
                &format!("/v1/merchant/store-orders/{order_id}"),
                json!({ "status": status }),
            )
            .bearer_auth(&aline.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "advance to {status}");
    }
    let response = aline
        .client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": jean.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let delivery_id = delivery_of(&app.pool, order_id).await;

    let now = OffsetDateTime::now_utc();
    let forty_days_ago = now - Duration::days(40);

    // Two breadcrumbs past the window, one fresh. `recorded_at` is the
    // backdated column — a real push can't travel back in time.
    let mut conn = pool.acquire().await.unwrap();
    for recorded_at in [forty_days_ago, forty_days_ago, now] {
        sqlx::query(
            "INSERT INTO commerce.delivery_locations (delivery_id, lat, lng, recorded_at) \
             VALUES ($1, -1.9620, 30.1290, $2)",
        )
        .bind(delivery_id)
        .bind(recorded_at)
        .execute(&mut *conn)
        .await
        .unwrap();
    }

    // Three refresh tokens on the customer's account: one live (expires in
    // 20 days), one expired, one revoked — both dead ones backdated past
    // the 30-day window, since a token dead for only 5 days is inside the
    // retention window and must survive (the kept-recently-dead grace).
    let user_id = chantal_id(&app.pool, "+250780000033").await;
    let tokens = [
        // (token_hash, expires_at, revoked_at)
        ("live-token-hash", now + Duration::days(20), None),
        ("expired-token-hash", forty_days_ago, None),
        (
            "revoked-token-hash",
            now + Duration::days(20),
            Some(forty_days_ago),
        ),
    ];
    for (hash, expires_at, revoked_at) in tokens {
        sqlx::query(
            "INSERT INTO accounts.refresh_tokens (token_hash, user_id, expires_at, revoked_at, created_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(hash)
        .bind(user_id)
        .bind(expires_at)
        .bind(revoked_at)
        .bind(forty_days_ago)
        .execute(&mut *conn)
        .await
        .unwrap();
    }

    // The prune with the default 30-day retention.
    let cutoff = now - Duration::days(30);
    let deleted_locations = commerce::deliveries::prune_old_locations(&mut conn, cutoff)
        .await
        .unwrap();
    let deleted_tokens = accounts::refresh_tokens::prune_dead_tokens(&mut conn, cutoff)
        .await
        .unwrap();
    assert_eq!(deleted_locations, 2, "the two 40-day-old breadcrumbs died");
    assert_eq!(deleted_tokens, 2, "expired + long-revoked tokens died");

    let (fresh_breadcrumbs,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM commerce.delivery_locations WHERE delivery_id = $1")
            .bind(delivery_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(fresh_breadcrumbs, 1, "the fresh breadcrumb survives");

    let (surviving_tokens,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM accounts.refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(surviving_tokens, 1, "only the live token survives");
    let (survivor_hash,): (String,) =
        sqlx::query_as("SELECT token_hash FROM accounts.refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(survivor_hash, "live-token-hash");
}

/// The account id behind a seeded phone.
async fn chantal_id(pool: &PgPool, phone: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM accounts.users WHERE phone = $1")
        .bind(phone)
        .fetch_one(pool)
        .await
        .unwrap()
}
