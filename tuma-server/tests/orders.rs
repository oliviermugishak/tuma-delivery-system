//! The checkout redesign: one customer checkout → one order group → one
//! store order per participating store, funded by one payment with
//! explicit allocations. Covers the directive's scenarios A–F, H–J plus
//! the state machine and admin controls.

mod common;

use common::{
    MIGRATOR, TestClient, login, seed_customer, seed_merchant, seed_rider, seed_store_manager,
    spawn_app, token_for,
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
    if response.status() != 200 {
        let body = response.text().await.unwrap_or_default();
        panic!("group_count got non-200: {body}");
    }
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
    // The happy path: accepted → preparing, then the handoff's pickup.
    for status in ["accepted", "preparing"] {
        let response = advance(status).await;
        assert_eq!(response.status(), 200, "advance to {status}");
        assert_eq!(response.json::<Value>().await.unwrap()["status"], status);
    }
    // picked_up is NOT a bare advance: without a rider attached the
    // delivery would be stranded — the handoff action (the rider's
    // number) is the only door in.
    let response = advance("picked_up").await;
    assert_eq!(response.status(), 400, "bare pickup rejected");
    assert!(
        response.json::<Value>().await.unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("rider")
    );
    // The real handoff: rider number in, picked_up out.
    let rider = seed_rider(&app.pool, "Jean", "+250780000002").await;
    let response = aline
        .client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "handoff picks the order up");
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "picked_up"
    );
    // The merchant still advances the delivery's end: picked_up →
    // delivered (the same ledger event as the rider's own Delivered).
    let response = advance("delivered").await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "delivered"
    );
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

/// Order operations are the merchants' monopoly: the platform admin has no
/// order-mutation or payment-collection endpoints at all. Allocations exist
/// as passive records, created automatically at checkout.
#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_has_no_order_endpoints(pool: sqlx::PgPool) {
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

    // The order-advance and collection endpoints do not exist on the admin
    // namespace — they were removed on purpose: store orders belong to the
    // merchants alone. The routes answer 404 (not even 403): nothing is
    // there to authorize.
    assert_eq!(
        admin
            .patch_json(
                &format!("/v1/admin/store-orders/{order_id}"),
                json!({ "status": "accepted" }),
            )
            .send()
            .await
            .unwrap()
            .status(),
        404,
        "admin cannot advance orders — no such route"
    );
    assert_eq!(
        admin
            .post(&format!("/v1/admin/store-orders/{order_id}/collect"))
            .send()
            .await
            .unwrap()
            .status(),
        404,
        "no collection route anywhere — the money system is not built"
    );

    // The payment ledger: one payment, one allocation, created automatically
    // at checkout with the store's explicit slice — passive records, no
    // endpoint mutates them.
    let (payment_status, allocation_amount, allocation_status): (String, i64, String) =
        sqlx::query_as(
            r#"
            SELECT p.status::text, pa.amount, pa.status::text
            FROM commerce.payments p
            JOIN commerce.payment_allocations pa ON pa.payment_id = p.id
            WHERE p.order_group_id = $1
            "#,
        )
        .bind(group["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(payment_status, "pending");
    assert_eq!(allocation_amount, 5000 * 2 + 1500);
    assert_eq!(allocation_status, "pending");

    // The merchant's own surface is untouched and fully operational.
    let response = aline
        .client
        .get(&format!("/v1/merchant/store-orders/{order_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
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

// The cancel integrity (review P01/P02): a cancel that only flipped the
// status burned the reserved stock and left the allocation `pending`
// forever — the group's payment could then never complete.

/// Review P11: the checkout's coordinates ride the same ±90/±180 rule as
/// the stores/search inputs — an out-of-range lat/lng is a 422, not a
/// poisoning of the group (and, downstream, of the rider's map).
#[sqlx::test(migrator = "MIGRATOR")]
async fn checkout_rejects_out_of_range_coordinates(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000014").await;
    // A latitude past the pole: nonsense geography, refused at the door.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "address_lat": 91.0,
                "address_lng": 30.0622,
                "items": [line(rice_sp, 1)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "lat beyond ±90 is refused");

    // Same for a longitude past the antimeridian...
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "address_lat": -1.9499,
                "address_lng": 181.0,
                "items": [line(rice_sp, 1)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "lng beyond ±180 is refused");

    // ...and nothing was placed by either attempt.
    assert_eq!(group_count(&chantal, &token).await, 0);
}

/// Review P12: the cart is capped at 50 lines — an unbounded `items`
/// array is a denial-of-wallet and a denial-of-database. The 422's field
/// error names the cap so the client can say so. (Validation runs before
/// the handler, so unknown product ids never get that far on the 51 side.)
#[sqlx::test(migrator = "MIGRATOR")]
async fn checkout_caps_the_cart_at_fifty_lines(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let (chantal, token) = customer_session(&app, "+250780000015").await;
    let cart = |lines: usize| {
        json!({
            "address_text": "KG 7 Ave, Remera",
            "items": (0..lines).map(|_| line(Uuid::new_v4(), 1)).collect::<Vec<_>>(),
        })
    };

    // 50 lines passes validation (it fails later, in the domain, on the
    // unknown product — anything but a 422 proves the cap did not fire).
    let response = chantal
        .post_json("/v1/orders", cart(50))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_ne!(response.status(), 422, "50 lines is inside the cap");

    // 51 lines is a 422 whose details name the cap.
    let response = chantal
        .post_json("/v1/orders", cart(51))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "51 lines is over the cap");
    let body: Value = response.json().await.unwrap();
    let messages: Vec<String> = body["details"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("50")),
        "the error names the cap: {messages:?}"
    );
}

/// The quantity ceiling: one line never buys more than 99 units. One over
/// is a 422 naming the cap; the ceiling itself checks out fine.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_quantity_above_the_ceiling_is_refused(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000017").await;

    // 100 units of one line is a 422 whose error names the cap.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "items": [line(rice_sp, 100)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        422,
        "quantity over the ceiling is refused"
    );
    let body: Value = response.json().await.unwrap();
    let messages: Vec<String> = body["details"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("99")),
        "the error names the cap: {messages:?}"
    );

    // 99 is the legal ceiling: the checkout lands.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "items": [line(rice_sp, 99)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "the ceiling itself is legal");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_cancelled_order_gives_its_stock_back(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Simba Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    // Tracked stock: 5 units on the shelf.
    let rice_sp = attach(&aline, store, rice, 12000, json!(5)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000010").await;
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 2)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    let group: Value = response.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap().to_string();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();

    // The reservation: 5 − 2 = 3 on the shelf.
    let (stock_after_checkout,): (i64,) = sqlx::query_as(
        "SELECT stock FROM marketplace.store_products WHERE id = $1",
    )
    .bind(rice_sp)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(stock_after_checkout, 3);

    // The cancel gives the 2 units back.
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{order_id}/cancel"),
            json!({ "reason": "changed my mind" }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The customer's typed reason is no longer discarded — it lands on
    // the cancelled store order.
    let detail: Value = chantal
        .get(&format!("/v1/orders/{group_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cancelled = detail["store_orders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["status"] == "cancelled")
        .expect("a cancelled store order exists");
    assert_eq!(cancelled["cancel_reason"], "changed my mind");

    let (stock_after_cancel,): (i64,) = sqlx::query_as(
        "SELECT stock FROM marketplace.store_products WHERE id = $1",
    )
    .bind(rice_sp)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(stock_after_cancel, 5, "the reserved units return to the shelf");

    // The slice's allocation is refunded, never left pending.
    let (allocation,): (String,) = sqlx::query_as(
        "SELECT status::text FROM commerce.payment_allocations WHERE store_order_id = $1",
    )
    .bind(order_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(allocation, "refunded");

    // The event trail: the checkout's `placed` opens it, the customer's
    // cancel closes it — two events, both the customer's own acts.
    let events: Vec<(String, String)> = sqlx::query_as(
        "SELECT status::text, actor_kind::text \
         FROM commerce.order_status_events \
         WHERE store_order_id = $1 ORDER BY created_at, id",
    )
    .bind(order_id.parse::<uuid::Uuid>().unwrap())
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut sorted = events;
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            ("cancelled".to_string(), "customer".to_string()),
            ("placed".to_string(), "customer".to_string()),
        ]
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_fully_cancelled_group_refunds_its_payment(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Simba Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let beans = create_product(&aline, "Beans 1KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(null)).await;
    let beans_sp = attach(&aline, store, beans, 3500, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000010").await;
    let response = checkout(
        &chantal,
        &token,
        "Anywhere",
        json!([line(rice_sp, 1), line(beans_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    let group: Value = response.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap().to_string();
    let orders = group["store_orders"].as_array().unwrap();
    assert_eq!(orders.len(), 1);

    let order_id = orders[0]["id"].as_str().unwrap().to_string();
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{order_id}/cancel"),
            json!({}),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // One store order, fully cancelled: nothing pending anywhere — the
    // group's payment is refunded, not stuck `pending` forever.
    let (payment,): (String,) = sqlx::query_as(
        "SELECT status::text FROM commerce.payments WHERE order_group_id = $1",
    )
    .bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(payment, "refunded");
}

// Review S27 (CRITICAL): the merchant's reject — PATCH
// {"status":"cancelled"} — must run the same ledger as the customer's
// cancel: reserved stock returns, the slice's allocation refunds, and
// the group's payment completes once the surviving delivery settles.
// The old advance wrote ONLY the status.

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_merchant_reject_restores_stock_and_settles_the_ledger(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let (chantal, token) = customer_session(&app, "+250780000016").await;
    let rider = seed_rider(&app.pool, "Jean", "+250780000002").await;

    // Two stores of one merchant, one checkout. A carries tracked stock;
    // Aline rejects A before accepting it.
    let store_a = create_store(&aline, "Aline Remera").await;
    let store_b = create_store(&aline, "Aline Kiyovu").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let beans = create_product(&aline, "Beans 1KG").await;
    let rice_sp = attach(&aline, store_a, rice, 5000, json!(5)).await;
    let beans_sp = attach(&aline, store_b, beans, 3500, json!(null)).await;
    set_open(&app.pool, store_a, true).await;
    set_open(&app.pool, store_b, true).await;

    let group: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 2), line(beans_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let group_id = group["id"].as_str().unwrap().to_string();
    let orders = group["store_orders"].as_array().unwrap();
    assert_eq!(orders.len(), 2);
    let order_a = orders
        .iter()
        .find(|o| o["store_id"].as_str().unwrap() == store_a.to_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let order_b = orders
        .iter()
        .find(|o| o["store_id"].as_str().unwrap() == store_b.to_string())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // The reject: cancelled straight from placed, WITH a reason — the
    // why travels with the status now.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{order_a}"),
            json!({ "status": "cancelled", "reason": "customer changed the order" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "placed → cancelled is legal");
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "cancelled"
    );

    // The fulfillment sheet carries the reason to the operator.
    let sheet: Value = aline
        .client
        .get(&format!("/v1/merchant/store-orders/{order_a}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sheet["cancel_reason"], "customer changed the order");

    // The event trail: the reject is the MERCHANT's cancel, distinct from
    // the customer's `placed` that opened the trail.
    let events: Vec<(String, String)> = sqlx::query_as(
        "SELECT status::text, actor_kind::text \
         FROM commerce.order_status_events \
         WHERE store_order_id = $1 ORDER BY created_at, id",
    )
    .bind(Uuid::parse_str(&order_a).unwrap())
    .fetch_all(&app.pool)
    .await
    .unwrap();
    let mut sorted = events;
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            ("cancelled".to_string(), "merchant".to_string()),
            ("placed".to_string(), "customer".to_string()),
        ],
        "the reject records a merchant cancel on top of the customer's placed"
    );

    // The ledger half 1: the 2 reserved units are back on the shelf.
    let (stock,): (i64,) =
        sqlx::query_as("SELECT stock FROM marketplace.store_products WHERE id = $1")
            .bind(rice_sp)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(stock, 5, "the reject returns the reserved stock");

    // The ledger half 2: A's slice can never collect — it refunds.
    let (allocation,): (String,) = sqlx::query_as(
        "SELECT status::text FROM commerce.payment_allocations WHERE store_order_id = $1",
    )
    .bind(Uuid::parse_str(&order_a).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(allocation, "refunded", "the rejected slice refunds");

    // The surviving store B fulfills normally: accepted → preparing →
    // handoff → delivered (the merchant advance settles its allocation).
    for status in ["accepted", "preparing"] {
        let response = aline
            .client
            .patch_json(
                &format!("/v1/merchant/store-orders/{order_b}"),
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
            &format!("/v1/merchant/store-orders/{order_b}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{order_b}"),
            json!({ "status": "delivered" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Nothing pending anywhere: settled B + refunded A ⇒ the group's
    // payment COLLECTS. Pre-fix this stayed `pending` forever.
    let (payment,): (String,) =
        sqlx::query_as("SELECT status::text FROM commerce.payments WHERE order_group_id = $1")
            .bind(Uuid::parse_str(&group_id).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        payment, "collected",
        "a rejected sibling must not strand the group's cash"
    );
}

// ---------------------------------------------------------------------------
// The delivery line is server-owned (geocoding-rebuild slice): with a pin,
// `order_groups.address_text` is derived from the pin and the client's
// text is a fallback at best — a coordinate pair is never a name. Without
// a pin, the client's own line is required, as always.
// ---------------------------------------------------------------------------

#[sqlx::test(migrator = "MIGRATOR")]
async fn checkout_with_a_pin_derives_the_place_and_never_trusts_coordinates(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000015").await;
    // The client "sends" a coordinate pair as its text — the exact abuse
    // the old contract allowed. The pin wins: the derived place name is
    // what gets snapshotted.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "-1.9449, 30.0619",
                "address_lat": -1.9449,
                "address_lng": 30.0619,
                "items": [line(rice_sp, 1)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["address_text"], "Test place 1",
        "the pin names the place — coordinates are never display text"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_pin_less_checkout_still_needs_the_customers_own_line(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 12000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000016").await;
    // No pin and no text: nothing honest to render — refused.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "items": [line(rice_sp, 1)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400, "no pin and no line is refused");

    // With a line, the pin-less checkout lands as it always has.
    let response = chantal
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "KG 7 Ave, Remera",
                "items": [line(rice_sp, 1)],
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["address_text"], "KG 7 Ave, Remera");
}

/// The board feed takes a status filter (W1.5): the Live board asks for
/// the in-flight statuses, History for the settled ones, and absent means
/// every status — the pre-filter behavior, unchanged.
#[sqlx::test(migrator = "MIGRATOR")]
async fn the_board_feed_filters_by_status(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let (chantal, token) = customer_session(&app, "+250780000018").await;

    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let board = |query: &str| {
        aline
            .client
            .get(&format!("/v1/merchant/orders{query}"))
            .bearer_auth(&aline.token)
    };

    // First checkout: stays placed. Second: accepted by the merchant.
    let first: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let placed_id = first["store_orders"][0]["id"].as_str().unwrap();

    let second: Value = checkout(
        &chantal,
        &token,
        "KN 4 Ave, Kigali",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let accepted_id = second["store_orders"][0]["id"].as_str().unwrap();
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{accepted_id}"),
            json!({ "status": "accepted" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Third checkout: handoff to a rider, then delivered (picked_up is
    // the handoff's door only — a bare advance is refused).
    let third: Value = checkout(
        &chantal,
        &token,
        "KK 40 Street, Kigali",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let settled_id = third["store_orders"][0]["id"].as_str().unwrap();
    let rider = seed_rider(&app.pool, "Jean", "+250780000019").await;
    for step in [
        (
            "patch",
            format!("/v1/merchant/store-orders/{settled_id}"),
            json!({ "status": "accepted" }),
        ),
        (
            "patch",
            format!("/v1/merchant/store-orders/{settled_id}"),
            json!({ "status": "preparing" }),
        ),
        (
            "post",
            format!("/v1/merchant/store-orders/{settled_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        ),
        (
            "patch",
            format!("/v1/merchant/store-orders/{settled_id}"),
            json!({ "status": "delivered" }),
        ),
    ] {
        let response = match step.0 {
            "patch" => aline.client.patch_json(&step.1, step.2),
            _ => aline.client.post_json(&step.1, step.2),
        }
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 200, "the settle walk must be legal");
    }

    // The live filter: only in-flight orders.
    let response = board("?status=placed,accepted,preparing,picked_up")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let live: Value = response.json().await.unwrap();
    let live_statuses: Vec<&str> = live
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        live_statuses.len(),
        2,
        "placed + accepted only: {live_statuses:?}"
    );
    assert!(live_statuses.contains(&"placed"));
    assert!(live_statuses.contains(&"accepted"));

    // The history filter: only the settled order.
    let response = board("?status=delivered,cancelled").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let history: Value = response.json().await.unwrap();
    let history_rows = history.as_array().unwrap();
    assert_eq!(history_rows.len(), 1);
    assert_eq!(history_rows[0]["status"], "delivered");
    assert_eq!(history_rows[0]["id"], settled_id);

    // A single status narrows to exactly that status.
    let response = board("?status=placed").send().await.unwrap();
    let placed_only: Value = response.json().await.unwrap();
    let rows = placed_only.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], placed_id);

    // Unknown statuses are refused at the door.
    let response = board("?status=bogus").send().await.unwrap();
    assert_eq!(response.status(), 400, "an unknown status label is a 400");

    // No filter = every status, the pre-existing behavior.
    let response = board("").send().await.unwrap();
    let everything: Value = response.json().await.unwrap();
    assert_eq!(everything.as_array().unwrap().len(), 3);
}

/// The cancel reason is bounded: a 500-character wall of text is a 422,
/// not an unbounded note field.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_cancel_reason_over_five_hundred_chars_is_refused(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;

    let (chantal, token) = customer_session(&app, "+250780000020").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap();

    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{order_id}"),
            json!({ "status": "cancelled", "reason": "x".repeat(501) }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "an over-long reason is refused");
}

/// The popularity counter (migration 14): a checkout line is +1 (per line,
/// not per unit), a cancel gives the lines back — never below zero. This
/// is the write-side twin of search's `empty_query_shows_popular_products_
/// and_open_stores` contract.
#[sqlx::test(migrator = "MIGRATOR")]
async fn checkout_moves_the_popularity_counter(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let beans = create_product(&aline, "Beans 1KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    let beans_sp = attach(&aline, store, beans, 3500, json!(null)).await;
    set_open(&app.pool, store, true).await;

    async fn order_lines(pool: &PgPool, store_product_id: Uuid) -> i64 {
        sqlx::query_scalar(
            "SELECT order_lines FROM commerce.product_popularity WHERE store_product_id = $1",
        )
        .bind(store_product_id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    let (chantal, token) = customer_session(&app, "+250780000021").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 2), line(beans_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(group["store_orders"].as_array().unwrap().len(), 1);

    // One counter row per checkout LINE, +1 each — the 2-unit rice line
    // still counts once.
    assert_eq!(order_lines(&app.pool, rice_sp).await, 1);
    assert_eq!(order_lines(&app.pool, beans_sp).await, 1);

    // Cancelling the store order takes both lines back, stopping at zero.
    let group_id = group["id"].as_str().unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap();
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{order_id}/cancel"),
            json!({ "reason": "changed my mind" }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(order_lines(&app.pool, rice_sp).await, 0);
    assert_eq!(order_lines(&app.pool, beans_sp).await, 0);
}

/// The event trail (migration 15): every transition appends exactly one
/// row — what happened, by whom — inside the change's own transaction.
/// A full walk: placed (checkout) → accepted → preparing (merchant PATCH)
/// → picked_up (handoff by the operator) → delivered (the rider's token).
#[sqlx::test(migrator = "MIGRATOR")]
async fn every_transition_leaves_an_event_trail(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;
    let rider = seed_rider(&app.pool, "Jean", "+250780000002").await;

    let (chantal, token) = customer_session(&app, "+250780000022").await;
    let group: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();

    // accepted → preparing: the merchant operator's PATCH.
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

    // The handoff: the operator types the rider's number.
    let response = aline
        .client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Delivered by the RIDER's own token — a different actor, a real one.
    let (delivery_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM commerce.deliveries WHERE store_order_id = $1")
            .bind(order_id.parse::<Uuid>().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let response = TestClient::new(&app.address)
        .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
        .bearer_auth(token_for(&app, rider.account.id, 3600))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The trail: five rows, one per transition. created_at has microsecond
    // resolution, so the deterministic assertions are the COUNT, the
    // multiset of (status, kind) pairs, and `placed` being the earliest.
    let trail: Vec<(String, String)> = sqlx::query_as(
        "SELECT status::text, actor_kind::text \
         FROM commerce.order_status_events \
         WHERE store_order_id = $1 ORDER BY created_at, id",
    )
    .bind(order_id.parse::<Uuid>().unwrap())
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(trail.len(), 5, "one event per transition: {trail:?}");
    let mut sorted = trail;
    sorted.sort();
    let mut expected = vec![
        ("placed".to_string(), "customer".to_string()),
        ("accepted".to_string(), "merchant".to_string()),
        ("preparing".to_string(), "merchant".to_string()),
        ("picked_up".to_string(), "merchant".to_string()),
        ("delivered".to_string(), "rider".to_string()),
    ];
    expected.sort();
    assert_eq!(sorted, expected);
    let (first_status, first_kind): (String, String) = sqlx::query_as(
        "SELECT status::text, actor_kind::text \
         FROM commerce.order_status_events \
         WHERE store_order_id = $1 ORDER BY created_at, id LIMIT 1",
    )
    .bind(order_id.parse::<Uuid>().unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        (first_status, first_kind),
        ("placed".to_string(), "customer".to_string()),
        "the checkout's own `placed` event opens the trail"
    );
}

/// The ledger's first reader: the customer's group detail carries each
/// store order's status trail in history order — the timeline's source of
/// truth. The checkout response itself stays wire-unchanged (no events
/// key while the trail is empty at that moment).
#[sqlx::test(migrator = "MIGRATOR")]
async fn the_detail_response_carries_the_event_trail(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;
    let rider = seed_rider(&app.pool, "Jean", "+250780000023").await;

    let (chantal, token) = customer_session(&app, "+250780000024").await;
    let response = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await;
    assert_eq!(response.status(), 201);
    let checkout_body: Value = response.json().await.unwrap();
    // The checkout's wire is unchanged: no trail on a brand-new group.
    assert!(
        checkout_body["store_orders"][0].get("events").is_none(),
        "the checkout response carries no events"
    );

    let group_id = checkout_body["id"].as_str().unwrap().to_string();
    let order_id = checkout_body["store_orders"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // accepted → preparing: the merchant operator's PATCH.
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

    // The handoff: the operator types the rider's number.
    let response = aline
        .client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Delivered by the RIDER's own token — a different actor, a real one.
    let (delivery_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM commerce.deliveries WHERE store_order_id = $1")
            .bind(order_id.parse::<Uuid>().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let response = TestClient::new(&app.address)
        .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
        .bearer_auth(token_for(&app, rider.account.id, 3600))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The customer's detail: the trail, in the history's order, with the
    // wire labels and a timestamp per memory row.
    let response = chantal
        .get(&format!("/v1/orders/{group_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let detail: Value = response.json().await.unwrap();
    let events = detail["store_orders"][0]["events"].as_array().unwrap();
    let trail: Vec<(&str, &str)> = events
        .iter()
        .map(|event| {
            (
                event["status"].as_str().unwrap(),
                event["actor"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        trail,
        vec![
            ("placed", "customer"),
            ("accepted", "merchant"),
            ("preparing", "merchant"),
            ("picked_up", "merchant"),
            ("delivered", "rider"),
        ],
        "the trail reads in history order: {trail:?}"
    );
    for event in events {
        let at = event["at"].as_str().unwrap_or_default();
        assert!(
            !at.is_empty(),
            "every memory row carries its moment: {event}"
        );
    }
}

/// Merchant History: the board rows carry the trail and the cancel
/// reason — the receipt drawer's facts, read straight off the ledger.
#[sqlx::test(migrator = "MIGRATOR")]
async fn merchant_history_rows_carry_the_trail_and_reason(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice 5KG").await;
    let rice_sp = attach(&aline, store, rice, 5000, json!(null)).await;
    set_open(&app.pool, store, true).await;
    let rider = seed_rider(&app.pool, "Jean", "+250780000025").await;

    let (chantal, token) = customer_session(&app, "+250780000026").await;

    // Order one: the customer cancels it with a reason while it is placed.
    let first: Value = checkout(
        &chantal,
        &token,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let group_id = first["id"].as_str().unwrap();
    let cancelled_id = first["store_orders"][0]["id"].as_str().unwrap();
    let response = chantal
        .post_json(
            &format!("/v1/orders/{group_id}/store-orders/{cancelled_id}/cancel"),
            json!({ "reason": "changed my mind" }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Order two: walked all the way to delivered by the rider's token.
    let second: Value = checkout(
        &chantal,
        &token,
        "KN 4 Ave, Kigali",
        json!([line(rice_sp, 1)]),
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    let delivered_id = second["store_orders"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    for status in ["accepted", "preparing"] {
        let response = aline
            .client
            .patch_json(
                &format!("/v1/merchant/store-orders/{delivered_id}"),
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
            &format!("/v1/merchant/store-orders/{delivered_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let (delivery_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM commerce.deliveries WHERE store_order_id = $1")
            .bind(delivered_id.parse::<Uuid>().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let response = TestClient::new(&app.address)
        .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
        .bearer_auth(token_for(&app, rider.account.id, 3600))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The History filter: both settled orders, each with its trail.
    let response = aline
        .client
        .get("/v1/merchant/orders?status=delivered,cancelled")
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let history: Value = response.json().await.unwrap();
    let rows = history.as_array().unwrap();
    assert_eq!(rows.len(), 2, "both settled orders: {rows:?}");

    let cancelled_row = rows
        .iter()
        .find(|row| row["status"] == "cancelled")
        .expect("the cancelled order is in History");
    assert_eq!(cancelled_row["cancel_reason"], "changed my mind");
    assert_eq!(cancelled_row["id"], cancelled_id);
    let cancelled_trail = cancelled_row["events"].as_array().unwrap();
    assert_eq!(
        cancelled_trail
            .iter()
            .map(|event| (
                event["status"].as_str().unwrap(),
                event["actor"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        vec![("placed", "customer"), ("cancelled", "customer")],
        "the cancelled trail ends with the customer's cancel"
    );

    let delivered_row = rows
        .iter()
        .find(|row| row["status"] == "delivered")
        .expect("the delivered order is in History");
    assert!(
        delivered_row.get("cancel_reason").is_none(),
        "a living order has no reason to carry"
    );
    let delivered_trail = delivered_row["events"].as_array().unwrap();
    let last = delivered_trail.last().unwrap();
    assert_eq!(
        (
            last["status"].as_str().unwrap(),
            last["actor"].as_str().unwrap()
        ),
        ("delivered", "rider")
    );

    // The fulfillment sheet: the detail response carries the trail too.
    let response = aline
        .client
        .get(&format!("/v1/merchant/store-orders/{cancelled_id}"))
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let detail: Value = response.json().await.unwrap();
    let detail_trail = detail["events"].as_array().unwrap();
    let last = detail_trail.last().unwrap();
    assert_eq!(
        (
            last["status"].as_str().unwrap(),
            last["actor"].as_str().unwrap()
        ),
        ("cancelled", "customer")
    );
    assert_eq!(detail["cancel_reason"], "changed my mind");
}
