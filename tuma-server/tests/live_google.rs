//! LIVE-FIRE verification against the REAL Google APIs. Ignored by
//! default (the hermetic suites never touch the network); run explicitly
//! when you want proof the integrations work:
//!
//!   cargo test --test live_google -- --ignored --nocapture
//!
//! Every test fails loudly if a key is missing or restricted, and prints
//! derived data (never the keys). The Maps SDK keys (Android
//! `MAPS_API_KEY`, web `VITE_GOOGLE_MAPS_API_KEY`) are client-renderer
//! keys — they are verified on-device (maps render), not from this
//! server-side harness.

mod common;

use common::{MIGRATOR, TestApp, TestClient};
use geocoding::GeocodingProvider;
use routing::RoutingProvider;
use secrecy::ExposeSecret;
use tuma_server::config::Config;

use serde_json::json;
use sqlx::PgPool;

/// Real Kigali pins, far enough apart that Google must answer with
/// different places (UTC / Kigali Heights ↔ Kimironko).
const PIN_UTC: (f64, f64) = (-1.9441, 30.0619);
const PIN_KIMIRONKO: (f64, f64) = (-1.9367, 30.0927);

/// The REAL config — the same path the server boots with. `.env` is
/// loaded by `get_configuration` (dotenvy inside app-config), so the
/// keys below are exactly what production would use.
fn live_config() -> Config {
    let mut config = tuma_server::config::get_configuration().expect("config loads");
    // Force the real backends (the keys must exist — panics name the env
    // var if not).
    config.routing.backend = app_config::RoutingBackend::Google;
    config.geocoding.backend = app_config::GeocodingBackend::Google;

    let routing_key = config
        .routing
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().trim().to_string())
        .unwrap_or_default();
    if routing_key.is_empty() {
        panic!(
            "APP_ROUTING__API_KEY is missing — enable the Directions API and set it in tuma-server/.env"
        );
    }
    let geo_key = config
        .geocoding
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().trim().to_string())
        .unwrap_or_default();
    if geo_key.is_empty() {
        panic!(
            "APP_GEOCODING__API_KEY is missing — enable the Geocoding API and set it in tuma-server/.env"
        );
    }
    config
}

#[ignore = "live-fire: hits the real Google APIs — run with --ignored"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn live_geocoding_names_a_real_kigali_pin(_pool: PgPool) {
    let config = live_config();
    let key = config
        .geocoding
        .api_key
        .unwrap()
        .expose_secret()
        .to_string();
    let geocoder = geocoding::GoogleGeocoding::new(&key, config.geocoding.region.clone());

    let name = geocoder
        .reverse(geocoding::Coord {
            lat: PIN_UTC.0,
            lng: PIN_UTC.1,
        })
        .await
        .expect("geocoding request failed — is the Geocoding API enabled for this key?")
        .expect("Google returned no place for a real Kigali pin");
    println!("GEOCODE UTC        → {name}");
    assert!(!name.trim().is_empty());

    let other = geocoder
        .reverse(geocoding::Coord {
            lat: PIN_KIMIRONKO.0,
            lng: PIN_KIMIRONKO.1,
        })
        .await
        .expect("geocoding request failed")
        .expect("Google returned no place for Kimironko");
    println!("GEOCODE KIMIRONKO  → {other}");
    assert_ne!(name, other, "two distinct pins must derive distinct names");
}

#[ignore = "live-fire: hits the real Google APIs — run with --ignored"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn live_directions_routes_a_real_kigali_trip(_pool: PgPool) {
    let config = live_config();
    let key = config.routing.api_key.unwrap().expose_secret().to_string();
    let router = routing::GoogleRouting::new(&key, config.routing.region.clone());

    let route = router
        .route(
            routing::Coord {
                lat: PIN_UTC.0,
                lng: PIN_UTC.1,
            },
            routing::Coord {
                lat: PIN_KIMIRONKO.0,
                lng: PIN_KIMIRONKO.1,
            },
        )
        .await
        .expect("directions request failed — is the Directions API enabled for this key?")
        .expect("Google returned no road route between two Kigali pins");

    let points = routing::decode_polyline(&route.polyline);
    println!(
        "DIRECTIONS UTC → KIMIRONKO: {} polyline points, ETA {}s (~{} min)",
        points.len(),
        route.duration_secs,
        route.duration_secs / 60
    );
    assert!(points.len() >= 2, "a road route is more than one point");
    assert!(route.duration_secs > 0);
}

/// Diagnostic: which key serves which API — the answer to "is the
/// routing key the same as the geo key?" without printing any secret.
#[ignore = "live-fire: hits the real Google APIs — run with --ignored"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn key_matrix_serves_the_intended_apis(_pool: PgPool) {
    let config = live_config();
    let geo_key = config
        .geocoding
        .api_key
        .unwrap()
        .expose_secret()
        .to_string();
    let routing_key = config.routing.api_key.unwrap().expose_secret().to_string();

    let geo_with_geo = geocoding::GoogleGeocoding::new(&geo_key, None)
        .reverse(geocoding::Coord {
            lat: PIN_UTC.0,
            lng: PIN_UTC.1,
        })
        .await;
    println!(
        "GEOCODING key  → Geocoding API: {}",
        match &geo_with_geo {
            Ok(Some(name)) => format!("OK ({name})"),
            Ok(None) => "OK but no result".into(),
            Err(e) => format!("FAILED: {e}"),
        }
    );
    assert!(
        matches!(geo_with_geo, Ok(Some(_))),
        "the geocoding key must serve the Geocoding API"
    );

    let routing_with_routing = routing::GoogleRouting::new(&routing_key, None)
        .route(
            routing::Coord {
                lat: PIN_UTC.0,
                lng: PIN_UTC.1,
            },
            routing::Coord {
                lat: PIN_KIMIRONKO.0,
                lng: PIN_KIMIRONKO.1,
            },
        )
        .await;
    println!(
        "ROUTING key    → Directions API: {}",
        match &routing_with_routing {
            Ok(Some(route)) => format!("OK ({}s)", route.duration_secs),
            Ok(None) => "OK but no route".into(),
            Err(e) => format!("FAILED: {e}"),
        }
    );
    assert!(
        matches!(routing_with_routing, Ok(Some(_))),
        "the routing key must serve the Directions API"
    );
}

// --- end-to-end: the real server, real backends, real derivation -------

async fn spawn_app_live(config: Config, pool: PgPool) -> TestApp {
    let listener = tokio::net::TcpListener::bind(format!("{}:0", config.application.host))
        .await
        .expect("bind");
    let address = format!(
        "{}:{}",
        config.application.host,
        listener.local_addr().unwrap().port()
    );

    let state = tuma_server::app::AppState::new(
        pool.clone(),
        std::sync::Arc::new(accounts::AccountManager::new(4)),
        config.secret.jwt_signing_key.clone(),
        config.auth.dev_otp_code.clone(),
        config.application.cookie_secure,
        storage::build_service(&config.storage).expect("storage"),
        routing::build_service(&config.routing).expect("routing"),
        geocoding::build_service(&config.geocoding).expect("geocoding"),
        config.rate_limit.clone(),
    );
    let app = tuma_server::app::build_app_with_state(state);
    std::mem::forget(tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    }));
    TestApp {
        pool,
        address,
        config,
    }
}

#[ignore = "live-fire: hits the real Google APIs — run with --ignored"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn e2e_address_save_derives_the_name_from_google(pool: PgPool) {
    let app = spawn_app_live(live_config(), pool).await;
    let client = TestClient::new(&app.address);
    let seeded = common::seed_customer(&app.pool, "+250780000100").await;
    let token = common::token_for(&app, seeded.account.id, 3600);

    let save = |lat: f64, lng: f64| {
        client
            .post_json(
                "/v1/addresses",
                json!({"label": "Home", "lat": lat, "lng": lng}),
            )
            .bearer_auth(&token)
    };

    let response = save(PIN_UTC.0, PIN_UTC.1).send().await.unwrap();
    assert_eq!(response.status(), 201, "the save must land");
    let first: serde_json::Value = response.json().await.unwrap();
    let text = first["address_text"].as_str().expect("derived text");
    println!("E2E SAVE UTC        → {text}");
    assert!(!text.trim().is_empty());
    assert!(
        !geocoding::is_coordinate_pair(text),
        "a name, never coordinates"
    );

    let response = save(PIN_KIMIRONKO.0, PIN_KIMIRONKO.1).send().await.unwrap();
    assert_eq!(response.status(), 201);
    let second: serde_json::Value = response.json().await.unwrap();
    let other = second["address_text"].as_str().expect("derived text");
    println!("E2E SAVE KIMIRONKO  → {other}");
    assert_ne!(text, other, "distinct pins derive distinct names");

    // The cache holds both places — repeat saves never re-bill.
    let rows: i64 = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "count!" FROM geocoding.cache"#)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(rows, 2, "one cache row per distinct place");
}

#[ignore = "live-fire: hits the real Google APIs — run with --ignored"]
#[sqlx::test(migrator = "MIGRATOR")]
async fn e2e_handoff_caches_a_real_road_route(pool: PgPool) {
    let app = spawn_app_live(live_config(), pool).await;
    let client = TestClient::new(&app.address);

    // The merchant: store at UTC, product attached, store open.
    let seeded = common::seed_merchant(&app.pool, "aline@example.com", "Aline's").await;
    let merchant_token = common::token_for(&app, seeded.account.id, 3600);

    let response = client
        .post_json(
            "/v1/merchant/stores",
            json!({"name": "Aline UTC", "lat": PIN_UTC.0, "lng": PIN_UTC.1}),
        )
        .bearer_auth(&merchant_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "the store save must land");
    let store: serde_json::Value = response.json().await.unwrap();
    let store_id = store["id"].as_str().unwrap().to_string();
    // The store's address_text is Google-derived now — show it.
    println!("E2E STORE SAVE      → {}", store["address_text"]);
    assert!(
        store["address_text"]
            .as_str()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false),
        "a store with a pin derives a display name"
    );

    let response = client
        .post_json(
            "/v1/merchant/products",
            json!({"name": "Rice 5KG", "store_id": store_id, "description": "grain"}),
        )
        .bearer_auth(&merchant_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let product: serde_json::Value = response.json().await.unwrap();

    let response = client
        .post_json(
            "/v1/merchant/store-products",
            json!({"product_id": product["id"], "store_id": store_id, "price": 5000}),
        )
        .bearer_auth(&merchant_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let sp_id = response.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let response = client
        .patch_json(
            &format!("/v1/merchant/stores/{store_id}"),
            json!({"is_open": true}),
        )
        .bearer_auth(&merchant_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // The customer: address at Kimironko (Google-derived on save).
    let (derived_name, customer_token) = {
        let seeded = common::seed_customer(&app.pool, "+250780000101").await;
        let token = common::token_for(&app, seeded.account.id, 3600);
        let response = client
            .post_json(
                "/v1/addresses",
                json!({"label": "Home", "lat": PIN_KIMIRONKO.0, "lng": PIN_KIMIRONKO.1}),
            )
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let saved: serde_json::Value = response.json().await.unwrap();
        let derived = saved["address_text"]
            .as_str()
            .expect("derived text")
            .to_string();
        println!("E2E CUSTOMER SAVE   → {derived}");
        (derived, token)
    };

    // Checkout with the pinned address (the group carries the pin). The
    // client's text is deliberately garbage — the server derives from the
    // pin, and the same pin hits the same cache entry as the save above.
    let response = client
        .post_json(
            "/v1/orders",
            json!({
                "address_text": "garbage the client must not be able to inject",
                "address_lat": PIN_KIMIRONKO.0,
                "address_lng": PIN_KIMIRONKO.1,
                "items": [{"store_product_id": sp_id, "quantity": 1}],
            }),
        )
        .bearer_auth(&customer_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "checkout must land");
    let group: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        group["address_text"], derived_name,
        "the snapshot is the server-derived place name — client text is ignored"
    );
    let order_id = group["store_orders"][0]["id"].as_str().unwrap().to_string();

    // The walk: accepted → preparing → handoff (the rider gate fires a
    // REAL Directions call) → delivered settles the cash.
    let rider = common::seed_rider(&app.pool, "Jean", "+250780000102").await;
    for step in [
        (
            "patch",
            "/v1/merchant/store-orders/{id}",
            json!({ "status": "accepted" }),
        ),
        (
            "patch",
            "/v1/merchant/store-orders/{id}",
            json!({ "status": "preparing" }),
        ),
        (
            "post",
            "/v1/merchant/store-orders/{id}/handoff",
            json!({ "rider_number": rider.rider.rider_number }),
        ),
        (
            "patch",
            "/v1/merchant/store-orders/{id}",
            json!({ "status": "delivered" }),
        ),
    ] {
        let path = step.1.replace("{id}", &order_id);
        let response = match step.0 {
            "patch" => client.patch_json(&path, step.2),
            _ => client.post_json(&path, step.2),
        }
        .bearer_auth(&merchant_token)
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 200, "the walk must be legal: {path}");
    }

    // The proof: the delivery now carries a REAL road route + ETA.
    let (polyline, eta): (Option<String>, Option<time::OffsetDateTime>) = sqlx::query_as(
        "SELECT route_polyline, eta_target FROM commerce.deliveries WHERE store_order_id = $1",
    )
    .bind(&order_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let polyline = polyline.expect("the handoff must cache a Google road route");
    let points = routing::decode_polyline(&polyline);
    let eta = eta.expect("the handoff must cache a Google ETA");
    println!(
        "E2E HANDOFF         → {} polyline points, ETA {} min — REAL Directions data on the delivery",
        points.len(),
        (eta - time::OffsetDateTime::now_utc())
            .whole_minutes()
            .abs(),
    );
    assert!(points.len() >= 2, "a road route is more than one point");

    // And the customer's tracking poll reads it back.
    let tracking: serde_json::Value = client
        .get(&format!(
            "/v1/orders/{}/tracking",
            group["id"].as_str().unwrap()
        ))
        .bearer_auth(&customer_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cached = tracking["deliveries"]
        .as_array()
        .and_then(|d| d.first())
        .and_then(|d| d["route_polyline"].as_str())
        .expect("the tracking payload carries the cached road route");
    assert_eq!(cached, polyline, "tracking replays the cached route");
}
