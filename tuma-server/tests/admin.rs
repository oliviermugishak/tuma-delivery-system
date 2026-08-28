//! Admin-only merchant management: create, list, deactivate — and the role
//! guard that keeps everyone else out (S5).

mod common;

use accounts::UserRole;
use common::{MIGRATOR, TestClient, login, seed_customer, seed_staff, spawn_app, token_for};
use serde_json::{Value, json};
use uuid::Uuid;

/// An admin session on its own client.
async fn admin_client(app: &common::TestApp, email: &str) -> TestClient {
    seed_staff(&app.pool, UserRole::Admin, email).await;
    let client = TestClient::new(&app.address);
    assert_eq!(login(&client, email, "Password123").await.status(), 204);
    client
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_create_a_merchant(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .post_json(
            "/v1/admin/merchants",
            json!({
                "name": "Aline's Kitchen",
                "email": "Aline@Example.com",
                "password": "Merchant123"
            }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["role"], "merchant");
    assert_eq!(body["email"], "aline@example.com"); // stored lowercase
    assert_eq!(body["name"], "Aline's Kitchen");
    assert!(body["phone"].is_null()); // phone is customer identity only
    assert_eq!(body["is_active"], true);
    assert!(body.get("password_hash").is_none());

    // The merchant signs in with the password the admin chose.
    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "aline@example.com",
            "Merchant123"
        )
        .await
        .status(),
        204
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn create_merchant_rejects_a_duplicate_email(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .post_json(
            "/v1/admin/merchants",
            json!({ "email": "aline@example.com", "password": "Merchant123" }),
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
            json!({ "email": "ALINE@example.com", "password": "Merchant123" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "a user with this email already exists");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_list_merchants(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    seed_staff(&app.pool, UserRole::Merchant, "one@example.com").await;
    seed_staff(&app.pool, UserRole::Merchant, "two@example.com").await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client.get("/v1/admin/merchants").send().await.unwrap();
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    let merchants = body.as_array().expect("a list of merchants");
    assert_eq!(merchants.len(), 2, "admins are not merchants");
    let emails: Vec<&str> = merchants
        .iter()
        .map(|merchant| merchant["email"].as_str().unwrap())
        .collect();
    assert!(emails.contains(&"one@example.com"));
    assert!(emails.contains(&"two@example.com"));
    assert!(
        merchants
            .iter()
            .all(|merchant| merchant["role"] == "merchant")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_can_deactivate_and_reactivate_a_merchant(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let merchant = seed_staff(&app.pool, UserRole::Merchant, "merchant@example.com").await;
    let client = admin_client(&app, "admin@example.com").await;

    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", merchant.id),
            json!({ "is_active": false }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["is_active"], false);

    // A deactivated merchant cannot log in.
    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "merchant@example.com",
            "Password123"
        )
        .await
        .status(),
        401
    );

    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", merchant.id),
            json!({ "is_active": true }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["is_active"], true);

    assert_eq!(
        login(
            &TestClient::new(&app.address),
            "merchant@example.com",
            "Password123"
        )
        .await
        .status(),
        204
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn set_merchant_active_404s_for_non_merchants(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let admin = seed_staff(&app.pool, UserRole::Admin, "admin@example.com").await;
    let client = TestClient::new(&app.address);
    assert_eq!(
        login(&client, "admin@example.com", "Password123")
            .await
            .status(),
        204
    );

    // Unknown id, and an admin's id (the update is scoped to role=merchant).
    for id in [Uuid::new_v4(), admin.id] {
        let response = client
            .patch_json(
                &format!("/v1/admin/merchants/{id}"),
                json!({ "is_active": false }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no merchant at {id}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_sees_a_merchant_in_detail(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let merchant = seed_staff(&app.pool, UserRole::Merchant, "merchant@example.com").await;
    let customer = seed_customer(&app.pool, "+250780003001").await;

    // The merchant builds a small catalog: two stores, three products.
    let merchant_client = TestClient::new(&app.address);
    assert_eq!(
        login(&merchant_client, "merchant@example.com", "Password123")
            .await
            .status(),
        204
    );
    let mut store_ids = Vec::new();
    for name in ["Kitchen", "Coffee Stand"] {
        let response = merchant_client
            .post_json("/v1/merchant/stores", json!({ "name": name }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body: Value = response.json().await.unwrap();
        store_ids.push(body["id"].as_str().unwrap().to_string());
    }
    for (i, price) in [3500i64, 1200, 500].into_iter().enumerate() {
        let response = merchant_client
            .post_json(
                "/v1/merchant/products",
                json!({
                    "store_id": store_ids[i % 2],
                    "name": format!("Dish {}", i + 1),
                    "price": price
                }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }

    let response = client
        .get(&format!("/v1/admin/merchants/{}", merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["email"], "merchant@example.com");
    assert_eq!(body["role"], "merchant");

    let stores = body["stores"].as_array().expect("a list of stores");
    assert_eq!(stores.len(), 2);
    let first = stores
        .iter()
        .find(|store| store["id"] == store_ids[0])
        .expect("the first store");
    assert_eq!(first["product_count"], 2);
    let second = stores
        .iter()
        .find(|store| store["id"] == store_ids[1])
        .expect("the second store");
    assert_eq!(second["product_count"], 1);

    // The menu itself stays out of the admin's response — the merchant
    // manages it. Unknown ids and non-merchant ids are the same 404.
    for id in [Uuid::new_v4(), customer.id] {
        let response = client
            .get(&format!("/v1/admin/merchants/{id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404, "no merchant at {id}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_sees_the_platform_summary(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    seed_staff(&app.pool, UserRole::Merchant, "one@example.com").await;
    seed_staff(&app.pool, UserRole::Merchant, "two@example.com").await;
    seed_customer(&app.pool, "+250780003002").await;

    // One merchant opens a store with products; the other stays empty.
    let merchant_client = TestClient::new(&app.address);
    assert_eq!(
        login(&merchant_client, "two@example.com", "Password123")
            .await
            .status(),
        204
    );
    let response = merchant_client
        .post_json("/v1/merchant/stores", json!({ "name": "Kitchen" }))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    let store_id = body["id"].as_str().unwrap();
    merchant_client
        .patch_json(
            &format!("/v1/merchant/stores/{store_id}"),
            json!({ "is_open": true }),
        )
        .send()
        .await
        .unwrap();
    merchant_client
        .post_json("/v1/merchant/stores", json!({ "name": "Closed Corner" }))
        .send()
        .await
        .unwrap();
    for price in [3500i64, 1200] {
        merchant_client
            .post_json(
                "/v1/merchant/products",
                json!({
                    "store_id": store_id,
                    "name": format!("Dish {price}"),
                    "price": price
                }),
            )
            .send()
            .await
            .unwrap();
    }

    let response = client.get("/v1/admin/summary").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["merchants"], 2);
    assert_eq!(body["customers"], 1);
    assert_eq!(body["stores"], 2);
    assert_eq!(body["open_stores"], 1);
    assert_eq!(body["products"], 2);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_manages_customers(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_customer(&app.pool, "+250780004001").await;
    seed_customer(&app.pool, "+250780004002").await;
    let merchant = seed_staff(&app.pool, UserRole::Merchant, "merchant@example.com").await;

    // The list is customers only.
    let response = client.get("/v1/admin/customers").send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    let customers = body.as_array().expect("a list of customers");
    assert_eq!(customers.len(), 2);
    assert!(customers.iter().all(|c| c["role"] == "customer"));

    // Deactivating locks the customer out immediately (fresh user lookup per
    // request); reactivating lets them back in.
    let token = token_for(&app, &one, 3600);
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
            &format!("/v1/admin/customers/{}", one.id),
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

    // The toggle is scoped to customers: a merchant's id is a 404.
    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", merchant.id),
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
            &format!("/v1/admin/customers/{}", one.id),
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
            &format!("/v1/admin/customers/{}", one.id),
            json!({ "phone": "+250780004002" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);

    let response = client
        .patch_json(
            &format!("/v1/admin/customers/{}", one.id),
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

    // Deleting is scoped too: a merchant's id is a 404, a customer's id is
    // a 204 and the list shrinks.
    let response = client
        .delete(&format!("/v1/admin/customers/{}", merchant.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    let response = client
        .delete(&format!("/v1/admin/customers/{}", one.id))
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
async fn admin_edits_and_deletes_a_merchant(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = admin_client(&app, "admin@example.com").await;
    let one = seed_staff(&app.pool, UserRole::Merchant, "one@example.com").await;
    seed_staff(&app.pool, UserRole::Merchant, "two@example.com").await;

    // Identity edits: name + email overwrite in one PATCH.
    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", one.id),
            json!({ "name": "Renamed Kitchen", "email": "renamed@example.com" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Renamed Kitchen");
    assert_eq!(body["email"], "renamed@example.com");

    // An email another merchant already holds is a 409.
    let response = client
        .patch_json(
            &format!("/v1/admin/merchants/{}", one.id),
            json!({ "email": "two@example.com" }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);

    // The merchant builds a store with a product, then the admin deletes
    // the account: the store and product must follow via cascade.
    let merchant_client = TestClient::new(&app.address);
    assert_eq!(
        login(&merchant_client, "renamed@example.com", "Password123")
            .await
            .status(),
        204
    );
    let response = merchant_client
        .post_json("/v1/merchant/stores", json!({ "name": "Kitchen" }))
        .send()
        .await
        .unwrap();
    let body: Value = response.json().await.unwrap();
    let store_id = body["id"].as_str().unwrap();
    merchant_client
        .post_json(
            "/v1/merchant/products",
            json!({ "store_id": store_id, "name": "Dish", "price": 3500 }),
        )
        .send()
        .await
        .unwrap();

    let response = client
        .delete(&format!("/v1/admin/merchants/{}", one.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let response = client
        .get(&format!("/v1/admin/merchants/{}", one.id))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    // The cascade reached the catalog: one merchant, no stores, no products.
    let response = client.get("/v1/admin/summary").send().await.unwrap();
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["merchants"], 1);
    assert_eq!(body["stores"], 0);
    assert_eq!(body["products"], 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn admin_routes_reject_non_admins(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    // A signed-in merchant gets 403, not 401.
    seed_staff(&app.pool, UserRole::Merchant, "merchant@example.com").await;
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
                json!({ "email": "x@example.com", "password": "Merchant123" }),
            )
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    // A customer bearer token gets the same 403.
    let customer = seed_customer(&app.pool, "+250780002001").await;
    let token = token_for(&app, &customer, 3600);
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
