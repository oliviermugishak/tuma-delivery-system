//! The two-level catalog: merchant products (identity) and store_products
//! (per-store price, stock, availability). The customer buys a
//! store_product — the menu is the store's assortment, filtered to what is
//! available.

mod common;

use common::{
    MIGRATOR, TestClient, login, seed_customer, seed_merchant, seed_store_manager, spawn_app,
    token_for,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

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

async fn open_store(pool: &PgPool, store_id: Uuid) {
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("UPDATE marketplace.stores SET is_open = true WHERE id = $1")
        .bind(store_id)
        .execute(&mut *conn)
        .await
        .unwrap();
}

/// Create a catalog product and return its id.
async fn create_product(session: &Operator, name: &str, price_free: bool) -> Uuid {
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
    assert_eq!(body["merchant_id"], session.merchant_id.to_string());
    assert!(
        body["price"].is_null(),
        "the catalog has no price — stores do"
    );
    let _ = price_free;
    body["id"].as_str().unwrap().parse().unwrap()
}

/// Attach a catalog product to a store with a price and return the store
/// product id.
async fn attach(
    session: &Operator,
    store_id: Uuid,
    product_id: Uuid,
    price: i64,
    stock: Value,
) -> Uuid {
    let response = session
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({
                "product_id": product_id,
                "store_id": store_id,
                "price": price,
                "stock": stock
            }),
        )
        .bearer_auth(&session.token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        201,
        "attached {product_id} to {store_id}"
    );
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

async fn create_store(session: &Operator, name: &str) -> Uuid {
    let response = session
        .client
        .post_json("/v1/merchant/stores", json!({ "name": name }))
        .bearer_auth(&session.token)
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn owner_builds_a_catalog_and_an_assortment(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let kitchen = create_store(&aline, "Aline Remera").await;

    let rice = create_product(&aline, "Rice 5KG", true).await;
    let milk = create_product(&aline, "Milk 1L", true).await;

    // The catalog lists both, oldest first, and carries no prices.
    let response = aline
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let catalog_list: Value = response.json().await.unwrap();
    let items = catalog_list.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "Rice 5KG");
    assert_eq!(items[1]["name"], "Milk 1L");

    // The same catalog product sells at different prices per store.
    let rice_remera = attach(&aline, kitchen, rice, 12000, json!(20)).await;
    attach(&aline, kitchen, milk, 1500, json!(null)).await; // untracked

    let response = aline
        .client
        .get("/v1/merchant/store-products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let assortment: Value = response.json().await.unwrap();
    let items = assortment.as_array().unwrap();
    assert_eq!(items.len(), 2);
    let rice_view = items
        .iter()
        .find(|item| item["id"] == rice_remera.to_string())
        .expect("the rice store product");
    assert_eq!(rice_view["product_name"], "Rice 5KG");
    assert_eq!(rice_view["price"], 12000);
    assert_eq!(rice_view["stock"], 20);
    assert_eq!(rice_view["store_name"], "Aline Remera");
    let milk_view = items
        .iter()
        .find(|item| item["product_name"] == "Milk 1L")
        .expect("the milk store product");
    assert!(milk_view["stock"].is_null(), "null stock = untracked");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_same_product_sells_differently_across_stores(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let remera = create_store(&aline, "Aline Remera").await;
    let kimironko = create_store(&aline, "Aline Kimironko").await;
    let rice = create_product(&aline, "Rice 5KG", true).await;

    attach(&aline, remera, rice, 12000, json!(20)).await;
    attach(&aline, kimironko, rice, 12500, json!(5)).await;
    open_store(&app.pool, remera).await;
    open_store(&app.pool, kimironko).await;

    let seeded = seed_customer(&app.pool, "+250780000010").await;
    let token = token_for(&app, seeded.account.id, 3600);
    let client = TestClient::new(&app.address);

    // Each store shows its own price for the same catalog product.
    for (store_id, expected_price, expected_stock) in [(remera, 12000, 20), (kimironko, 12500, 5)] {
        let response = client
            .get(&format!("/v1/stores/{store_id}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: Value = response.json().await.unwrap();
        let products = body["products"].as_array().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0]["name"], "Rice 5KG");
        assert_eq!(products[0]["price"], expected_price);
        assert!(products[0]["id"].is_string());
        let _ = expected_stock; // stock is deliberately not in the customer view
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn attach_requires_own_product_and_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let bruce = owner(&app, "bruce@example.com").await;

    let aline_store = create_store(&aline, "Aline Remera").await;
    let bruce_store = create_store(&bruce, "Bruce's").await;
    let bruce_product = create_product(&bruce, "Bruce's Burger", true).await;
    let aline_product = create_product(&aline, "Aline's Dish", true).await;

    // A foreign catalog product is a 404 ("product not found").
    let response = aline
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({
                "product_id": bruce_product,
                "store_id": aline_store,
                "price": 1000
            }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "product not found");

    // A foreign store is a 404 ("store not found").
    let response = aline
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({
                "product_id": aline_product,
                "store_id": bruce_store,
                "price": 1000
            }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "store not found");

    // Attaching the same product to the same store twice is a 409.
    assert_eq!(
        aline
            .client
            .post_json(
                "/v1/merchant/store-products",
                json!({ "product_id": aline_product, "store_id": aline_store, "price": 1000 })
            )
            .bearer_auth(&aline.token)
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    let response = aline
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": aline_product, "store_id": aline_store, "price": 2000 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "this store already sells this product");

    // Unknown ids are 404s too.
    let response = aline
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({
                "product_id": Uuid::new_v4(),
                "store_id": aline_store,
                "price": 1000
            }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn catalog_is_owner_territory_and_assortment_is_store_work(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, "Aline Remera").await;
    let product = create_product(&aline, "Aline's Dish", true).await;
    let manager =
        seed_store_manager(&app.pool, "manager@example.com", aline.merchant_id, store).await;
    let manager_token = token_for(&app, manager.id, 3600);
    let manager_client = TestClient::new(&app.address);

    // A store-scoped manager cannot create or edit the catalog.
    assert_eq!(
        manager_client
            .post_json("/v1/merchant/products", json!({ "name": "Manager Dish" }))
            .bearer_auth(&manager_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        manager_client
            .patch_json(
                &format!("/v1/merchant/products/{product}"),
                json!({ "name": "Hacked" })
            )
            .bearer_auth(&manager_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        manager_client
            .delete(&format!("/v1/merchant/products/{product}"))
            .bearer_auth(&manager_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    // But they run their store's assortment.
    let response = manager_client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product, "store_id": store, "price": 3500, "stock": 10 }),
        )
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let store_product: Value = response.json().await.unwrap();
    let store_product_id = store_product["id"].as_str().unwrap().to_string();

    // Their list shows their store only (the other store is empty anyway).
    let response = manager_client
        .get("/v1/merchant/store-products")
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let items: Value = response.json().await.unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);

    // Price updates are part of running the store.
    let response = manager_client
        .patch_json(
            &format!("/v1/merchant/store-products/{store_product_id}"),
            json!({ "price": 4000, "stock": null, "is_available": false }),
        )
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["price"], 4000);
    assert!(
        body["stock"].is_null(),
        "explicit null switches to untracked"
    );
    assert_eq!(body["is_available"], false);

    // Detaching is store work too.
    assert_eq!(
        manager_client
            .delete(&format!("/v1/merchant/store-products/{store_product_id}"))
            .bearer_auth(&manager_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn catalog_edits_flow_through_to_the_assortment(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, "Aline Remera").await;
    let product = create_product(&aline, "Rice 5KG", true).await;
    let store_product = attach(&aline, store, product, 12000, json!(20)).await;

    // Catalog rename shows up in the assortment view.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/products/{product}"),
            json!({ "name": "Rice 10KG" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let response = aline
        .client
        .get("/v1/merchant/store-products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    let items: Value = response.json().await.unwrap();
    assert_eq!(items[0]["product_name"], "Rice 10KG");

    // Price and availability live on the store_product, untouched by the
    // catalog rename.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-products/{store_product}"),
            json!({ "price": 999 }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap()["price"], 999);

    // Deleting the catalog product takes it out of every assortment.
    let response = aline
        .client
        .delete(&format!("/v1/merchant/products/{product}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let response = aline
        .client
        .get("/v1/merchant/store-products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.json::<Value>().await.unwrap(), json!([]));

    // A foreign catalog product looks like a missing one.
    let bruce = owner(&app, "bruce@example.com").await;
    let bruce_product = create_product(&bruce, "Bruce's Burger", true).await;
    let response = aline
        .client
        .delete(&format!("/v1/merchant/products/{bruce_product}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_menu_shows_only_available_items(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG", true).await;
    let sold_out = create_product(&aline, "Sold Out Dish", true).await;

    attach(&aline, store, rice, 12000, json!(20)).await;
    let sold_out_sp = attach(&aline, store, sold_out, 5000, json!(1)).await;
    aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-products/{sold_out_sp}"),
            json!({ "is_available": false }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    open_store(&app.pool, store).await;

    let seeded = seed_customer(&app.pool, "+250780000011").await;
    let token = token_for(&app, seeded.account.id, 3600);
    let client = TestClient::new(&app.address);

    let response = client
        .get(&format!("/v1/stores/{store}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    let products = body["products"].as_array().unwrap();
    assert_eq!(products.len(), 1); // the paused item is hidden
    assert_eq!(products[0]["name"], "Rice 5KG");
    assert_eq!(products[0]["price"], 12000);
}

/// The platform's cookie login reaches the catalog too.
#[sqlx::test(migrator = "MIGRATOR")]
async fn catalog_reachable_through_the_cookie_transport(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let client = TestClient::new(&app.address);
    assert_eq!(
        login(&client, "owner@example.com", "Password123")
            .await
            .status(),
        204
    );
    let response = client
        .post_json("/v1/merchant/products", json!({ "name": "Tea" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    assert_eq!(
        client
            .get("/v1/merchant/products")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
}
