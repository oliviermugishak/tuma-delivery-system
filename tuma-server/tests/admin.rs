//! Admin-only management: merchant businesses (create with owner account +
//! membership, list, edit, suspend, delete), customer administration, and
//! the guard that keeps everyone else out.

mod common;

use common::{MIGRATOR, TestClient, login, seed_customer, seed_merchant, spawn_app, token_for};
use serde_json::{Value, json};
use uuid::Uuid;

/// An admin session on its own client.
async fn admin_client(app: &common::TestApp, email: &str) -> TestClient {
    common::seed_admin(&app.pool, email).await;
    let client = TestClient::new(&app.address);
    assert_eq!(login(&client, email, "Password123").await.status(), 204);
    client
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_provision_a_merchant_business_with_its_owner(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .post_json(
            "/v1/admin/merchants",
            json!({
                "name": "Aline's Kitchen",
                "business_email": "biz@alinekitchen.rw",
                "email": "Aline@Example.com",
                "password": "Merchant123"
            }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Aline's Kitchen");
    assert_eq!(body["business_email"], "biz@alinekitchen.rw");
    assert_eq!(body["owner_email"], "aline@example.com"); // stored lowercase
    assert_eq!(body["status"], "active");
    assert!(body.get("password_hash").is_none());
    let merchant_id = body["id"].as_str().unwrap().to_string();

    // The owner signs into the merchant wing with the password the admin chose.
    let owner_client = TestClient::new(&app.address);
    assert_eq!(
        login(&owner_client, "aline@example.com", "Merchant123")
            .await
            .status(),
        204
    );

    // And /me ties the account to the business through the membership.
    let response = owner_client.get("/v1/me").send().await.unwrap();
    let body: Value = response.json().await.unwrap();
    let memberships = body["merchant_memberships"].as_array().unwrap();
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0]["merchant_id"], merchant_id);
    assert_eq!(memberships[0]["role"], "owner");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn create_merchant_rejects_a_duplicate_owner_email(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .post_json(
            "/v1/admin/merchants",
            json!({
                "name": "First Kitchen",
                "email": "aline@example.com",
                "password": "Merchant123"
            }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    // Same address in different case must collide too — and the error must
    // name the identity that actually collided.
    let response = client
        .post_json(
            "/v1/admin/merchants",
            json!({
                "name": "Second Kitchen",
                "email": "ALINE@example.com",
                "password": "Merchant123"
            }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "a user with this email already exists");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_list_merchant_businesses(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    seed_merchant(&app.pool, "one@example.com", "One Business").await;
    seed_merchant(&app.pool, "two@example.com", "Two Business").await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client.get("/v1/admin/merchants").send().await.unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    let merchants = body.as_array().expect("a list of businesses");
    assert_eq!(merchants.len(), 2);
    let names: Vec<&str> = merchants
        .iter()
        .map(|merchant| merchant["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"One Business"));
    assert!(names.contains(&"Two Business"));
    assert!(
        merchants
            .iter()
            .all(|merchant| merchant["status"] == "active")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_suspend_and_reactivate_a_business(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let seeded = seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", seeded.merchant.id),
            json!({ "status": "suspended" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "suspended");

    // A suspended business still exists in detail.
    let response = client
        .get(&format!("/v1/admin/merchants/{}", seeded.merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["status"],
        "suspended"
    );

    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", seeded.merchant.id),
            json!({ "status": "active" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap()["status"], "active");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_merchant_routes_404_for_unknown_ids(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    for id in [Uuid::new_v4()] {
        let response = client
            .get(&format!("/v1/admin/merchants/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no business at {id}");

        let response = client
            .patch_json(
                &format!("/v1/admin/merchants/{id}"),
                json!({ "status": "suspended" }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no business at {id}");

        let response = client
            .delete(&format!("/v1/admin/merchants/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no business at {id}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_sees_a_business_in_detail(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let seeded = seed_merchant(&app.pool, "owner@example.com", "Aline's Kitchen").await;
    let customer = seed_customer(&app.pool, "+250780003001").await;

    // The owner builds two stores.
    let owner_client = TestClient::new(&app.address);
    assert_eq!(
        login(&owner_client, "owner@example.com", "Password123")
            .await
            .status(),
        204
    );
    let mut store_ids = Vec::new();
    for name in ["Kitchen", "Coffee Stand"] {
        let response = owner_client
            .post_json("/v1/merchant/stores", json!({ "name": name }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body: Value = response.json().await.unwrap();
        store_ids.push(body["id"].as_str().unwrap().to_string());
    }

    let response = client
        .get(&format!("/v1/admin/merchants/{}", seeded.merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Aline's Kitchen");

    let stores = body["stores"].as_array().expect("a list of stores");
    assert_eq!(stores.len(), 2);
    assert_eq!(stores[0]["product_count"], 0);
    assert_eq!(stores[0]["is_open"], false);

    let members = body["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["email"], "owner@example.com");
    assert_eq!(members[0]["role"], "owner");

    // Unknown ids are 404s. A customer id is not a business id.
    for id in [Uuid::new_v4(), customer.account.id] {
        let response = client
            .get(&format!("/v1/admin/merchants/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no business at {id}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_sees_the_platform_summary(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_merchant(&app.pool, "one@example.com", "One Business").await;
    seed_merchant(&app.pool, "two@example.com", "Two Business").await;
    seed_customer(&app.pool, "+250780003002").await;

    // One business has an open store and a closed one.
    common::seed_store(&app.pool, one.merchant.id, "Open Kitchen", true).await;
    common::seed_store(&app.pool, one.merchant.id, "Closed Corner", false).await;

    let response = client.get("/v1/admin/summary").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["merchants"], 2);
    assert_eq!(body["customers"], 1);
    assert_eq!(body["stores"], 2);
    assert_eq!(body["open_stores"], 1);
    assert_eq!(body["products"], 0);
    assert_eq!(body["store_products"], 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_manages_customers(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_customer(&app.pool, "+250780004001").await;
    seed_customer(&app.pool, "+250780004002").await;
    let merchant = seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;

    // The list is customers only.
    let response = client.get("/v1/admin/customers").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    let customers = body.as_array().expect("a list of customers");
    assert_eq!(customers.len(), 2);

    // Deactivating locks the customer out immediately (fresh account lookup
    // per request); reactivating lets them back in.
    let token = token_for(&app, one.account.id, 3600);
    let me_client = TestClient::new(&app.address);
    assert_eq!(
        me_client
            .get("/v1/me")
            .bearer_auth(token.clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", one.account.id),
            json!({ "is_active": false }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["is_active"], false);
    assert_eq!(
        me_client
            .get("/v1/me")
            .bearer_auth(token.clone())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // The toggle is scoped to customers: an operator's account is a 404.
    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", merchant.account.id),
            json!({ "is_active": false }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // Identity edits: name and phone overwrite, absent fields keep their
    // value. The corrected phone frees the customer to keep using the app.
    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", one.account.id),
            json!({ "name": "Mugisha Corrected", "phone": "+250780004009" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Mugisha Corrected");
    assert_eq!(body["phone"], "+250780004009");

    // A phone another customer already holds is a 409.
    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", one.account.id),
            json!({ "phone": "+250780004002" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);

    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", one.account.id),
            json!({ "is_active": true }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        me_client
            .get("/v1/me")
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    // Deleting is scoped too: an operator's id is a 404, a customer's id is
    // a 204 and the list shrinks.
    let response = client
        .delete(&format!("/v1/admin/customers/{}", merchant.account.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    let response = client
        .delete(&format!("/v1/admin/customers/{}", one.account.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let response = client.get("/v1/admin/customers").send().await.unwrap();
    let body: Value = response.json().await.unwrap();
    let customers = body.as_array().expect("a list of customers");
    assert_eq!(customers.len(), 1);
    assert_eq!(customers[0]["phone"], "+250780004002");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_edits_and_deletes_a_business(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_merchant(&app.pool, "one@example.com", "One Business").await;
    seed_merchant(&app.pool, "two@example.com", "Two Business").await;

    // Business edits: name + contact overwrite in one PATCH.
    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", one.merchant.id),
            json!({ "name": "Renamed Kitchen", "business_phone": "+250788000111" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Renamed Kitchen");
    assert_eq!(body["business_phone"], "+250788000111");

    // The owner builds a store, then the admin deletes the business: the
    // stores and assortments must follow via cascade.
    let owner_client = TestClient::new(&app.address);
    assert_eq!(
        login(&owner_client, "one@example.com", "Password123")
            .await
            .status(),
        204
    );
    owner_client
        .post_json("/v1/merchant/stores", json!({ "name": "Kitchen" }))
        .send()
        .await
        .unwrap();

    let response = client
        .delete(&format!("/v1/admin/merchants/{}", one.merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let response = client
        .get(&format!("/v1/admin/merchants/{}", one.merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // The cascade reached the marketplace: one business, no stores. The
    // owner's ACCOUNT survives (it is an identity, not part of the
    // business) — but it no longer unlocks the merchant wing.
    let response = client.get("/v1/admin/summary").send().await.unwrap();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["merchants"], 1);
    assert_eq!(body["stores"], 0);

    let client = TestClient::new(&app.address);
    assert_eq!(
        login(&client, "one@example.com", "Password123")
            .await
            .status(),
        401,
        "a membership-less password account cannot enter the platform"
    );
}

/// One customer order through the business — the order history a delete
/// must refuse to rewrite.
async fn place_order_with_merchant(app: &common::TestApp, merchant_id: uuid::Uuid) {
    let (store_product_id,): (Uuid,) = sqlx::query_as(
        "SELECT sp.id FROM marketplace.store_products sp \
         JOIN marketplace.stores s ON s.id = sp.store_id \
         WHERE s.merchant_id = $1 LIMIT 1",
    )
    .bind(merchant_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let seeded = seed_customer(&app.pool, "+250780099003").await;
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

/// S28: the store_orders/payment_allocations FKs have no ON DELETE —
/// before the typed pre-check, deleting a business that ever appeared in
/// an order was a 500 from the constraint. Now it is a 409 naming the
/// remedy (suspend).
#[sqlx::test(migrator = "MIGRATOR")]
async fn deleting_a_merchant_with_order_history_is_a_409_not_a_500(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_merchant(&app.pool, "one@example.com", "One Business").await;
    let two = seed_merchant(&app.pool, "two@example.com", "Two Business").await;

    // Stock the business's store, open it, and take one real order through it.
    let store = common::seed_store(&app.pool, one.merchant.id, "One Kitchen", true).await;
    let owner_client = TestClient::new(&app.address);
    assert_eq!(
        login(&owner_client, "one@example.com", "Password123")
            .await
            .status(),
        204
    );
    let product = owner_client
        .post_json("/v1/merchant/products", json!({ "name": "Rice 5KG" }))
        .send()
        .await
        .unwrap();
    assert_eq!(product.status(), 201);
    let product_id: Uuid = product.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let attach = owner_client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store.id, "price": 5000, "stock": 5 }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), 201);
    place_order_with_merchant(&app, one.merchant.id).await;

    // The delete is refused with its remedy, not a 500.
    let response = client
        .delete(&format!("/v1/admin/merchants/{}", one.merchant.id))
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
        body["message"].as_str().unwrap().contains("suspend"),
        "the message names the remedy: {body}"
    );

    // The business survived.
    assert_eq!(
        client
            .get(&format!("/v1/admin/merchants/{}", one.merchant.id))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    // A business with no history still deletes cleanly.
    let response = client
        .delete(&format!("/v1/admin/merchants/{}", two.merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204, "no history deletes cleanly");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_routes_reject_non_admins(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    // A signed-in operator gets 403, not 401.
    seed_merchant(&app.pool, "merchant@example.com", "Aline's Kitchen").await;
    let merchant_client = TestClient::new(&app.address);
    assert_eq!(
        login(&merchant_client, "merchant@example.com", "Password123")
            .await
            .status(),
        204
    );
    assert_eq!(
        merchant_client
            .get("/v1/admin/merchants")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        merchant_client
            .post_json(
                "/v1/admin/merchants",
                json!({ "name": "X", "email": "x@example.com", "password": "Merchant123" }),
            )
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    // A customer bearer token gets the same 403.
    let customer = seed_customer(&app.pool, "+250780002001").await;
    let token = token_for(&app, customer.account.id, 3600);
    let response = TestClient::new(&app.address)
        .get("/v1/admin/merchants")
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);

    // No credentials at all → 401.
    let response = TestClient::new(&app.address)
        .get("/v1/admin/merchants")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

/// S34: the admin lists are paged — limit/offset clamp like the orders
/// PageQuery (1..=200, default 50), so an unbounded page can never be
/// fetched, and paging actually moves through the rows.
#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_lists_page_with_limit_and_offset(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    for (email, name) in [
        ("m1@example.com", "One Business"),
        ("m2@example.com", "Two Business"),
        ("m3@example.com", "Three Business"),
    ] {
        seed_merchant(&app.pool, email, name).await;
    }

    // limit=2: only the first two (oldest) rows.
    let page: Value = client
        .get("/v1/admin/merchants?limit=2&offset=0")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let merchants = page.as_array().unwrap();
    assert_eq!(merchants.len(), 2);
    assert_eq!(merchants[0]["name"], "One Business");
    assert_eq!(merchants[1]["name"], "Two Business");

    // offset=2: the rest of the list.
    let page: Value = client
        .get("/v1/admin/merchants?limit=2&offset=2")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let merchants = page.as_array().unwrap();
    assert_eq!(merchants.len(), 1);
    assert_eq!(merchants[0]["name"], "Three Business");

    // Absent limit means the default 50, not everything forever — the
    // default page is already bigger than this dataset.
    let page: Value = client
        .get("/v1/admin/merchants")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.as_array().unwrap().len(), 3);

    // The clamp is shared: 0 and 1000 both normalize (1 and 200).
    let page: Value = client
        .get("/v1/admin/merchants?limit=0")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1, "limit=0 clamps to 1");

    // Customers page the same way.
    seed_customer(&app.pool, "+250780005001").await;
    seed_customer(&app.pool, "+250780005002").await;
    let page: Value = client
        .get("/v1/admin/customers?limit=1")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1);

    // Riders too.
    client
        .post_json(
            "/v1/admin/riders",
            json!({ "name": "Jean", "phone": "+250780005003" }),
        )
        .send()
        .await
        .unwrap();
    client
        .post_json(
            "/v1/admin/riders",
            json!({ "name": "Eric", "phone": "+250780005004" }),
        )
        .send()
        .await
        .unwrap();
    let page: Value = client
        .get("/v1/admin/riders?limit=1&offset=1")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let riders = page.as_array().unwrap();
    assert_eq!(riders.len(), 1);
    assert_eq!(riders[0]["name"], "Eric", "offset skips the oldest rider");
}

/// W3.5: the summary's stale-delivery count — picked_up orders whose ETA
/// lapsed past the 15-minute grace with nobody marking them delivered.
/// The operator's "needs attention" line on the overview.
#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_summary_counts_stale_deliveries(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_merchant(&app.pool, "one@example.com", "One Business").await;

    // Stock the business's store, open it, and take one real order.
    let store = common::seed_store(&app.pool, one.merchant.id, "One Kitchen", true).await;
    let owner_client = TestClient::new(&app.address);
    assert_eq!(
        login(&owner_client, "one@example.com", "Password123")
            .await
            .status(),
        204
    );
    let product = owner_client
        .post_json("/v1/merchant/products", json!({ "name": "Rice 5KG" }))
        .send()
        .await
        .unwrap();
    assert_eq!(product.status(), 201);
    let product_id: Uuid = product.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let attach = owner_client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store.id, "price": 5000, "stock": 5 }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), 201);
    place_order_with_merchant(&app, one.merchant.id).await;

    let (order_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM commerce.store_orders ORDER BY created_at DESC LIMIT 1")
            .fetch_one(&app.pool)
            .await
            .unwrap();

    // accepted → preparing, then handoff with a seeded rider: the order is
    // out for delivery.
    for status in ["accepted", "preparing"] {
        let response = owner_client
            .patch_json(
                &format!("/v1/merchant/store-orders/{order_id}"),
                json!({ "status": status }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "advance to {status}");
    }
    let rider = common::seed_rider(&app.pool, "Jean", "+250780003013").await;
    let response = owner_client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": rider.rider.rider_number }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Fresh handoff: nothing stale (the ETA is current, and a NULL
    // route-less ETA self-filters out of the comparison either way).
    let body: Value = client
        .get("/v1/admin/summary")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["stale_deliveries"], 0);

    // Backdate the ETA past the 15-minute grace (the direct-SQL fixture
    // precedent): exactly one delivery needs attention.
    sqlx::query("UPDATE commerce.deliveries SET eta_target = now() - interval '1 hour' WHERE store_order_id = $1")
        .bind(order_id)
        .execute(&app.pool)
        .await
        .unwrap();
    let body: Value = client
        .get("/v1/admin/summary")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["stale_deliveries"], 1);
}
