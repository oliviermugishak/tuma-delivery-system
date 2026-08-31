//! Slice D1: Tuma-owned riders. Admin creates riders (account + profile +
//! unique number); a rider signs in with the same OTP flow as customers but
//! lands on rider mode — no customer profile is ensured, `/me` carries the
//! rider profile. Admin edit/deactivate/delete mirrors customer
//! administration; delete is blocked by delivery history.

mod common;

use common::{MIGRATOR, TestClient, login, seed_customer, seed_merchant, spawn_app, token_for};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const DEV_CODE: &str = "123456"; // configuration/local.yml auth.dev_otp_code

async fn admin_client(app: &common::TestApp, email: &str) -> TestClient {
    common::seed_admin(&app.pool, email).await;
    let client = TestClient::new(&app.address);
    assert_eq!(login(&client, email, "Password123").await.status(), 204);
    client
}

async fn create_rider(client: &TestClient, name: &str, phone: &str) -> Value {
    let response = client
        .post_json("/v1/admin/riders", json!({ "name": name, "phone": phone }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json().await.unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_creates_riders_with_unique_numbers(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    let jean = create_rider(&client, "Jean", "+250780000002").await;
    assert_eq!(jean["name"], "Jean");
    assert_eq!(jean["phone"], "+250780000002");
    assert_eq!(jean["is_active"], true);
    assert!(jean["rider_number"].is_i64(), "the number is generated");

    // The rider list carries the numbers the merchant will ask for.
    let response = client.get("/v1/admin/riders").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let list: Value = response.json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // A second rider gets a different number — merchants type these.
    let aline = create_rider(&client, "Aline Uwase", "+250780000003").await;
    assert_ne!(jean["rider_number"], aline["rider_number"]);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn rider_signs_in_via_otp_into_rider_mode(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let rider = create_rider(&client, "Jean", "+250780000002").await;

    // The exact customer OTP flow, on the rider's phone.
    let phone_client = TestClient::new(&app.address);
    assert_eq!(
        phone_client
            .post_json("/v1/auth/otp/request", json!({ "phone": "+250780000002" }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let response = phone_client
        .post_json(
            "/v1/auth/otp/verify",
            json!({ "phone": "+250780000002", "code": DEV_CODE }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();

    // Rider mode is the surface: the rider profile is there, and NO
    // customer profile was ensured alongside it.
    assert_eq!(body["user"]["rider"]["rider_number"], rider["rider_number"]);
    assert_eq!(body["user"]["rider"]["name"], "Jean");
    assert!(
        body["user"]["customer"].is_null(),
        "a rider account has no customer profile"
    );
    let token = body["token"].as_str().unwrap().to_string();

    // /me agrees, and the rider cannot act as a customer.
    let response = phone_client
        .get("/v1/me")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let me: Value = response.json().await.unwrap();
    assert_eq!(me["rider"]["rider_number"], rider["rider_number"]);
    assert!(me["customer"].is_null());

    assert_eq!(
        phone_client
            .get("/v1/stores")
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn rider_phone_collisions_are_typed_conflicts(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    // An existing customer's phone — the typed identity-collision answer.
    seed_customer(&app.pool, "+250780000009").await;
    let response = client
        .post_json(
            "/v1/admin/riders",
            json!({ "name": "Jean", "phone": "+250780000009" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["message"],
        "a user with this phone number already exists"
    );

    // A rider's phone cannot be re-used for a second rider either.
    create_rider(&client, "Jean", "+250780000002").await;
    let response = client
        .post_json(
            "/v1/admin/riders",
            json!({ "name": "Eric", "phone": "+250780000002" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn phones_without_a_country_code_are_rejected(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    // The admin form's free-text field is the door the local format walks
    // through — the server refuses it rather than guessing a country: two
    // spellings of one number must never become two identities.
    for phone in ["0783002002", "250783002002", "783002002"] {
        let response = client
            .post_json(
                "/v1/admin/riders",
                json!({ "name": "Jean", "phone": phone }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 422, "phone {phone} should be rejected");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_edits_and_deactivates_a_rider(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let rider = create_rider(&client, "Jean", "+250780000002").await;
    let id = &rider["id"].as_str().unwrap();

    // Provided overwrites, absent keeps: rename only.
    let response = client
        .patch_json(
            &format!("/v1/admin/riders/{id}"),
            json!({ "name": "Jean Bosco" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Jean Bosco");
    assert_eq!(body["phone"], "+250780000002");
    assert_eq!(body["is_active"], true);

    // Deactivate — the assignability tool.
    let response = client
        .patch_json(
            &format!("/v1/admin/riders/{id}"),
            json!({ "is_active": false }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap()["is_active"], false);

    // A rider name is NOT NULL: an empty edit is a 400, not a clear.
    let response = client
        .patch_json(&format!("/v1/admin/riders/{id}"), json!({ "name": "  " }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // Phone edit moves the OTP anchor too — and a taken phone is the 409.
    seed_customer(&app.pool, "+250780000009").await;
    let response = client
        .patch_json(
            &format!("/v1/admin/riders/{id}"),
            json!({ "phone": "+250780000009" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);

    // A good phone edit lands in both places — the account (OTP anchor) now
    // answers to the new phone, which is what the rider signs in with.
    let response = client
        .patch_json(
            &format!("/v1/admin/riders/{id}"),
            json!({ "phone": "+250780000010" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let mut conn = app.pool.acquire().await.unwrap();
    let (account_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM accounts.users WHERE phone = '+250780000010'")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    let (rider_account,): (Uuid,) =
        sqlx::query_as("SELECT account_id FROM commerce.riders WHERE id = $1")
            .bind(uuid_str(&rider["id"]))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(account_id, rider_account);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn rider_delete_is_blocked_by_delivery_history(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    // A real checkout creates a delivery row.
    let merchant = seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let op_token = token_for(&app, merchant.account.id, 3600);
    let op = TestClient::new(&app.address);
    let store = json_must(
        op.post_json(
            "/v1/merchant/stores",
            json!({ "name": "Remera", "delivery_fee": 1000 }),
        )
        .bearer_auth(&op_token)
        .send()
        .await
        .unwrap(),
    )
    .await;
    // New stores start closed; opening is a deliberate act (orders.rs pattern).
    let mut conn = app.pool.acquire().await.unwrap();
    sqlx::query("UPDATE marketplace.stores SET is_open = true WHERE id = $1")
        .bind(uuid_str(&store["id"]))
        .execute(&mut *conn)
        .await
        .unwrap();
    let product = json_must(
        op.post_json("/v1/merchant/products", json!({ "name": "Rice" }))
            .bearer_auth(&op_token)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let sp = json_must(
        op.post_json(
            "/v1/merchant/store-products",
            json!({
                "product_id": product["id"],
                "store_id": store["id"],
                "price": 5000,
                "stock": null
            }),
        )
        .bearer_auth(&op_token)
        .send()
        .await
        .unwrap(),
    )
    .await;
    let customer = seed_customer(&app.pool, "+250780000009").await;
    let cust_token = token_for(&app, customer.account.id, 3600);
    let cust_client = TestClient::new(&app.address);
    let group = json_must(
        cust_client
            .post_json(
                "/v1/orders",
                json!({
                    "address_text": "KG 7 Ave",
                    "items": [{ "store_product_id": sp["id"], "quantity": 1 }]
                }),
            )
            .bearer_auth(&cust_token)
            .send()
            .await
            .unwrap(),
    )
    .await;

    // Assign the rider to that delivery the way D2's handoff will.
    let rider = create_rider(&client, "Jean", "+250780000002").await;
    let mut conn = app.pool.acquire().await.unwrap();
    sqlx::query(
        "UPDATE commerce.deliveries d SET rider_id = $1 \
         FROM commerce.store_orders so \
         WHERE d.store_order_id = so.id AND so.order_group_id = $2",
    )
    .bind(uuid_str(&rider["id"]))
    .bind(uuid_str(&group["id"]))
    .execute(&mut *conn)
    .await
    .unwrap();

    // Delete is blocked with the typed 409 — history survives.
    let response = client
        .delete(&format!(
            "/v1/admin/riders/{}",
            rider["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["message"],
        "this rider has delivery history — deactivate instead"
    );

    // A rider with no history deletes cleanly — the typo remedy.
    let fresh = create_rider(&client, "Eric", "+250780000003").await;
    let response = client
        .delete(&format!(
            "/v1/admin/riders/{}",
            fresh["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
}

async fn json_must(response: reqwest::Response) -> Value {
    response.json().await.unwrap()
}

fn uuid_str(value: &Value) -> Uuid {
    value.as_str().unwrap().parse().unwrap()
}
