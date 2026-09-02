//! Stores — the fulfillment boundary. Merchant operators manage their
//! business's stores through membership-scoped endpoints; customers browse
//! open stores. Ownership is resolved server-side from memberships.

mod common;

use common::{
    MIGRATOR, TestClient, login, seed_customer, seed_merchant, seed_store_manager, spawn_app,
    token_for,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// A signed-in operator session on its own client.
struct Operator {
    client: TestClient,
    token: String,
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

fn store_input() -> Value {
    json!({
        "name": "Aline's Kitchen",
        "description": "Fresh food, fast.",
        "address_text": "KN 4 Ave, Kigali",
        "lat": -1.9512,
        "lng": 30.0623,
        "category": "Grill",
        "delivery_fee": 1500
    })
}

/// Create a store and return its id.
async fn create_store(session: &Operator, input: Value) -> Uuid {
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn owner_creates_a_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;

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
    assert_eq!(body["category"], "Grill");
    assert_eq!(body["delivery_fee"], 1500); // integer RWF round-trips
    assert_eq!(body["is_open"], false); // new stores start closed
    // The store belongs to the BUSINESS, not the operator's account.
    assert_eq!(body["merchant_id"], aline.merchant_id.to_string());
    assert!(body["created_at"].is_string());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn owner_can_have_multiple_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;

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
    let aline = owner(&app, "aline@example.com").await;

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
async fn owner_gets_a_store_by_id_and_a_foreign_store_404s(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let bruce = owner(&app, "bruce@example.com").await;

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

    // Another business's store looks like a missing one.
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
async fn owner_updates_their_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let bruce = owner(&app, "bruce@example.com").await;
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
                "category": "Grill House",
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
    assert_eq!(body["category"], "Grill House");
    assert_eq!(body["delivery_fee"], 2000);
    assert_eq!(body["is_open"], true);
    assert_eq!(body["address_text"], "KN 4 Ave, Kigali");
    assert_eq!(body["lat"], json!(-1.9512));

    // Another business cannot update it.
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
async fn owner_deletes_a_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let bruce = owner(&app, "bruce@example.com").await;
    let store_id = create_store(&aline, store_input()).await;
    let bruce_store = create_store(&bruce, json!({ "name": "Bruce's" })).await;

    // Bruce cannot delete Aline's store — it looks missing.
    let response = bruce
        .client
        .delete(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // Aline deletes her store; it stops existing.
    let response = aline
        .client
        .delete(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let response = aline
        .client
        .get(&format!("/v1/merchant/stores/{store_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // Unknown ids are 404s; Bruce's store survived untouched.
    let response = aline
        .client
        .delete(&format!("/v1/merchant/stores/{}", Uuid::new_v4()))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
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

/// One customer order through this store — the order history a delete
/// must refuse to rewrite.
async fn place_order_at(app: &common::TestApp, store_id: Uuid) {
    let (store_product_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM marketplace.store_products WHERE store_id = $1 LIMIT 1")
            .bind(store_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let seeded = seed_customer(&app.pool, "+250780099002").await;
    let token = token_for(app, seeded.account.id, 3600);
    let response = TestClient::new(&app.address)
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KN 4 Ave, Kigali",
                "items": [{ "store_product_id": store_product_id, "quantity": 1 }]
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "the order history exists");
}

/// S28: the store_orders FK has no ON DELETE — before the typed
/// pre-check, deleting a store that ever appeared in an order was a 500
/// from the constraint. Now it is a 409 naming the remedy (close it).
#[sqlx::test(migrator = "MIGRATOR")]
async fn deleting_a_store_with_order_history_is_a_409_not_a_500(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, store_input()).await;

    // Stock the store, open it, and take one real order through it.
    let product = aline
        .client
        .post_json("/v1/merchant/products", json!({ "name": "Rice 5KG" }))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(product.status(), 201);
    let product_id: Uuid = product.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let attach = aline
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store, "price": 5000, "stock": 5 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), 201);
    sqlx::query("UPDATE marketplace.stores SET is_open = true WHERE id = $1")
        .bind(store)
        .execute(&app.pool)
        .await
        .unwrap();
    place_order_at(&app, store).await;

    // The delete is refused with its remedy, not a 500.
    let response = aline
        .client
        .delete(&format!("/v1/merchant/stores/{store}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        409,
        "order history is a conflict, not a 500"
    );
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "conflict");
    assert!(
        body["message"].as_str().unwrap().contains("close"),
        "the message names the remedy: {body}"
    );

    // The store survived.
    assert_eq!(
        aline
            .client
            .get(&format!("/v1/merchant/stores/{store}"))
            .bearer_auth(&aline.token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    // A store with no history still deletes cleanly.
    let fresh = create_store(&aline, json!({ "name": "Aline Nyamirambo" })).await;
    assert_eq!(
        aline
            .client
            .delete(&format!("/v1/merchant/stores/{fresh}"))
            .bearer_auth(&aline.token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}

/// Scenario I — a store-scoped manager reaches exactly their store.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_store_scoped_manager_reaches_only_their_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let remix_store = create_store(&aline, json!({ "name": "Aline Remera" })).await;
    let downtown_store = create_store(&aline, json!({ "name": "Aline Downtown" })).await;
    let manager = seed_store_manager(
        &app.pool,
        "manager@example.com",
        aline.merchant_id,
        remix_store,
    )
    .await;
    let token = token_for(&app, manager.id, 3600);
    let client = TestClient::new(&app.address);

    // The list shows only the scoped store.
    let response = client
        .get("/v1/merchant/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0]["id"], remix_store.to_string());

    // Their store is reachable; the other store of the same business is not.
    assert_eq!(
        client
            .get(&format!("/v1/merchant/stores/{remix_store}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(&format!("/v1/merchant/stores/{downtown_store}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    // Managers cannot create stores (a business-level act).
    let response = client
        .post_json("/v1/merchant/stores", json!({ "name": "Manager's Store" }))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // And cannot delete their own store either.
    let response = client
        .delete(&format!("/v1/merchant/stores/{remix_store}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // But they run their store: opening it is part of the job.
    let response = client
        .patch_json(
            &format!("/v1/merchant/stores/{remix_store}"),
            json!({ "is_open": true }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap()["is_open"], true);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_suspended_business_grants_nothing(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let seeded = seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let token = token_for(&app, seeded.account.id, 3600);
    let client = TestClient::new(&app.address);

    // The business is suspended straight in the database — the next request
    // must refuse the operator, because authorization is resolved fresh.
    sqlx::query("UPDATE marketplace.merchants SET status = 'suspended' WHERE id = $1")
        .bind(seeded.merchant.id)
        .execute(&app.pool)
        .await
        .unwrap();

    let response = client
        .get("/v1/merchant/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_sees_only_open_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let (chantal, token) = customer_session(&app, "+250780000001").await;

    let open_id = create_store(&aline, json!({ "name": "Open kitchen" })).await;
    open(&app.pool, aline.merchant_id, open_id).await;
    create_store(&aline, json!({ "name": "Closed kitchen" })).await; // stays closed

    let response = chantal
        .get("/v1/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0]["name"], "Open kitchen");
    assert_eq!(stores[0]["is_open"], true);
    assert!(stores[0]["category"].is_null());

    // The closed store is indistinguishable from a missing one.
    let closed = create_store(&aline, json!({ "name": "Still closed" })).await;
    let response = chantal
        .get(&format!("/v1/stores/{closed}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // The open store's detail carries its (still empty) menu.
    let response = chantal
        .get(&format!("/v1/stores/{open_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["store"]["id"], open_id.to_string());
    assert_eq!(body["products"], json!([]));
}

/// "Stores near you" with the customer's real location: server-computed
/// distances, nearest first, coordinate-less stores trailing.
#[sqlx::test(migrator = "MIGRATOR")]
async fn list_stores_computes_distance_and_sorts_nearest_first(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    // Two located stores: the customer's pin sits right next to Near
    // Kitchen (~100 m); Far Kitchen is ~5 km away. Plus one store without
    // coordinates at all.
    create_store(
        &aline,
        json!({ "name": "Near Kitchen", "lat": -1.9630, "lng": 30.1290 }),
    )
    .await;
    create_store(
        &aline,
        json!({ "name": "Far Kitchen", "lat": -1.9390, "lng": 30.1255 }),
    )
    .await;
    create_store(&aline, json!({ "name": "Unlocated Kitchen" })).await;
    for name in ["Near Kitchen", "Far Kitchen", "Unlocated Kitchen"] {
        let mut conn = pool.acquire().await.unwrap();
        let store_id: (Uuid,) = sqlx::query_as(
            "SELECT id FROM marketplace.stores WHERE name = $1 AND merchant_id = $2",
        )
        .bind(name)
        .bind(aline.merchant_id)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        open(&app.pool, aline.merchant_id, store_id.0).await;
    }

    let (chantal, token) = customer_session(&app, "+250780000020").await;
    // Customer pin at KG 7 Ave, Remera — right next to Near Kitchen.
    let response = chantal
        .get("/v1/stores?lat=-1.9620&lng=30.1290")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores.len(), 3);

    // Nearest first; the unlocated store trails.
    assert_eq!(stores[0]["name"], "Near Kitchen");
    assert_eq!(stores[1]["name"], "Far Kitchen");
    assert_eq!(stores[2]["name"], "Unlocated Kitchen");

    // Server-computed facts: Near ≈ 100 m, Far ≈ 5 km.
    let near_distance = stores[0]["distance_m"].as_i64().unwrap();
    assert!(
        (50..=1_200).contains(&near_distance),
        "near store distance, got {near_distance}"
    );
    let far_distance = stores[1]["distance_m"].as_i64().unwrap();
    assert!(
        (2_200..=3_000).contains(&far_distance),
        "far store distance, got {far_distance}"
    );
    assert!(near_distance < far_distance);
    // Pure ride time at 25 km/h, ceil.
    assert_eq!(stores[0]["eta_min"].as_i64().unwrap(), 1); // ~0.1 km → 1 min
    assert_eq!(stores[1]["eta_min"].as_i64().unwrap(), 7); // ~2.6 km → 7 min
    // The trailing store has no distance facts at all.
    assert!(stores[2]["distance_m"].is_null());
    assert!(stores[2]["eta_min"].is_null());

    // Without a location the response is exactly the old shape: no
    // distance fields, creation order preserved.
    let response = chantal
        .get("/v1/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();
    assert_eq!(stores[0]["name"], "Near Kitchen"); // creation order
    assert!(stores[0]["distance_m"].is_null());

    // One coordinate alone, or an out-of-range/non-numeric one, is a 400 —
    // not a guess.
    for bad in [
        "?lat=-1.96",
        "?lng=30.12",
        "?lat=-999&lng=30.12",
        "?lat=abc&lng=30.12",
    ] {
        let response = chantal
            .get(&format!("/v1/stores{bad}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "bad location query {bad}");
    }
}

async fn open(pool: &PgPool, merchant_id: Uuid, store_id: Uuid) {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("UPDATE marketplace.stores SET is_open = true WHERE id = $1 AND merchant_id = $2")
        .bind(store_id)
        .bind(merchant_id)
        .execute(&mut *conn)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn audiences_are_capability_gated(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let (chantal, customer_token) = customer_session(&app, "+250780000001").await;

    // Customers cannot enter the merchant namespace.
    let response = chantal
        .post_json("/v1/merchant/stores", store_input())
        .bearer_auth(&customer_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // Operators cannot browse the customer catalog.
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

/// A sanity check that the harness login path matches the cookie flow used
/// by the platform tests.
#[sqlx::test(migrator = "MIGRATOR")]
async fn operator_login_reaches_the_merchant_wing(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let client = TestClient::new(&app.address);
    assert_eq!(
        login(&client, "owner@example.com", "Password123")
            .await
            .status(),
        204
    );
    assert_eq!(
        client
            .get("/v1/merchant/stores")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}

/// Open a store by its name within one merchant (test helper).
async fn open_named(pool: &PgPool, merchant_id: Uuid, name: &str) {
    let mut conn = pool.acquire().await.unwrap();
    let (store_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM marketplace.stores WHERE name = $1 AND merchant_id = $2")
            .bind(name)
            .bind(merchant_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    drop(conn);
    open(pool, merchant_id, store_id).await;
}

/// Server-side search: `?q=` matches store names and categories,
/// case-insensitively; closed stores never surface; a matchless term is
/// an empty array; whitespace-only means the plain feed, identical to
/// the no-query response.
#[sqlx::test(migrator = "MIGRATOR")]
async fn list_stores_search_matches_name_or_category(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    create_store(
        &aline,
        json!({ "name": "Aline's Kitchen", "category": "Grill" }),
    )
    .await;
    create_store(
        &aline,
        json!({ "name": "Kigali Heights Cafe", "category": "Coffee" }),
    )
    .await;
    // Matches "grill" by name but stays closed — customers never see it.
    create_store(&aline, json!({ "name": "Grill Master HQ" })).await;
    open_named(&app.pool, aline.merchant_id, "Aline's Kitchen").await;
    open_named(&app.pool, aline.merchant_id, "Kigali Heights Cafe").await;

    let (chantal, token) = customer_session(&app, "+250780000021").await;

    // By name — case-insensitive substring.
    let response = chantal
        .get("/v1/stores?q=aline")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let names: Vec<&str> = stores
        .as_array()
        .unwrap()
        .iter()
        .map(|store| store["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Aline's Kitchen"]);

    // By category.
    let response = chantal
        .get("/v1/stores?q=coffee")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    assert_eq!(stores[0]["name"], "Kigali Heights Cafe");

    // The closed store is invisible to search even when the term matches.
    let response = chantal
        .get("/v1/stores?q=grill")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    let names: Vec<&str> = stores
        .as_array()
        .unwrap()
        .iter()
        .map(|store| store["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Aline's Kitchen"]);

    // No match: an empty array, not an error.
    let response = chantal
        .get("/v1/stores?q=zzz")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    assert!(stores.as_array().unwrap().is_empty());

    // Whitespace-only and no query are the same plain feed.
    let response = chantal
        .get("/v1/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let plain: Value = response.json().await.unwrap();
    let response = chantal
        .get("/v1/stores?q=%20%20")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let blank: Value = response.json().await.unwrap();
    assert_eq!(plain, blank);
    assert_eq!(plain.as_array().unwrap().len(), 2);
}

/// Search composes with the customer's location: the filter runs first,
/// then the remaining matches sort nearest-first with their distances.
#[sqlx::test(migrator = "MIGRATOR")]
async fn list_stores_search_combines_with_location(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    // Two grills (near and far) and a cafe right next to the customer.
    create_store(
        &aline,
        json!({ "name": "Near Grill", "category": "Grill", "lat": -1.9630, "lng": 30.1290 }),
    )
    .await;
    create_store(
        &aline,
        json!({ "name": "Far Grill", "category": "Grill", "lat": -1.9390, "lng": 30.1255 }),
    )
    .await;
    create_store(
        &aline,
        json!({ "name": "Near Cafe", "category": "Coffee", "lat": -1.9621, "lng": 30.1291 }),
    )
    .await;
    for name in ["Near Grill", "Far Grill", "Near Cafe"] {
        open_named(&app.pool, aline.merchant_id, name).await;
    }

    let (chantal, token) = customer_session(&app, "+250780000022").await;
    let response = chantal
        .get("/v1/stores?q=grill&lat=-1.9620&lng=30.1290")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let stores = stores.as_array().unwrap();

    // Only the grills, nearest first; the cafe — though closest — is out.
    assert_eq!(stores.len(), 2);
    assert_eq!(stores[0]["name"], "Near Grill");
    assert_eq!(stores[1]["name"], "Far Grill");
    assert!(stores[0]["distance_m"].as_i64().unwrap() < stores[1]["distance_m"].as_i64().unwrap());
}

/// The LIKE wildcards are escaped — searching "%" finds the literal
/// percent, not everything — and an over-long term is a 400.
#[sqlx::test(migrator = "MIGRATOR")]
async fn list_stores_search_escapes_wildcards_and_caps_length(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;

    create_store(
        &aline,
        json!({ "name": "100% Juices", "category": "Juice" }),
    )
    .await;
    create_store(
        &aline,
        json!({ "name": "Plain Kitchen", "category": "Food" }),
    )
    .await;
    open_named(&app.pool, aline.merchant_id, "100% Juices").await;
    open_named(&app.pool, aline.merchant_id, "Plain Kitchen").await;

    let (chantal, token) = customer_session(&app, "+250780000023").await;

    // A literal percent inside a store name is findable.
    let response = chantal
        .get("/v1/stores?q=100%25")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    assert_eq!(stores[0]["name"], "100% Juices");

    // A bare wildcard is escaped, so it matches nothing rather than
    // everything. A bare "%" still finds the store whose name genuinely
    // contains a percent sign — escaping preserves the literal — but
    // "Plain Kitchen" (no wildcard character in it) stays out.
    let response = chantal
        .get("/v1/stores?q=%25")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    let names: Vec<&str> = stores
        .as_array()
        .unwrap()
        .iter()
        .map(|store| store["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["100% Juices"]);

    // A bare underscore is also escaped, and nothing in the seed has a
    // literal "_" — so it matches nothing rather than everything.
    let response = chantal
        .get("/v1/stores?q=_")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let stores: Value = response.json().await.unwrap();
    assert!(
        stores.as_array().unwrap().is_empty(),
        "an escaped underscore must not match everything"
    );

    // Over-long search term: 400, same validation-at-the-door style as
    // the coordinates.
    let long = "a".repeat(101);
    let response = chantal
        .get(&format!("/v1/stores?q={long}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn store_responses_carry_the_contact_surface(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;

    let store_id = create_store(
        &aline,
        json!({
            "name": "Contact Kitchen",
            "contact_phone": "+250788123456",
            "contact_email": "hello@aline.rw"
        }),
    )
    .await;
    open(&app.pool, aline.merchant_id, store_id).await;

    let (chantal, token) = customer_session(&app, "+250780000030").await;

    // The list carries both contacts.
    let response = chantal
        .get("/v1/stores")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let stores: Value = response.json().await.unwrap();
    let store = stores
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Contact Kitchen")
        .unwrap()
        .clone();
    assert_eq!(store["contact_phone"], json!("+250788123456"));
    assert_eq!(store["contact_email"], json!("hello@aline.rw"));

    // The detail (the info sheet's data source) carries both too.
    let id = store["id"].as_str().unwrap();
    let response = chantal
        .get(&format!("/v1/stores/{id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let detail: Value = response.json().await.unwrap();
    assert_eq!(detail["store"]["contact_phone"], json!("+250788123456"));
    assert_eq!(detail["store"]["contact_email"], json!("hello@aline.rw"));

    // A store created without contacts leaves the fields null — the app
    // renders honest "no contact" rows, never invented ones.
    let bare_id = create_store(&aline, json!({ "name": "Bare Kitchen" })).await;
    open(&app.pool, aline.merchant_id, bare_id).await;
    let response = chantal
        .get(&format!("/v1/stores/{bare_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let detail: Value = response.json().await.unwrap();
    assert!(detail["store"]["contact_phone"].is_null());
    assert!(detail["store"]["contact_email"].is_null());
}

/// S36: two writers, one store — the loser's PATCH must be a 409, not a
/// silent last-write-wins. The second writer holds the row lock with an
/// uncommitted UPDATE between the handler's read and write: the handler
/// parks on the lock with the old updated_at already read, the writer
/// commits, and the precondition no longer matches.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_stale_store_patch_is_a_conflict_not_a_silent_overwrite(pool: sqlx::PgPool) {
    let app = spawn_app(pool.clone()).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, store_input()).await;

    // A normal PATCH still lands — the precondition must not fight
    // legitimate sequential edits.
    let first = aline
        .client
        .patch_json(
            &format!("/v1/merchant/stores/{store}"),
            json!({ "name": "Aline Remera" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    assert_eq!(first.json::<Value>().await.unwrap()["name"], "Aline Remera");

    // The second writer wins the race: its uncommitted UPDATE moves
    // updated_at and holds the row lock.
    let mut second_writer = pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE marketplace.stores SET description = 'Hijacked by the second writer' WHERE id = $1",
    )
    .bind(store)
    .execute(&mut *second_writer)
    .await
    .unwrap();

    let stale_patch = tokio::spawn({
        let client = aline.client.clone();
        let token = aline.token.clone();
        async move {
            client
                .patch_json(
                    &format!("/v1/merchant/stores/{store}"),
                    json!({ "description": "Aline's own words" }),
                )
                .bearer_auth(&token)
                .send()
                .await
                .unwrap()
        }
    });
    // Let the handler read the row and park on the lock before committing.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    second_writer.commit().await.unwrap();

    let response = stale_patch.await.unwrap();
    assert_eq!(
        response.status(),
        409,
        "a lost read-write race is a conflict, not a silent overwrite"
    );
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "conflict");
    assert!(
        body["message"].as_str().unwrap().contains("someone else"),
        "the message says reload and retry: {body}"
    );

    // The second writer's change survived — nothing was overwritten.
    let (description,): (Option<String>,) =
        sqlx::query_as("SELECT description FROM marketplace.stores WHERE id = $1")
            .bind(store)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        description.as_deref(),
        Some("Hijacked by the second writer"),
        "the stale PATCH must not land"
    );
}
