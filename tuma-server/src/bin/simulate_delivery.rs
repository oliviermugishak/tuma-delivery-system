//! The dev simulator (tracking doc §9): walks a moto-speed route through
//! the REAL `POST /v1/deliveries/{id}/location` endpoint, so the founder
//! watches a delivery move before any rider exists and the production
//! path stays byte-identical. Simulation lives only in this bin — the
//! server never knows.
//!
//! The walk follows the cached road route when the handoff stored one
//! (decoded waypoint-by-waypoint); without a route it falls back to the
//! straight line. Moto speed is the locked 25 km/h; one point every 5
//! seconds, like the rider screen will push.
//!
//! Usage:
//!   cargo run --bin simulate_delivery                          # newest handoff
//!   cargo run --bin simulate_delivery --deliver                # ... and mark it delivered
//!   cargo run --bin simulate_delivery --rider 2                # a specific rider's newest run
//!   cargo run --bin simulate_delivery --order 10               # a specific order number
//!   cargo run --bin simulate_delivery --list                   # just show what's out there
//!   cargo run --bin simulate_delivery --rider 2 --order 10 --deliver

use anyhow::bail;
use sqlx::postgres::PgPoolOptions;
use tuma_server::config::get_configuration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut store_order: Option<uuid::Uuid> = None;
    let mut order_number: Option<i64> = None;
    let mut rider_number: Option<i64> = None;
    let mut deliver = false;
    let mut list = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--store-order" => {
                store_order = Some(args.next().expect("--store-order needs an id").parse()?);
            }
            "--order" => {
                order_number = Some(args.next().expect("--order needs a number").parse()?);
            }
            "--rider" => {
                rider_number = Some(args.next().expect("--rider needs a number").parse()?);
            }
            "--deliver" => deliver = true,
            "--list" => list = true,
            other => bail!(
                "unknown argument {other} (use --rider <n>, --order <n>, --store-order <id>, --deliver, --list)"
            ),
        }
    }

    let config = get_configuration().expect("Failed to get configuration");
    let base = format!("http://127.0.0.1:{}", config.application.port);

    let pool = PgPoolOptions::new()
        .connect_with(config.database_with_db())
        .await?;

    if list {
        let rows = sqlx::query_as::<_, (uuid::Uuid, i64, i64, String, Option<String>)>(
            r#"
            SELECT so.id, so.number, r.rider_number, s.name, d.route_polyline
            FROM commerce.deliveries d
            JOIN commerce.store_orders so ON so.id = d.store_order_id
            JOIN marketplace.stores s ON s.id = so.store_id
            JOIN commerce.riders r ON r.id = d.rider_id
            WHERE so.status = 'picked_up'
            ORDER BY d.handoff_at DESC
            "#,
        )
        .fetch_all(&pool)
        .await?;
        if rows.is_empty() {
            println!("nothing out for delivery — hand an order off first");
            return Ok(());
        }
        println!("out for delivery now:");
        for (id, number, rider, store, polyline) in rows {
            println!(
                "  order #{number} · rider #{rider} · {store} · route: {} · store_order_id {id}",
                if polyline.is_some() { "cached" } else { "none" },
            );
        }
        return Ok(());
    }

    // The run to simulate: the newest handoff by default, narrowed by
    // --rider (a specific rider's run) and/or --order (a specific order
    // number). A cached route_polyline means the walk follows the real
    // road (decoded below) instead of the straight line. The delivery
    // stores the rider PROFILE id — the phone lives one join away,
    // through commerce.riders to accounts.users.
    let run = sqlx::query_as::<_, (uuid::Uuid, String, f64, f64, Option<String>)>(
        r#"
        SELECT d.id, u.phone, s.lat, s.lng, d.route_polyline
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        JOIN marketplace.stores s ON s.id = so.store_id
        JOIN commerce.riders r ON r.id = d.rider_id
        JOIN accounts.users u ON u.id = r.account_id
        WHERE so.status = 'picked_up'
          AND d.rider_id IS NOT NULL
          AND s.lat IS NOT NULL AND s.lng IS NOT NULL
          AND og.address_lat IS NOT NULL AND og.address_lng IS NOT NULL
          AND ($1::uuid IS NULL OR so.id = $1)
          AND ($2::bigint IS NULL OR so.number = $2)
          AND ($3::bigint IS NULL OR r.rider_number = $3)
        ORDER BY d.handoff_at DESC
        LIMIT 1
        "#,
    )
    .bind(store_order)
    .bind(order_number)
    .bind(rider_number)
    .fetch_optional(&pool)
    .await?;
    let Some((delivery_id, rider_phone, from_lat, from_lng, cached_polyline)) = run else {
        bail!(
            "no delivery to simulate: hand one off first (merchant endpoint, rider assigned, \
             store + destination coordinates present)"
        );
    };

    let (to_lat, to_lng, customer): (f64, f64, Option<String>) = sqlx::query_as(
        r#"
        SELECT og.address_lat, og.address_lng, c.name
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        LEFT JOIN accounts.customers c ON c.user_id = og.user_id
        WHERE d.id = $1
        "#,
    )
    .bind(delivery_id)
    .fetch_one(&pool)
    .await?;

    // The rider signs in exactly like the app does: phone + dev OTP code.
    // The token then rides the same endpoint the real rider screen uses.
    let client = reqwest::Client::new();
    let status = client
        .post(format!("{base}/api/v1/auth/otp/request"))
        .json(&serde_json::json!({ "phone": rider_phone }))
        .send()
        .await?
        .status();
    if status != 200 {
        bail!("otp request failed: {status}");
    }
    let dev_code = config
        .auth
        .dev_otp_code
        .clone()
        .ok_or_else(|| anyhow::anyhow!("auth.dev_otp_code must be configured to simulate"))?;
    let verify: serde_json::Value = client
        .post(format!("{base}/api/v1/auth/otp/verify"))
        .json(&serde_json::json!({ "phone": rider_phone, "code": dev_code }))
        .send()
        .await?
        .json()
        .await?;
    let token = verify["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("otp verify did not return a token"))?
        .to_string();

    // The path to walk: the cached road route decoded into waypoints when
    // the handoff stored one, else the straight line store → destination.
    // Either way the walk subdivides it at the locked moto speed (25 km/h)
    // into one point every 5 seconds — the exact push cadence the rider
    // screen will use. The server's rules (≥25m/15s breadcrumbs, strays)
    // govern the rest; the simulator just tells the truth.
    const MOTO_SPEED_KMH: f64 = 25.0;
    const STEP_SECS: u64 = 5;
    let meters_per_step = MOTO_SPEED_KMH * 1000.0 / 3600.0 * STEP_SECS as f64;
    let waypoints: Vec<(f64, f64)> = match cached_polyline.as_deref() {
        Some(encoded) if !encoded.is_empty() => {
            let decoded = routing::decode_polyline(encoded);
            if decoded.len() >= 2 {
                println!(
                    "walking the cached road route ({} waypoints)",
                    decoded.len()
                );
                decoded
                    .into_iter()
                    .map(|point| (point.lat, point.lng))
                    .collect()
            } else {
                println!("cached polyline undecodable — falling back to the straight line");
                vec![(from_lat, from_lng), (to_lat, to_lng)]
            }
        }
        _ => {
            println!("no cached route — walking the straight line (no Directions key)");
            vec![(from_lat, from_lng), (to_lat, to_lng)]
        }
    };

    // Subdivide the waypoint path into ~`meters_per_step` legs.
    let mut path: Vec<(f64, f64)> = vec![waypoints[0]];
    for window in waypoints.windows(2) {
        let (a, b) = (window[0], window[1]);
        let leg_m = commerce::haversine_m(a.0, a.1, b.0, b.1) as f64;
        let sub_steps = (leg_m / meters_per_step).ceil().max(1.0) as usize;
        for step in 1..=sub_steps {
            let t = step as f64 / sub_steps as f64;
            path.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    let leg_m = {
        let mut total = 0.0;
        for window in path.windows(2) {
            total +=
                commerce::haversine_m(window[0].0, window[0].1, window[1].0, window[1].1) as f64;
        }
        total
    };

    println!(
        "simulating delivery {delivery_id}: {} points over {leg_m:.0} m ≈ {} min{}",
        path.len(),
        (leg_m / (MOTO_SPEED_KMH * 1000.0 / 60.0)) as u64,
        if deliver {
            " — will mark delivered"
        } else {
            ""
        },
    );
    println!(
        "  customer: {} (call when you're close)",
        customer.as_deref().unwrap_or("unknown")
    );

    for (step, (lat, lng)) in path.iter().enumerate() {
        let response = client
            .post(format!("{base}/api/v1/deliveries/{delivery_id}/location"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "lat": lat, "lng": lng }))
            .send()
            .await?;
        if response.status() != 204 {
            bail!("location push failed at step {step}: {}", response.status());
        }
        println!("  [{:3}/{}] ({lat:.5}, {lng:.5})", step + 1, path.len());
        if step + 1 < path.len() {
            tokio::time::sleep(std::time::Duration::from_secs(STEP_SECS)).await;
        }
    }

    if deliver {
        let response = client
            .post(format!("{base}/api/v1/deliveries/{delivery_id}/delivered"))
            .bearer_auth(&token)
            .send()
            .await?;
        println!("delivered: {}", response.status());
    }

    println!("done — watch it on the customer's tracking screen.");
    Ok(())
}
