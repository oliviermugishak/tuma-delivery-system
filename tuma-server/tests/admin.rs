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
