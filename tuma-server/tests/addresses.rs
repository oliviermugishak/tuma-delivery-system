mod common;

use common::{MIGRATOR, TestClient, seed_customer, spawn_app, token_for};
use serde_json::json;

// ---------------------------------------------------------------------------
// The saved-address book: create → default flip → list → patch → delete.
// The location screen's whole surface, over the wire.
// ---------------------------------------------------------------------------

#[sqlx::test(migrator = "MIGRATOR")]
async fn addresses_require_a_customer(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = client.get("/v1/addresses").send().await.unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn first_address_becomes_default_and_kind_note_travel(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000004").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .post_json(
            "/v1/addresses",
            json!({
                "label": "Home",
                "address_text": "KK 40 Street, Kigali",
                "lat": -1.9449,
                "lng": 30.0619,
                "kind": "home",
                "note": "Gate on the left side"
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    assert_eq!(created["is_default"], json!(true));
    assert_eq!(created["kind"], json!("home"));
    assert_eq!(created["note"], json!("Gate on the left side"));
    assert_eq!(created["address_text"], json!("KK 40 Street, Kigali"));

    // A second address, explicitly marked default, demotes the first.
    let response = client
        .post_json(
            "/v1/addresses",
            json!({
                "label": "Work",
                "address_text": "KN 4 Ave, Kigali",
                "lat": -1.9502,
                "lng": 30.0631,
                "is_default": true,
                "kind": "work"
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    // The list is default-first, then newest.
    let response = client
        .get("/v1/addresses")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let list: serde_json::Value = response.json().await.unwrap();
    let items = list.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["label"], json!("Work"));
    assert_eq!(items[0]["is_default"], json!(true));
    assert_eq!(items[1]["is_default"], json!(false));

    // A create without kind defaults to `other` (the server, not the
    // client, owns the fallback).
    let response = client
        .post_json(
            "/v1/addresses",
            json!({"label": "Old", "address_text": "Somewhere"}),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let third: serde_json::Value = response.json().await.unwrap();
    assert_eq!(third["kind"], json!("other"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn address_patch_updates_fields_and_empty_label_is_rejected(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000005").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .post_json(
            "/v1/addresses",
            json!({
                "label": "Home",
                "address_text": "KK 40 Street, Kigali",
                "lat": -1.9449,
                "lng": 30.0619,
                "kind": "home",
                "note": "Blue gate"
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = response.json().await.unwrap();
    let id = created["id"].as_str().unwrap();

    // Patch: address + kind change, coordinates too (the pin moved on the
    // map), and the note explicitly cleared with an empty string.
    let response = client
        .patch_json(
            &format!("/v1/addresses/{id}"),
            json!({
                "address_text": "KK 40 Street, Kicukiro",
                "kind": "other",
                "note": "",
                "lat": -1.9461,
                "lng": 30.0607
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["address_text"], json!("KK 40 Street, Kicukiro"));
    assert_eq!(updated["kind"], json!("other"));
    assert_eq!(updated["note"], serde_json::Value::Null);
    assert_eq!(updated["lat"], json!(-1.9461));
    assert_eq!(updated["label"], json!("Home")); // absent keeps

    // An empty label/address is a 400, not a silent blank row.
    let response = client
        .patch_json(&format!("/v1/addresses/{id}"), json!({"label": "  "}))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // Someone else's address is a 404 — the book is per-customer.
    let other = seed_customer(&app.pool, "+250780000006").await;
    let other_token = token_for(&app, other.account.id, 3600);
    let response = client
        .get("/v1/addresses")
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    let other_list: serde_json::Value = response.json().await.unwrap();
    assert_eq!(other_list.as_array().unwrap().len(), 0);

    // Delete removes exactly the owner's row.
    let response = client
        .delete(&format!("/v1/addresses/{id}"))
        .bearer_auth(&other_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    let response = client
        .delete(&format!("/v1/addresses/{id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
}

/// Review S32: an absent note field must not wipe the saved rider note —
/// provided-overwrites, absent-keeps. Removing a note is deliberate:
/// an empty string is the explicit clear (the wire cannot tell an absent
/// field from an explicit null).
#[sqlx::test(migrator = "MIGRATOR")]
async fn an_absent_note_survives_the_patch_but_empty_clears_it(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000010").await;
    let token = token_for(&app, seeded.account.id, 3600);

    let response = client
        .post_json(
            "/v1/addresses",
            json!({
                "label": "Home",
                "address_text": "KG 7 Ave, Kigali",
                "note": "Blue gate"
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let id = created["id"].as_str().unwrap();

    // A patch without a note keeps the rider's "blue gate" instruction.
    let response = client
        .patch_json(&format!("/v1/addresses/{id}"), json!({"label": "Home2"}))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["label"], json!("Home2"));
    assert_eq!(
        updated["note"],
        json!("Blue gate"),
        "an absent note must survive the patch"
    );

    // Clearing is explicit: an empty string.
    let response = client
        .patch_json(&format!("/v1/addresses/{id}"), json!({"note": ""}))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        updated["note"],
        serde_json::Value::Null,
        "an empty string is the deliberate clear"
    );
}

/// Review P11: saved addresses ride the same ±90/±180 rule as the stores/
/// search inputs — on create and on patch (the pin moved is no license to
/// leave the planet).
#[sqlx::test(migrator = "MIGRATOR")]
async fn address_coordinates_are_range_validated_on_create_and_patch(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000007").await;
    let token = token_for(&app, seeded.account.id, 3600);

    // Create with a latitude beyond the pole: 422.
    let response = client
        .post_json(
            "/v1/addresses",
            json!({
                "label": "Nowhere",
                "address_text": "KG 7 Ave, Kigali",
                "lat": -91.0,
                "lng": 30.0619
            }),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "lat beyond ±90 is refused");

    // A valid address, then a patch moving it beyond the antimeridian: 422.
    let response = client
        .post_json(
            "/v1/addresses",
            json!({"label": "Home", "address_text": "KK 40 Street, Kigali"}),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let created: serde_json::Value = response.json().await.unwrap();
    let id = created["id"].as_str().unwrap();

    let response = client
        .patch_json(&format!("/v1/addresses/{id}"), json!({"lng": -180.5}))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422, "lng beyond ±180 is refused");

    // The untouched boundaries are honest zero: exactly ±90/±180 pass.
    let response = client
        .patch_json(
            &format!("/v1/addresses/{id}"),
            json!({"lat": -90.0, "lng": 180.0}),
        )
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "the boundary itself is legal");
}

// ---------------------------------------------------------------------------
// The geocoding proxy: server-side key or an honest 502 — never a client
// key, never invented addresses.
// ---------------------------------------------------------------------------

#[sqlx::test(migrator = "MIGRATOR")]
async fn geo_proxy_requires_a_customer(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let response = client
        .get("/v1/geo/reverse?lat=-1.9449&lng=30.0619")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn geo_proxy_without_a_key_degrades_to_502(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);
    let seeded = seed_customer(&app.pool, "+250780000007").await;
    let token = token_for(&app, seeded.account.id, 3600);

    // No geocoding key is configured in the test app — the proxy says so
    // plainly (502) instead of guessing an address.
    let response = client
        .get("/v1/geo/reverse?lat=-1.9449&lng=30.0619")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 502);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], json!("bad_gateway"));

    let response = client
        .get("/v1/geo/search?q=Kicukiro")
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 502);
}
