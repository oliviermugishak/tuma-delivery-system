//! Slice D2: handoff + tracking plumbing. The merchant assigns by rider
//! number (`picked_up`), the rider's phone breadcrumbs the run (≥25m/15s)
//! and marks it delivered — settling the delivery's payment allocation
//! (the cash state) — and the customer polls tracking, which answers 204
//! when nothing changed. Anti-probe everywhere: foreign riders/customers
//! see 404, indistinguishable from missing.

mod common;

use common::{
    MIGRATOR, TestClient, seed_customer, seed_merchant, seed_rider, spawn_app, token_for,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

struct Operator {
    client: TestClient,
    token: String,
}

async fn owner(app: &common::TestApp, email: &str, business: &str) -> Operator {
    let seeded = seed_merchant(&app.pool, email, business).await;
    Operator {
        client: TestClient::new(&app.address),
        token: token_for(app, seeded.account.id, 3600),
    }
}

async fn customer_session(app: &common::TestApp, phone: &str) -> (TestClient, String) {
    let seeded = seed_customer(&app.pool, phone).await;
    (
        TestClient::new(&app.address),
        token_for(app, seeded.account.id, 3600),
    )
}

async fn rider_session(app: &common::TestApp, name: &str, phone: &str) -> common::RiderSeed {
    seed_rider(&app.pool, name, phone).await
}

async fn create_store(op: &Operator, name: &str) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/stores",
            json!({ "name": name, "delivery_fee": 1500, "lat": -1.9620, "lng": 30.1290 }),
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

async fn attach(op: &Operator, store_id: Uuid, product_id: Uuid, price: i64) -> Uuid {
    let response = op
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "product_id": product_id, "store_id": store_id, "price": price, "stock": null }),
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

async fn checkout(client: &TestClient, token: &str, address: &str, items: Value) -> Value {
    client
        .post_json(
            "/v1/orders",
            json!({ "address_text": address, "address_lat": -1.9499, "address_lng": 30.0622, "items": items }),
        )
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn line(store_product_id: Uuid, quantity: i32) -> Value {
    json!({ "store_product_id": store_product_id, "quantity": quantity })
}

/// One customer order at one open store, advanced to `preparing` — ready
/// for a handoff. Returns the group JSON and the store-order id.
async fn preparing_order(
    app: &common::TestApp,
    op: &Operator,
    customer: (&TestClient, &str),
) -> (Value, String) {
    let store = create_store(op, "Aline Remera").await;
    let rice = create_product(op, "Rice").await;
    let rice_sp = attach(op, store, rice, 5000).await;
    set_open(&app.pool, store, true).await;
    let group = checkout(
        customer.0,
        customer.1,
        "KG 7 Ave, Remera",
        json!([line(rice_sp, 1)]),
    )
    .await;
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();
    // accept → preparing (the handoff's legal entry state).
    for status in ["accepted", "preparing"] {
        let response = op
            .client
            .patch_json(
                &format!("/v1/merchant/store-orders/{order_id}"),
                json!({ "status": status }),
            )
            .bearer_auth(&op.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "advance to {status}");
    }
    (group, order_id)
}

/// The delivery id behind a store order.
async fn delivery_of(pool: &PgPool, store_order_id: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM commerce.deliveries WHERE store_order_id = $1")
        .bind(Uuid::parse_str(store_order_id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Handoff by rider number, advanced to `preparing` first by the caller's
/// order. Asserts 200 and returns the response.
async fn handoff(op: &Operator, order_id: &str, rider_number: i64) -> reqwest::Response {
    op.client
        .post_json(
            &format!("/v1/merchant/store-orders/{order_id}/handoff"),
            json!({ "rider_number": rider_number }),
        )
        .bearer_auth(&op.token)
        .send()
        .await
        .unwrap()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn handoff_assigns_by_rider_number_and_picks_up(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;

    // The number is the whole interface — the store types it, the server
    // resolves an ACTIVE rider.
    let response = handoff(&aline, &order_id, rider.rider.rider_number).await;
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "picked_up");

    // The delivery carries the assignment: rider, handoff time, and the
    // route-less ETA (locked ride-speed estimate; no polyline invented).
    let (assigned_rider, handoff_at, eta, polyline): (
        Uuid,
        Option<time::OffsetDateTime>,
        Option<time::OffsetDateTime>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT rider_id, handoff_at, eta_target, route_polyline \
         FROM commerce.deliveries WHERE store_order_id = $1",
    )
    .bind(Uuid::parse_str(&order_id).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(assigned_rider, rider.rider.id);
    assert!(handoff_at.is_some(), "handoff stamps the moment");
    assert!(eta.is_some(), "the ride-speed ETA is set without a key");
    assert!(polyline.is_none(), "no geometry is invented without a key");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn handoff_legality_reassignment_and_anti_probe(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let bruce = owner(&app, "bruce@example.com", "Bruce's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let jean = rider_session(&app, "Jean", "+250780000002").await;
    let eric = rider_session(&app, "Eric", "+250780000003").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;

    // A foreign merchant's handoff: 404, indistinguishable from missing.
    assert_eq!(
        handoff(&bruce, &order_id, jean.rider.rider_number)
            .await
            .status(),
        404
    );
    // Unknown number and a deactivated rider are the same answer.
    assert_eq!(handoff(&aline, &order_id, 999999).await.status(), 404);
    sqlx::query("UPDATE commerce.riders SET is_active = false WHERE id = $1")
        .bind(eric.rider.id)
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        handoff(&aline, &order_id, eric.rider.rider_number)
            .await
            .status(),
        404
    );
    // The state machine refuses the skip: a fresh order at `placed` is not
    // handable — the merchant must accept and prepare first.
    let fresh_store = create_store(&aline, "Aline Kimironko").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, fresh_store, rice, 5000).await;
    set_open(&app.pool, fresh_store, true).await;
    let second = checkout(
        &chantal.0,
        &chantal.1,
        "KG 7 Ave",
        json!([line(rice_sp, 1)]),
    )
    .await;
    let second_id = second["store_orders"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        handoff(&aline, &second_id, jean.rider.rider_number)
            .await
            .status(),
        400,
        "placed is not handable"
    );

    // Re-assignment while picked_up: handoff simply runs again.
    assert_eq!(
        handoff(&aline, &order_id, jean.rider.rider_number)
            .await
            .status(),
        200
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn location_push_breadcrumbs_respect_the_noise_rule(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;
    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200
    );
    let delivery_id = delivery_of(&app.pool, &order_id).await;
    let token = token_for(&app, rider.account.id, 3600);
    let push = |lat: f64, lng: f64| {
        TestClient::new(&app.address)
            .post_json(
                &format!("/v1/deliveries/{delivery_id}/location"),
                json!({ "lat": lat, "lng": lng }),
            )
            .bearer_auth(&token)
    };

    // First position ever: recorded.
    assert_eq!(push(-1.9620, 30.1290).send().await.unwrap().status(), 204);
    // A GPS echo seconds later: throttled — still 204, no new breadcrumb.
    assert_eq!(push(-1.96201, 30.12901).send().await.unwrap().status(), 204);
    // A real move (~6km): recorded.
    assert_eq!(push(-1.9499, 30.0622).send().await.unwrap().status(), 204);

    let (count, last_lat, last_lng): (i64, f64, f64) = sqlx::query_as(
        "SELECT \
            (SELECT COUNT(*) FROM commerce.delivery_locations WHERE delivery_id = $1), \
            (SELECT last_lat FROM commerce.deliveries WHERE id = $1), \
            (SELECT last_lng FROM commerce.deliveries WHERE id = $1)",
    )
    .bind(delivery_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count, 2, "the echo was throttled, the move recorded");
    assert_eq!(last_lat, -1.9499);
    assert_eq!(last_lng, 30.0622);

    // Ownership: a foreign rider is a 404.
    let foreign = rider_session(&app, "Eric", "+250780000003").await;
    assert_eq!(
        TestClient::new(&app.address)
            .post_json(
                &format!("/v1/deliveries/{delivery_id}/location"),
                json!({ "lat": -1.9499, "lng": 30.0622 }),
            )
            .bearer_auth(token_for(&app, foreign.account.id, 3600))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    // Delivered ends the run: pushing to a finished delivery is a 409.
    assert_eq!(
        TestClient::new(&app.address)
            .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        push(-1.9499, 30.0623).send().await.unwrap().status(),
        409,
        "no positions after the delivery is done"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn rider_delivered_settles_the_cash_state(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;
    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200
    );
    let delivery_id = delivery_of(&app.pool, &order_id).await;
    let token = token_for(&app, rider.account.id, 3600);

    // Delivered: the order advances and the cash follows — the delivery's
    // allocation settles, and the single-store group's payment collects.
    let response = TestClient::new(&app.address)
        .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "delivered");

    let (allocation, payment): (String, String) = sqlx::query_as(
        "SELECT a.status::text, p.status::text \
         FROM commerce.payment_allocations a \
         JOIN commerce.payments p ON p.id = a.payment_id \
         WHERE a.store_order_id = $1",
    )
    .bind(Uuid::parse_str(&order_id).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(allocation, "settled");
    assert_eq!(payment, "collected");

    // The machine refuses a second delivery — the cash does not move twice.
    assert_eq!(
        TestClient::new(&app.address)
            .post(&format!("/v1/deliveries/{delivery_id}/delivered"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_multi_store_group_collects_only_when_all_are_settled(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let simba = owner(&app, "simba@example.com", "Simba's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let jean = rider_session(&app, "Jean", "+250780000002").await;

    // Two stores in one checkout → two deliveries, one payment.
    let aline_store = create_store(&aline, "Aline Remera").await;
    let rice = create_product(&aline, "Rice").await;
    let rice_sp = attach(&aline, aline_store, rice, 5000).await;
    set_open(&app.pool, aline_store, true).await;
    let simba_store = create_store(&simba, "Simba Town").await;
    let milk = create_product(&simba, "Milk").await;
    let milk_sp = attach(&simba, simba_store, milk, 1500).await;
    set_open(&app.pool, simba_store, true).await;

    let group = checkout(
        &chantal.0,
        &chantal.1,
        "KG 7 Ave",
        json!([line(rice_sp, 1), line(milk_sp, 1)]),
    )
    .await;
    let aline_order = group["store_orders"][0]["id"].as_str().unwrap().to_string();
    let simba_order = group["store_orders"][1]["id"].as_str().unwrap().to_string();

    for (op, order_id) in [(&aline, &aline_order), (&simba, &simba_order)] {
        for status in ["accepted", "preparing"] {
            assert_eq!(
                op.client
                    .patch_json(
                        &format!("/v1/merchant/store-orders/{order_id}"),
                        json!({ "status": status }),
                    )
                    .bearer_auth(&op.token)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
        }
        assert_eq!(
            handoff(op, order_id, jean.rider.rider_number)
                .await
                .status(),
            200
        );
    }
    let token = token_for(&app, jean.account.id, 3600);

    // First delivery settles ITS allocation; the group payment stays open.
    let aline_delivery = delivery_of(&app.pool, &aline_order).await;
    assert_eq!(
        TestClient::new(&app.address)
            .post(&format!("/v1/deliveries/{aline_delivery}/delivered"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let payment: String = sqlx::query_scalar(
        "SELECT p.status::text FROM commerce.payments p WHERE p.order_group_id = $1",
    )
    .bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        payment, "pending",
        "one settled allocation is not collected"
    );

    // The second delivery completes the cash: the payment collects.
    let simba_delivery = delivery_of(&app.pool, &simba_order).await;
    assert_eq!(
        TestClient::new(&app.address)
            .post(&format!("/v1/deliveries/{simba_delivery}/delivered"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let payment: String = sqlx::query_scalar(
        "SELECT p.status::text FROM commerce.payments p WHERE p.order_group_id = $1",
    )
    .bind(Uuid::parse_str(group["id"].as_str().unwrap()).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(payment, "collected");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_merchants_advance_settles_the_same_way(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;
    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200
    );

    // The merchant advances picked_up → delivered — the same event to the
    // ledger as the rider's action (doc §8: both are real actors).
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{order_id}"),
            json!({ "status": "delivered" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let (allocation, payment): (String, String) = sqlx::query_as(
        "SELECT a.status::text, p.status::text \
         FROM commerce.payment_allocations a \
         JOIN commerce.payments p ON p.id = a.payment_id \
         WHERE a.store_order_id = $1",
    )
    .bind(Uuid::parse_str(&order_id).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(allocation, "settled", "merchant delivery settles too");
    assert_eq!(payment, "collected");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn tracking_answers_204_when_nothing_changed(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;
    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200
    );
    let delivery_id = delivery_of(&app.pool, &order_id).await;
    let group_id = group["id"].as_str().unwrap();

    // First poll: 200 with the per-delivery snapshot.
    let response = chantal
        .0
        .get(&format!("/v1/orders/{group_id}/tracking"))
        .bearer_auth(&chantal.1)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["group_id"], group_id);
    assert_eq!(body["deliveries"][0]["store_name"], "Aline Remera");
    assert_eq!(body["deliveries"][0]["status"], "picked_up");
    let changed_at = body["changed_at"].as_str().unwrap().to_string();

    // The same changed_at echoed back: nothing moved — 204. A `+` in the
    // RFC-3339 string must survive as a plus, not a space.
    let since = changed_at.replace('+', "%2B");
    let poll = |since: &str| {
        chantal
            .0
            .get(&format!("/v1/orders/{group_id}/tracking?since={since}"))
            .bearer_auth(&chantal.1)
            .send()
    };
    assert_eq!(poll(&since).await.unwrap().status(), 204);

    // The rider moves — the next poll sees the change: 200 with the new
    // position and the recorded trail point.
    let token = token_for(&app, rider.account.id, 3600);
    assert_eq!(
        TestClient::new(&app.address)
            .post_json(
                &format!("/v1/deliveries/{delivery_id}/location"),
                json!({ "lat": -1.9499, "lng": 30.0622 }),
            )
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let response = poll(&since).await.unwrap();
    assert_eq!(response.status(), 200, "the rider moved — 200, not 204");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["deliveries"][0]["last_lat"], -1.9499);
    assert_eq!(body["deliveries"][0]["trail"].as_array().unwrap().len(), 1);

    // Anti-probe: another customer's tracking is a 404; a rider asking is
    // a 403.
    let (bruce, bruce_token) = customer_session(&app, "+250780000020").await;
    assert_eq!(
        bruce
            .get(&format!("/v1/orders/{group_id}/tracking"))
            .bearer_auth(&bruce_token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        TestClient::new(&app.address)
            .get(&format!("/v1/orders/{group_id}/tracking"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}

/// The stray rule (tracking doc §4): a position far off the cached road
/// route re-routes — and when no Directions backend answers, the still
/// valid route is preserved while the ETA re-arms from the new position.
/// An on-route position changes nothing.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_stray_position_re_arms_the_eta_and_keeps_the_route(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;
    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200
    );
    let delivery_id = delivery_of(&app.pool, &order_id).await;

    // Seed the cached road route a Directions key would have produced at
    // handoff: a corridor ending at the checkout destination.
    let polyline = routing::encode_polyline(&[
        routing::Coord {
            lat: -1.9550,
            lng: 30.0700,
        },
        routing::Coord {
            lat: -1.9499,
            lng: 30.0622,
        },
    ]);
    sqlx::query("UPDATE commerce.deliveries SET route_polyline = $1 WHERE id = $2")
        .bind(&polyline)
        .bind(delivery_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (eta_before,): (Option<time::OffsetDateTime>,) =
        sqlx::query_as("SELECT eta_target FROM commerce.deliveries WHERE id = $1")
            .bind(delivery_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(eta_before.is_some());

    let token = token_for(&app, rider.account.id, 3600);
    let push = |lat: f64, lng: f64| {
        TestClient::new(&app.address)
            .post_json(
                &format!("/v1/deliveries/{delivery_id}/location"),
                json!({ "lat": lat, "lng": lng }),
            )
            .bearer_auth(&token)
    };

    // On the route (~90m from the corridor's end): recorded, and the plan
    // — route and ETA — is untouched.
    assert_eq!(push(-1.9500, 30.0630).send().await.unwrap().status(), 204);
    let (polyline_after, eta_after): (Option<String>, Option<time::OffsetDateTime>) =
        sqlx::query_as("SELECT route_polyline, eta_target FROM commerce.deliveries WHERE id = $1")
            .bind(delivery_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(polyline_after.as_deref(), Some(polyline.as_str()));
    assert_eq!(eta_after, eta_before);

    // ~6.5km off the corridor: strayed. The re-route has no backend (no
    // key in tests), so the ETA re-arms from the new position and the
    // still-valid route is NOT overwritten with nothing.
    assert_eq!(push(-1.9620, 30.1290).send().await.unwrap().status(), 204);
    let (polyline_after, eta_after, last_lat, last_lng): (
        Option<String>,
        Option<time::OffsetDateTime>,
        f64,
        f64,
    ) = sqlx::query_as(
        "SELECT route_polyline, eta_target, last_lat, last_lng \
         FROM commerce.deliveries WHERE id = $1",
    )
    .bind(delivery_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        polyline_after.as_deref(),
        Some(polyline.as_str()),
        "a backend outage must not erase a still-valid route"
    );
    assert!(eta_after.is_some());
    assert!(
        eta_after > eta_before,
        "the ETA re-armed from the new position"
    );
    assert_eq!(last_lat, -1.9620);
    assert_eq!(last_lng, 30.1290);

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM commerce.delivery_locations WHERE delivery_id = $1")
            .bind(delivery_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "strayed positions are real positions — recorded");
}

/// `picked_up` is the handoff's event, never a bare merchant advance: the
/// PATCH that skips the rider number would strand the delivery with no
/// one able to push it. The handoff endpoint stays the only door in.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_bare_advance_cannot_skip_the_rider_handoff(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@example.com", "Aline's").await;
    let chantal = customer_session(&app, "+250780000012").await;
    let rider = rider_session(&app, "Jean", "+250780000002").await;

    let (_group, order_id) = preparing_order(&app, &aline, (&chantal.0, &chantal.1)).await;

    // The lie the old board told: "Handed to rider" without a rider.
    let response = aline
        .client
        .patch_json(
            &format!("/v1/merchant/store-orders/{order_id}"),
            json!({ "status": "picked_up" }),
        )
        .bearer_auth(&aline.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400, "no rider number, no pickup");
    let body: Value = response.json().await.unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("rider number"),
        "the error names the remedy: {body}"
    );

    // The state is untouched — the handoff still owns the door.
    let (status,): (String,) =
        sqlx::query_as("SELECT status::text FROM commerce.store_orders WHERE id = $1")
            .bind(Uuid::parse_str(&order_id).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(status, "preparing");

    assert_eq!(
        handoff(&aline, &order_id, rider.rider.rider_number)
            .await
            .status(),
        200,
        "the handoff with a rider number still works"
    );
}
