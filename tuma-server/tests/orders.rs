//! The checkout redesign: one customer checkout → one order group → one
//! store order per participating store, funded by one payment with
//! explicit allocations. Covers the directive's scenarios A–F, H–J plus
//! the state machine and admin controls.

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

async fn owner(app: &common::TestApp, email: &str, business: &str) -> Operator {
    let seeded = seed_merchant(&app.pool, email, business).await;
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

async fn create_store(op: &Operator, name: &str) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/stores",
            json!({ "name": name, "delivery_fee": 1500 }),
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

/// Attach a catalog product to a store; `stock: null` = untracked.
async fn attach(op: &Operator, store_id: Uuid, product_id: Uuid, price: i64, stock: Value) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store_id, "price": price, "stock": stock }),
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

async fn checkout(
    client: &TestClient,
    token: &str,
    address: &str,
    items: Value,
    key: Option<&str>,
) -> reqwest::Response {
    let mut body = json!({ "address_text": address, "items": items });
    if let Some(key) = key {
        body["idempotency_key"] = json!(key);
    }
    client
        .post_json("/v1/orders", body)
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
}

fn line(store_product_id: Uuid, quantity: i32) -> Value {
    json!({ "store_product_id": store_product_id, "quantity": quantity })
}

/// Count the customer's groups (history length) — the all-or-nothing probe.
async fn group_count(client: &TestClient, token: &str) -> usize {
    let response = client
        .get("/v1/orders")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response
        .json::<Value>()
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .len()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_a_single_store_checkout(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's Kitchen").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let milk = create_product(&aline, "Milk 1L").await;
    let beans = create_product(&aline, "Beans 1KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(null)).await;
    let milk_sp = attach(&aline, store, milk, 1500, json!(null)).await;
    let beans_sp = attach(&aline, store, beans, 3500, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000001").await;
    let response = checkout(
        &chantal,
        &token,
        "KN 4 Ave, Kigali",
        json!([line(rice_sp, 1), line(milk_sp, 2), line(beans_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    // One group, one store order, three items.
    let store_orders = body["store_orders"].as_array().unwrap();
    assert_eq!(store_orders.len(), 1);
    assert_eq!(store_orders[0]["store_name"], "Aline Remera");
    assert_eq!(store_orders[0]["items"].as_array().unwrap().len(), 3);
    assert_eq!(store_orders[0]["status"], "placed");
    // 12000 + 1500×2 + 3500 = 18500; delivery 1500 → grand 20000.
    assert_eq!(body["subtotal"], 18500);
    assert_eq!(body["delivery_total"], 1500);
    assert_eq!(body["grand_total"], 20000);
    assert_eq!(body["status"], "in_progress");
    assert_eq!(body["payment_status"], "pending");
    assert!(body["number"].is_i64(), "human-friendly order number");
    assert!(
        body["idempotency_key"].is_null(),
        "client keys are not echoed"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_b_two_store_checkout_splits_per_store(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's Market").await;
    let simba = create_store(&aline, "Simba Remera").await;
    let kfc = create_store(&aline, "KFC Kimihurura").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let milk = create_product(&aline, "Milk 1L").await;
    let burger = create_product(&aline, "Burger").await;
    let fries = create_product(&aline, "Fries").await;
    let rice_sp = attach(&aline, simba, rice, 12000, json!(null)).await;
    let milk_sp = attach(&aline, simba, milk, 1500, json!(null)).await;
    let burger_sp = attach(&aline, kfc, burger, 7000, json!(null)).await;
    let fries_sp = attach(&aline, kfc, fries, 3000, json!(null)).await;
    set_open(&app.pool, simba, true).await;
    set_open(&app.pool, kfc, true).await;

    let (chantal, token) = customer_session(&app, "+250780000002").await;
    let response = checkout(
        &chantal,
        &token,
        "KN 4 Ave",
        json!([
            line(rice_sp, 1),
            line(milk_sp, 1),
            line(burger_sp, 1),
            line(fries_sp, 1)
        ]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();

    // 1 group, 2 store orders, 4 items split 2/2.
    let store_orders = body["store_orders"].as_array().unwrap();
    assert_eq!(store_orders.len(), 2);
    assert_eq!(
        store_orders
            .iter()
            .map(|order| order["items"].as_array().unwrap().len())
            .sum::<usize>(),
        4
    );

    // Per-store money: Simba 13,500 + 1,500 fee; KFC 10,000 + 1,500 fee.
    let simba_order = store_orders
        .iter()
        .find(|order| order["store_name"] == "Simba Remera")
        .unwrap();
    assert_eq!(simba_order["subtotal"], 13500);
    assert_eq!(simba_order["delivery_fee"], 1500);
    assert_eq!(simba_order["total"], 15000);
    let kfc_order = store_orders
        .iter()
        .find(|order| order["store_name"] == "KFC Kimihurura")
        .unwrap();
    assert_eq!(kfc_order["subtotal"], 10000);
    assert_eq!(kfc_order["total"], 11500);

    // The customer paid one checkout total: 23,500 + 3,000.
    assert_eq!(body["subtotal"], 23500);
    assert_eq!(body["delivery_total"], 3000);
    assert_eq!(body["grand_total"], 26500);
    assert_eq!(body["status"], "in_progress");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_c_same_merchant_two_stores_still_two_orders(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let remera = create_store(&aline, "Aline Remera").await;
    let downtown = create_store(&aline, "Aline Downtown").await;
    let rice = create_product(&aline, "Rice").await;
    let coffee = create_product(&aline, "Coffee").await;
    let rice_sp = attach(&aline, remera, rice, 5000, json!(null)).await;
    let coffee_sp = attach(&aline, downtown, coffee, 2000, json!(null)).await;
    set_open(&app.pool, remera, true).await;
    set_open(&app.pool, downtown, true).await;

    let (chantal, token) = customer_session(&app, "+250780000003").await;
    let body: Value = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1), line(coffee_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();

    // Same business, but the stores fulfill separately — never one order
    // carrying both stores' items.
    let store_orders = body["store_orders"].as_array().unwrap();
    assert_eq!(store_orders.len(), 2);
    let store_ids: Vec<&str> = store_orders
        .iter()
        .map(|order| order["store_id"].as_str().unwrap())
        .collect();
    assert_ne!(store_ids[0], store_ids[1]);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_d_and_h_each_merchant_sees_only_their_store_orders(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let bruce = owner(&app, "bruce@example.com", "Bruce's").await;
    let aline_store = create_store(&aline, "Aline Remera").await;
    let bruce_store = create_store(&bruce, "Bruce's Grill").await;
    let aline_item = create_product(&aline, "Aline Dish").await;
    let bruce_item = create_product(&bruce, "Bruce Burger").await;
    let aline_sp = attach(&aline, aline_store, aline_item, 5000, json!(null)).await;
    let bruce_sp = attach(&bruce, bruce_store, bruce_item, 7000, json!(null)).await;
    set_open(&app.pool, aline_store, true).await;
    set_open(&app.pool, bruce_store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000004").await;
    let body: Value = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(aline_sp, 1), line(bruce_sp, 2)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(body["store_orders"].as_array().unwrap().len(), 2);

    // Each operator sees only their own slice.
    for (op, expected_name, expected_total) in [
        (&aline, "Aline Remera", 5000 + 1500),
        (&bruce, "Bruce's Grill", 7000 * 2 + 1500),
    ] {
        let response = op
            .client
            .get("/v1/merchant/orders")
            .bearer_auth(&op.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let orders: Value = response.json().await.unwrap();
        let orders = orders.as_array().unwrap();
        assert_eq!(orders.len(), 1, "only their own store order");
        assert_eq!(orders[0]["store_name"], expected_name);
        assert_eq!(orders[0]["total"], expected_total);
        assert_eq!(orders[0]["address_text"], "Anywhere");
    }

    // Bruce cannot advance Aline's store order — it looks missing.
    let aline_order_id = aline
        .client
        .get("/v1/merchant/orders")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = bruce
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{aline_order_id}"),
            json!({ "status": "accepted" }),
        )
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_e_checkout_is_all_or_nothing(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let good_store = create_store(&aline, "Open Kitchen").await;
    let closed_store = create_store(&aline, "Closed Kitchen").await;
    let rice = create_product(&aline, "Rice").await;
    let burger = create_product(&aline, "Burger").await;
    let ghost = create_product(&aline, "Ghost Dish").await; // never attached
    let rice_sp = attach(&aline, good_store, rice, 5000, json!(null)).await;
    let burger_sp = attach(&aline, closed_store, burger, 7000, json!(null)).await;
    set_open(&app.pool, good_store, true).await;
    // closed_store stays closed.

    let (chantal, token) = customer_session(&app, "+250780000005").await;

    // A closed store in the cart kills the whole checkout, naming it.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1), line(burger_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "conflict");
    assert!(
        body["message"].as_str().unwrap().contains("Closed Kitchen"),
        "names the store: {body}"
    );

    // An unattached product kills it too.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1), line(ghost, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 409);
    assert_eq!(group_count(&chantal, &token).await, 0, "nothing was placed");

    // The happy path afterwards still works.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    assert_eq!(group_count(&chantal, &token).await, 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn checkout_reserves_stock_atomically(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(1)).await; // one bag on the shelf
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000006").await;

    // Asking for two when one exists fails and reserves nothing.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 2)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("stock"),
        "{body}"
    );
    assert_eq!(group_count(&chantal, &token).await, 0);

    // One succeeds and decrements the shelf.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);

    let response = aline
        .client
        .get("/v1/merchant/store-products")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    let items: Value = response.json().await.unwrap();
    assert_eq!(items[0]["stock"], 0, "the shelf is empty");

    // A second checkout of the same item now hits the empty shelf.
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 409);
    assert_eq!(group_count(&chantal, &token).await, 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_f_retry_with_the_same_key_returns_the_same_group(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000007").await;

    let first = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        Some("retry-key-1"),
    )
    .await;
    assert_eq!(first.status(), 201);
    let first_body: Value = first.json().await.unwrap();

    // A network blip retried the request: same key, same group, 200.
    let retry = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        Some("retry-key-1"),
    )
    .await;
    assert_eq!(retry.status(), 200);
    let retry_body: Value = retry.json().await.unwrap();
    assert_eq!(first_body["id"], retry_body["id"]);

    // A different key is a genuinely new checkout.
    let second = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        Some("retry-key-2"),
    )
    .await;
    assert_eq!(second.status(), 201);
    assert_eq!(group_count(&chantal, &token).await, 2);

    // Keys are per-customer: another customer's retry key is their own.
    let (bruce_customer, bruce_token) = customer_session(&app, "+250780000008").await;
    let response = checkout(
        &bruce_customer,
        &bruce_token,
        "X",
        json!([line(rice_sp, 1)]),
        Some("retry-key-1"),
    )
    .await;
    assert_eq!(response.status(), 201);
}

/// The fulfillment sheet: the operator opens one order and sees the items,
/// the delivery address, and the customer contact.
#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_opens_the_fulfillment_sheet(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let bruce = owner(&app, "bruce@example.com", "Bruce's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(20)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780011001").await;
    // The customer gave their name at signup — it flows to the sheet.
    assert_eq!(
        chantal
            .patch_json("/v1/me", json!({ "name": "Chantal" }))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let group: Value = checkout(
        &chantal,
        &token,
        "KN 4 Ave, Kigali",
        json!([line(rice_sp, 2)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap();

    // The owner reads the sheet: items, money, address, and the customer
    // contact to call when the rider is at the gate.
    let response = aline
        .client
        .get(&format!("/v1/merchant/store-orders/{order_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["number"], group["store_orders"][0]["number"]);
    assert_eq!(body["status"], "placed");
    assert_eq!(body["address_text"], "KN 4 Ave, Kigali");
    assert_eq!(body["customer_name"], "Chantal");
    assert_eq!(body["customer_phone"], "+250780011001");
    assert_eq!(body["payment_status"], "pending");
    assert_eq!(body["subtotal"], 24000);
    assert_eq!(body["delivery_fee"], 1500);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["product_name"], "Rice 5KG");
    assert_eq!(items[0]["quantity"], 2);

    // Another merchant's order is a plain 404.
    let response = bruce
        .client
        .get(&format!("/v1/merchant/store-orders/{order_id}"))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // The customer themselves cannot open the operator sheet.
    let response = chantal
        .get(&format!("/v1/merchant/store-orders/{order_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_i_a_store_scoped_manager_sees_only_their_stores_orders(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let remix = create_store(&aline, "Aline Remera").await;
    let downtown = create_store(&aline, "Aline Downtown").await;
    let remix_item = create_product(&aline, "Remera Dish").await;
    let downtown_item = create_product(&aline, "Downtown Dish").await;
    let remix_sp = attach(&aline, remix, remix_item, 5000, json!(null)).await;
    let downtown_sp = attach(&aline, downtown, downtown_item, 5000, json!(null)).await;
    set_open(&app.pool, remix, true).await;
    set_open(&app.pool, downtown, true).await;

    let manager =
        seed_store_manager(&app.pool, "manager@example.com", aline.merchant_id, remix).await;
    let manager_client = TestClient::new(&app.address);
    let manager_token = token_for(&app, manager.id, 3600);

    let (chantal, token) = customer_session(&app, "+250780000009").await;
    checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(remix_sp, 1), line(downtown_sp, 1)]),
        None,
    )
    .await;

    // The manager's board shows only Remera's slice, and they can advance it.
    let response = manager_client
        .get("/v1/merchant/orders")
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let orders: Value = response.json().await.unwrap();
    let orders = orders.as_array().unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["store_name"], "Aline Remera");

    let remix_order = orders[0]["id"].as_str().unwrap().to_string();
    let response = manager_client
        .patch_json(
            &format!("/v1/merchant/store-orders/{remix_order}"),
            json!({ "status": "accepted" }),
        )
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "accepted"
    );

    // Downtown's slice is invisible to them.
    let owner_orders: Value = aline
        .client
        .get("/v1/merchant/orders")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let downtown_order = owner_orders
        .as_array()
        .unwrap()
        .iter()
        .find(|order| order["store_name"] == "Aline Downtown")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = manager_client
        .patch_json(
            &format!("/v1/merchant/store-orders/{downtown_order}"),
            json!({ "status": "accepted" }),
        )
        .bearer_auth(&manager_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn scenario_j_store_orders_cancel_independently(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let simba = create_store(&aline, "Simba Remera").await;
    let kfc = create_store(&aline, "KFC Kimihurura").await;
    let rice = create_product(&aline, "Rice").await;
    let burger = create_product(&aline, "Burger").await;
    let rice_sp = attach(&aline, simba, rice, 12000, json!(null)).await;
    let burger_sp = attach(&aline, kfc, burger, 7000, json!(null)).await;
    set_open(&app.pool, simba, true).await;
    set_open(&app.pool, kfc, true).await;

    let (chantal, token) = customer_session(&app, "+250780000010").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1), line(burger_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let group_id = group["id"].as_str().unwrap().to_string();
    let orders = group["store_orders"].as_array().unwrap();
    let simba_order = orders
        .iter()
        .find(|o| o["store_name"] == "Simba Remera")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // The customer cancels one slice.
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{simba_order}/cancel"),
            json!({ "reason": "changed my mind" }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "cancelled"
    );

    // The other slice keeps cooking; the group is partially fulfilled.
    let response = chantal
        .get(&format!("/v1/orders/{group_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "partially_fulfilled");
    let orders = body["store_orders"].as_array().unwrap();
    assert_eq!(
        orders
            .iter()
            .find(|o| o["store_name"] == "KFC Kimihurura")
            .unwrap()["status"],
        "placed"
    );

    // Cancelling twice is an illegal transition.
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{simba_order}/cancel"),
            json!({}),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // Another customer's order is invisible.
    let (other, other_token) = customer_session(&app, "+250780000011").await;
    let response = other
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{simba_order}/cancel"),
            json!({}),
        )
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_six_status_machine_is_enforced(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000012").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();
    let advance = async |status: &str| {
        aline
            .client
            .patch_json(
                &format!("/v1/merchant/store-orders/{order_id}"),
                json!({ "status": status }),
            )
            .bearer_auth(&aline.token)
            .send()
            .await
            .unwrap()
    };

    // Nonsense status string.
    assert_eq!(advance("teleported").await.status(), 400);
    // Placed → delivered skips three states.
    assert_eq!(advance("delivered").await.status(), 400);
    // The happy path: accepted → preparing → picked_up → delivered.
    for status in ["accepted", "preparing", "picked_up", "delivered"] {
        let response = advance(status).await;
        assert_eq!(response.status(), 200, "advance to {status}");
        assert_eq!(response.json::<Value>().await.unwrap()["status"], status);
    }
    // A terminal state accepts no moves.
    assert_eq!(advance("delivered").await.status(), 400);
    assert_eq!(advance("cancelled").await.status(), 400);

    // The group followed its children to completed.
    let body: Value = chantal
        .get(&format!("/v1/orders/{}", group["id"].as_str().unwrap()))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["status"], "completed");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_controls_orders_and_collects_cash(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    common::seed_admin(&app.pool, "admin@example.com").await;
    let admin = TestClient::new(&app.address);
    assert_eq!(
        login(&admin, "admin@example.com", "Password123")
            .await
            .status(),
        204
    );

    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000013").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 2)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();

    // Admin advances any order.
    let response = admin
        .patch_json(
            &format!("/v1/admin/store-orders/{order_id}"),
            json!({ "status": "accepted" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The payment ledger: one payment, one allocation, amounts explicit.
    let (payment_id, allocation_amount): (Uuid, i64) = sqlx::query_as(
        r#"
        SELECT p.id, pa.amount
        FROM commerce.payments p
        JOIN commerce.payment_allocations pa ON pa.payment_id = p.id
        WHERE p.order_group_id = $1
        "#,
    )
    .bind(group["id"].as_str().unwrap().parse::<Uuid>().unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        allocation_amount,
        5000 * 2 + 1500,
        "the store's explicit slice"
    );

    // Collect the cash: payment → collected, allocations → settled.
    let response = admin
        .post(&format!("/v1/admin/payments/{payment_id}/collect"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "collected");
    assert_eq!(body["amount"], 5000 * 2 + 1500);

    let (allocation_status,): (String,) = sqlx::query_as(
        "SELECT status::text FROM commerce.payment_allocations WHERE payment_id = $1",
    )
    .bind(payment_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(allocation_status, "settled");

    // Collecting twice is refused.
    let response = admin
        .post(&format!("/v1/admin/payments/{payment_id}/collect"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // Non-admins cannot touch the controls.
    let response = aline
        .client
        .post(&format!("/v1/admin/payments/{payment_id}/collect"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn order_history_pages_newest_first(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000014").await;
    for key in ["page-1", "page-2", "page-3"] {
        assert_eq!(
            checkout(
                &chantal,
                &token,
                "Anywhere",
                json!([line(rice_sp, 1)]),
                Some(key)
            )
            .await
            .status(),
            201
        );
    }

    // Newest first, one capped page at a time.
    let page: Value = chantal
        .get("/v1/orders?limit=2&offset=0")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = page.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["stores"][0], "Aline Remera");

    let page: Value = chantal
        .get("/v1/orders?limit=2&offset=2")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn audiences_and_anonymous_are_gated(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;

    // Merchants cannot check out; customers cannot sit on the merchant board.
    let (chantal, customer_token) = customer_session(&app, "+250780000015").await;
    let response = checkout(&aline.client, &aline.token, "X", json!([]), None).await;
    assert_eq!(response.status(), 403);
    let response = chantal
        .get("/v1/merchant/orders")
        .bearer_auth(&customer_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // Anonymous everywhere.
    let anonymous = TestClient::new(&app.address);
    assert_eq!(
        anonymous
            .post_json("/v1/orders", json!({ "address_text": "X", "items": [] }))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        anonymous.get("/v1/orders").send().await.unwrap().status(),
        401
    );
    assert_eq!(
        anonymous
            .get("/v1/merchant/orders")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
}
