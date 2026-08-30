//! Discovery — `GET /v1/search`: product search across open stores, the
//! popular shelf from real order counts, and the validation contract
//! shared with the store feed. Closed stores and unavailable items are
//! invisible in both sections, by read-side rule.

mod common;

use common::{MIGRATOR, TestClient, seed_customer, seed_merchant, spawn_app, token_for};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// A signed-in operator session on its own client.
struct Operator {
    client: TestClient,
    token: String,
    #[allow(dead_code)]
    merchant_id: Uuid,
}

async fn owner(app: &common::TestApp, email: &str) -> Operator {
    let seeded = seed_merchant(&app.pool, email, "Aline's Kitchen").await;
    Operator {
        client: TestClient::new(&app.address),
        token: token_for(app, seeded.account.id, 3600),
        merchant_id: seeded.merchant.id,
    }
}

async fn customer_session(app: &common::TestApp, phone: &str) -> (TestClient, String) {
    let seeded = seed_customer(&app.pool, phone).await;
    (
        TestClient::new(&app.address),
        token_for(app, seeded.account.id, 3600),
    )
}

async fn create_store(session: &Operator, name: &str) -> Uuid {
    let response = session
        .client
        .post_json("/v1/merchant/stores", json!({ "name": name }))
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "store {name} created");
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

async fn create_product(session: &Operator, name: &str) -> Uuid {
    let response = session
        .client
        .post_json(
            "/v1/merchant/products",
            json!({ "name": name, "description": format!("About {name}") }),
        )
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "product {name} created");
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

/// Attach a catalog product to a store at a price and return the
/// store_product id (untracked stock — made-to-order).
async fn attach(session: &Operator, store_id: Uuid, product_id: Uuid, price: i64) -> Uuid {
    let response = session
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "store_id": store_id, "product_id": product_id, "price": price, "stock": null }),
        )
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "attached to {store_id}");
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

async fn open(pool: &PgPool, store_id: Uuid, is_open: bool) {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("UPDATE marketplace.stores SET is_open = $2 WHERE id = $1")
        .bind(store_id)
        .bind(is_open)
        .execute(&mut *conn)
        .await
        .unwrap();
}

async fn checkout(client: &TestClient, token: &str, items: Value) {
    let response = client
        .post_json(
            "/v1/orders",
            json!({ "address_text": "KN 4 Ave, Kigali", "items": items }),
        )
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
}

fn line(store_product_id: Uuid, quantity: i32) -> Value {
    json!({ "store_product_id": store_product_id, "quantity": quantity })
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn search_matches_products_across_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    let remera = create_store(&aline, "Aline Remera").await;
    let kimironko = create_store(&aline, "Aline Kimironko").await;
    let closed = create_store(&aline, "Aline Closed").await;
    open(&app.pool, remera, true).await;
    open(&app.pool, kimironko, true).await;

    // One catalog product, two stores, branch pricing — the discovery
    // result must carry each store's own price.
    let inyama = create_product(&aline, "Inyama Yishyu").await;
    let remera_inyama = attach(&aline, remera, inyama, 5000).await;
    let kimironko_inyama = attach(&aline, kimironko, inyama, 5500).await;
    let _fanta = attach(
        &aline,
        remera,
        create_product(&aline, "Fanta Orange").await,
        800,
    )
    .await;
    // Matches by name but its store is closed — invisible.
    let _secret = attach(
        &aline,
        closed,
        create_product(&aline, "Secret Stew").await,
        9000,
    )
    .await;

    let (chantal, token) = customer_session(&app, "+250780000030").await;

    let response = chantal
        .get("/v1/search?q=inyama")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    let products = body["products"].as_array().unwrap();
    assert_eq!(products.len(), 2, "both branches, closed store excluded");

    let by_price = |id: &Uuid| {
        products
            .iter()
            .find(|p| p["store_product_id"] == id.to_string())
            .unwrap()
            .clone()
    };
    assert_eq!(by_price(&remera_inyama)["price"], 5000);
    assert_eq!(by_price(&remera_inyama)["store_name"], "Aline Remera");
    assert_eq!(by_price(&kimironko_inyama)["price"], 5500);
    assert_eq!(by_price(&kimironko_inyama)["store_name"], "Aline Kimironko");
    // The fulfilling store's id rides along — the store is the
    // destination when a customer taps a product.
    assert_eq!(by_price(&remera_inyama)["store_id"], remera.to_string());

    // Stores section honors the same term: no store is called inyama.
    assert!(body["stores"].as_array().unwrap().is_empty());

    // No match at all: empty sections, not an error.
    let response = chantal
        .get("/v1/search?q=zzz")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert!(body["products"].as_array().unwrap().is_empty());
    assert!(body["stores"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn empty_query_shows_popular_products_and_open_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    let remera = create_store(&aline, "Aline Remera").await;
    let kimironko = create_store(&aline, "Aline Kimironko").await;
    open(&app.pool, remera, true).await;
    open(&app.pool, kimironko, true).await;

    let brochette = create_product(&aline, "Brochette").await;
    let brochette_sp = attach(&aline, kimironko, brochette, 3000).await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, remera, rice, 12000).await;
    let tea = create_product(&aline, "Tea").await;
    let _tea_sp = attach(&aline, remera, tea, 500).await;

    let (chantal, token) = customer_session(&app, "+250780000031").await;
    // Real purchases through the real checkout: brochette twice, rice once.
    checkout(&chantal, &token, json!([line(brochette_sp, 1)])).await;
    checkout(&chantal, &token, json!([line(brochette_sp, 2)])).await;
    checkout(&chantal, &token, json!([line(rice_sp, 1)])).await;

    // An ordered product whose store later closes falls off discovery —
    // the open-store rule is a read-side fact, not a write-time one.
    let closed_store = create_store(&aline, "Aline Night").await;
    open(&app.pool, closed_store, true).await;
    let night_sp = attach(
        &aline,
        closed_store,
        create_product(&aline, "Night Tea").await,
        700,
    )
    .await;
    checkout(&chantal, &token, json!([line(night_sp, 3)])).await;
    open(&app.pool, closed_store, false).await;

    // The default discovery state: no q.
    let response = chantal
        .get("/v1/search")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();

    let products = body["products"].as_array().unwrap();
    let names: Vec<&str> = products
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    // Most purchases first; Tea (zero orders) and Night Tea (closed
    // store) never appear.
    assert_eq!(names, vec!["Brochette", "Rice 5KG"]);
    assert_eq!(products[0]["price"], 3000);

    // Stores: both open ones, closed ones absent.
    let stores = body["stores"].as_array().unwrap();
    let store_names: Vec<&str> = stores.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(store_names.len(), 2);
    assert!(store_names.contains(&"Aline Remera"));
    assert!(store_names.contains(&"Aline Kimironko"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn search_validates_like_the_feed(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, "Aline Remera").await;
    open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000032").await;
    for bad in [
        "?lat=-1.96",
        "?lng=30.12",
        "?lat=-999&lng=30.12",
        "?lat=abc&lng=30.12",
        &format!("?q={}", "a".repeat(101)),
    ] {
        let response = chantal
            .get(&format!("/v1/search{bad}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "bad discovery query {bad}");
    }

    // A merchant is not a customer — the discovery shelf is the
    // customers' surface.
    let response = aline
        .client
        .get("/v1/search")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}
