//! Stores + products (S9, multi-store since S10b): merchants can have
//! several stores, products join a chosen store, and the customer catalog
//! shows open stores — ownership resolved server-side from the signed-in
//! user.

mod common;

use accounts::UserRole;
use common::{MIGRATOR, TestClient, seed_customer, seed_staff, spawn_app, token_for};
use serde_json::{Value, json};
use uuid::Uuid;

/// A signed-in session on its own client.
struct Session {
    client: TestClient,
    token: String,
    user_id: Uuid,
}

async fn merchant(app: &common::TestApp, email: &str) -> Session {
    let user = seed_staff(&app.pool, UserRole::Merchant, email).await;
    Session {
        client: TestClient::new(&app.address),
        token: token_for(app, &user, 3600),
        user_id: user.id,
    }
}

async fn customer(app: &common::TestApp, phone: &str) -> Session {
    let user = seed_customer(&app.pool, phone).await;
    Session {
        client: TestClient::new(&app.address),
        token: token_for(app, &user, 3600),
        user_id: user.id,
    }
}

fn store_input() -> Value {
    json!({
        "name": "Aline's Kitchen",
        "description": "Fresh food, fast.",
        "address_text": "KN 4 Ave, Kigali",
        "lat": -1.9512,
        "lng": 30.0623,
        "delivery_fee": 1500
    })
}

/// Create a store and return its id.
async fn create_store(session: &Session, input: Value) -> Uuid {
    let response = session
        .client
        .post_json("/v1/merchant/stores", input)
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

/// Add a product to one of the session's stores and return its id.
async fn create_product(session: &Session, store_id: Uuid, input: Value) -> Uuid {
    let mut body = input;
    body["store_id"] = json!(store_id.to_string());
    let response = session
        .client
        .post_json("/v1/merchant/products", body)
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_creates_a_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;

    let response = aline
        .client
        .post_json("/v1/merchant/stores", store_input())
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Aline's Kitchen");
    assert_eq!(body["description"], "Fresh food, fast.");
    assert_eq!(body["address_text"], "KN 4 Ave, Kigali");
    assert_eq!(body["lat"], json!(-1.9512));
    assert_eq!(body["lng"], json!(30.0623));
    assert_eq!(body["delivery_fee"], 1500); // integer RWF round-trips
    assert_eq!(body["is_open"], false); // new stores start closed
    assert_eq!(body["merchant_id"], aline.user_id.to_string());
    assert!(body["created_at"].is_string());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_can_have_multiple_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;

    let first = create_store(&aline, store_input()).await;
    let second = create_store(&aline, json!({ "name": "Aline's Café" })).await;
    assert_ne!(first, second);

    let response = aline
        .client
        .get("/v1/merchant/stores")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores.len(), 2); // both stores, oldest first
    assert_eq!(stores[0]["id"], first.to_string());
    assert_eq!(stores[0]["name"], "Aline's Kitchen");
    assert_eq!(stores[1]["id"], second.to_string());
    assert_eq!(stores[1]["name"], "Aline's Café");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn create_store_rejects_bad_input(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;

    // Negative delivery fee violates the integer-RWF rule.
    let response = aline
        .client
        .post_json(
            "/v1/merchant/stores",
            json!({ "name": "Bad", "delivery_fee": -100 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);

    // Empty name fails validation.
    let response = aline
        .client
        .post_json("/v1/merchant/stores", json!({ "name": "" }))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_gets_a_store_by_id(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;

    // Unknown id.
    let response = aline
        .client
        .get(&format!("/v1/merchant/stores/{}", Uuid::new_v4()))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    let store_id = create_store(&aline, store_input()).await;

    // Own store.
    let response = aline
        .client
        .get(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["id"], store_id.to_string());
    assert_eq!(body["name"], "Aline's Kitchen");

    // Another merchant's store looks like a missing one.
    let response = bruce
        .client
        .get(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_updates_their_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let store_id = create_store(&aline, store_input()).await;

    // Provided fields overwrite; empty description clears it; absent fields
    // (address, location) stay put.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/stores/{store_id}"),
            json!({
                "name": "Aline's Kitchen 2.0",
                "description": "",
                "delivery_fee": 2000,
                "is_open": true
            }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Aline's Kitchen 2.0");
    assert!(body["description"].is_null());
    assert_eq!(body["delivery_fee"], 2000);
    assert_eq!(body["is_open"], true);
    assert_eq!(body["address_text"], "KN 4 Ave, Kigali");
    assert_eq!(body["lat"], json!(-1.9512));

    // Another merchant cannot update it.
    let response = bruce
        .client
        .patch_json(
            &format!("/v1/merchant/stores/{store_id}"),
            json!({ "name": "Bruce's now" }),
        )
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_manages_their_menu(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let kitchen = create_store(&aline, store_input()).await;
    let cafe = create_store(&aline, json!({ "name": "Aline's Café" })).await;

    let product_id = create_product(
        &aline,
        kitchen,
        json!({ "name": "Ibirazi", "description": "Rice & beans", "price": 3500 }),
    )
    .await;
    create_product(&aline, cafe, json!({ "name": "Coffee", "price": 1200 })).await;

    // The menu spans all of the merchant's stores.
    let response = aline
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let menu: Value = response.json().await.unwrap();
    let menu = menu.as_array().unwrap();
    assert_eq!(menu.len(), 2);
    assert_eq!(menu[0]["name"], "Ibirazi");
    assert_eq!(menu[0]["price"], 3500); // integer RWF
    assert_eq!(menu[0]["is_available"], true); // default
    assert_eq!(menu[0]["store_id"], kitchen.to_string());
    assert_eq!(menu[1]["store_id"], cafe.to_string());

    // Update price + availability; untouched fields stay put.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/products/{product_id}"),
            json!({ "price": 4000, "is_available": false }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["price"], 4000);
    assert_eq!(body["is_available"], false);
    assert_eq!(body["name"], "Ibirazi");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn products_require_a_valid_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let bruce_store = create_store(&bruce, json!({ "name": "Bruce's" })).await;

    // Unknown store id.
    let response = aline
        .client
        .post_json(
            "/v1/merchant/products",
            json!({ "store_id": Uuid::new_v4().to_string(), "name": "Orphan", "price": 1000 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "store not found");

    // Another merchant's store id looks the same.
    let response = aline
        .client
        .post_json(
            "/v1/merchant/products",
            json!({ "store_id": bruce_store.to_string(), "name": "Sneaky", "price": 1000 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "store not found");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchants_cannot_touch_each_others_products(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let aline_store = create_store(&aline, store_input()).await;

    let product_id = create_product(
        &aline,
        aline_store,
        json!({ "name": "Aline's dish", "price": 3500 }),
    )
    .await;

    // Bruce cannot update Aline's product — it looks like a missing one.
    let response = bruce
        .client
        .patch_json(
            &format!("/v1/merchant/products/{product_id}"),
            json!({ "price": 1 }),
        )
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // And Bruce's menu is empty (he has no stores at all).
    let response = bruce
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let menu: Value = response.json().await.unwrap();
    assert_eq!(menu, json!([]));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_deletes_stores_and_products(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let store_id = create_store(&aline, store_input()).await;
    let kept = create_product(&aline, store_id, json!({ "name": "Kept", "price": 1200 })).await;
    let gone = create_product(&aline, store_id, json!({ "name": "Gone", "price": 3500 })).await;
    let bruce_store = create_store(&bruce, store_input()).await;

    // Bruce cannot delete Aline's store or product — it looks missing.
    for path in [
        format!("/v1/merchant/stores/{store_id}"),
        format!("/v1/merchant/products/{gone}"),
    ] {
        let response = delete_resource(&bruce, &path).await;
        assert_eq!(response.status(), 404, "bruce must not delete {path}");
    }

    // Aline deletes one product: 204, and the menu keeps only the other.
    let response = delete_resource(&aline, &format!("/v1/merchant/products/{gone}")).await;
    assert_eq!(response.status(), 204);
    let response = aline
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    let menu: Value = response.json().await.unwrap();
    let menu = menu.as_array().unwrap();
    assert_eq!(menu.len(), 1);
    assert_eq!(menu[0]["id"], kept.to_string());

    // Deleting the store cascades the remaining product away.
    let response = delete_resource(&aline, &format!("/v1/merchant/stores/{store_id}")).await;
    assert_eq!(response.status(), 204);
    let response = aline
        .client
        .get(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let response = aline
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    let menu: Value = response.json().await.unwrap();
    assert_eq!(menu, json!([]));

    // Unknown ids are 404s; Bruce's store survived untouched.
    let response =
        delete_resource(&aline, &format!("/v1/merchant/stores/{}", Uuid::new_v4())).await;
    assert_eq!(response.status(), 404);
    let response =
        delete_resource(&aline, &format!("/v1/merchant/products/{}", Uuid::new_v4())).await;
    assert_eq!(response.status(), 404);
    let response = bruce
        .client
        .get(&format!("/v1/merchant/stores/{bruce_store}"))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

/// A DELETE on the session's client with its bearer token.
async fn delete_resource(session: &Session, path: &str) -> reqwest::Response {
    session
        .client
        .delete(path)
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap()
}

/// Open a store and optionally seed its menu. Returns the store id.
async fn open_store(session: &Session, name: &str, products: &[(&str, i64, bool)]) -> Uuid {
    let store_id = create_store(session, json!({ "name": name })).await;
    session
        .client
        .patch_json(
            &format!("/v1/merchant/stores/{store_id}"),
            json!({ "is_open": true }),
        )
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    for (product_name, price, available) in products {
        create_product(
            session,
            store_id,
            json!({ "name": product_name, "price": price, "is_available": available }),
        )
        .await;
    }
    store_id
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_sees_only_open_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let chantal = customer(&app, "+250780000001").await;

    open_store(&aline, "Open kitchen", &[]).await;
    create_store(&bruce, json!({ "name": "Closed kitchen" })).await; // stays closed

    let response = chantal
        .client
        .get("/v1/stores")
        .bearer_auth(&chantal.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0]["name"], "Open kitchen");
    assert_eq!(stores[0]["is_open"], true);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_store_detail_shows_only_available_products(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let bruce = merchant(&app, "bruce@example.com").await;
    let chantal = customer(&app, "+250780000001").await;

    let open_id = open_store(
        &aline,
        "Open kitchen",
        &[("Ibirazi", 3500, true), ("Sold out", 5000, false)],
    )
    .await;
    let closed_id = create_store(&bruce, json!({ "name": "Closed kitchen" })).await;

    let response = chantal
        .client
        .get(&format!("/v1/stores/{open_id}"))
        .bearer_auth(&chantal.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["store"]["name"], "Open kitchen");
    let products = body["products"].as_array().unwrap();
    assert_eq!(products.len(), 1); // unavailable product hidden
    assert_eq!(products[0]["name"], "Ibirazi");
    assert_eq!(products[0]["price"], 3500);

    // A closed store is indistinguishable from a missing one.
    let response = chantal
        .client
        .get(&format!("/v1/stores/{closed_id}"))
        .bearer_auth(&chantal.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn audiences_are_role_gated(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = merchant(&app, "aline@example.com").await;
    let chantal = customer(&app, "+250780000001").await;

    // Customers cannot enter the merchant namespace.
    let response = chantal
        .client
        .post_json("/v1/merchant/stores", store_input())
        .bearer_auth(&chantal.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // Merchants cannot browse the customer catalog.
    let response = aline
        .client
        .get("/v1/stores")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // Anonymous gets 401 on both.
    let anonymous = TestClient::new(&app.address);
    let response = anonymous.get("/v1/stores").send().await.unwrap();
    assert_eq!(response.status(), 401);
    let response = anonymous.get("/v1/merchant/stores").send().await.unwrap();
    assert_eq!(response.status(), 401);
}
