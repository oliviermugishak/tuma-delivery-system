//! Deliveries: the tracking half of commerce (build order #4, slice D2).
//! One delivery per store order (created empty at checkout); the rider
//! handoff assigns it, the location push breadcrumbs it, and the delivered
//! event settles the cash — exactly the tracking doc's rules:
//!
//! - Handoff = entering the rider number: `preparing → picked_up` (or a
//!   re-assignment while still `picked_up`; after `delivered` it's frozen).
//! - Breadcrumbs are insert-only, gated by the ≥25m/15s GPS-noise rule.
//! - The cash state: the moment of delivery IS the moment of payment —
//!   the delivery's payment allocation settles, and the group's payment
//!   becomes collected when every allocation is settled. No other
//!   collection action exists.

use crate::orders::{AllocationStatus, OrderStatus, PaymentStatus, StoreOrder};
use sqlx::{Acquire, PgConnection};
use time::OffsetDateTime;
use uuid::Uuid;

/// A breadcrumb is inserted only when the rider has moved at least this
/// far from the last recorded position — the GPS-noise killer. Founder-
/// tunable default (tracking doc §12).
pub const BREADCRUMB_MIN_DISTANCE_M: i64 = 25;

/// ... or at least this long has passed since the last breadcrumb.
pub const BREADCRUMB_MIN_INTERVAL_SECS: i64 = 15;

/// The rider strayed from the cached route when the pushed position sits
/// at least this far from every point on it — the trigger for a re-route
/// (tracking doc §4: "re-fetched only when the rider strays ≥~200m").
pub const STRAY_MIN_DISTANCE_M: i64 = 200;

/// How far back the tracking trail reaches — the drawn trail is a tail,
/// not the whole history.
const TRAIL_TAIL_LEN: usize = 50;

/// Decode Google's encoded polyline (the cached route's format) into its
/// points, in route order. A truncated string decodes only its complete
/// prefix. commerce cannot import the `routing` crate (no cross-domain
/// dependencies), so the well-specified algorithm travels with the rule
/// that needs it — same deliberate duplication as [`haversine_m`], same
/// round-trip test discipline.
fn decode_route_polyline(encoded: &str) -> Vec<(f64, f64)> {
    let bytes = encoded.as_bytes();
    let mut points = Vec::new();
    let mut index = 0usize;
    let (mut lat, mut lng): (i64, i64) = (0, 0);
    let component = |index: &mut usize| -> Option<i64> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            if *index >= bytes.len() {
                return None;
            }
            let byte = (bytes[*index].wrapping_sub(63)) as u64;
            *index += 1;
            result |= (byte & 0x1f) << shift;
            shift += 5;
            if byte & 0x20 == 0 {
                break;
            }
        }
        Some(if result & 1 == 1 {
            !((result >> 1) as i64)
        } else {
            (result >> 1) as i64
        })
    };
    while index < bytes.len() {
        let (Some(d_lat), Some(d_lng)) = (component(&mut index), component(&mut index)) else {
            break;
        };
        lat += d_lat;
        lng += d_lng;
        points.push((lat as f64 / 1e5, lng as f64 / 1e5));
    }
    points
}

/// Distance in whole meters from a pushed position to the nearest point on
/// the decoded route — the stray check's measure.
fn distance_to_route(lat: f64, lng: f64, route: &[(f64, f64)]) -> i64 {
    route
        .iter()
        .map(|(route_lat, route_lng)| haversine_m(lat, lng, *route_lat, *route_lng))
        .min()
        .unwrap_or(i64::MAX)
}

#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    #[error("delivery not found")]
    NotFound,
    #[error("the delivery is not out for delivery")]
    NotOutForDelivery,
    #[error("an order cannot move from {from} to {to}")]
    Illegal {
        from: &'static str,
        to: &'static str,
    },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A road route cached on the delivery at handoff. The polyline arrives
/// from the routing adapter (None until a Directions key exists — no
/// geometry is ever invented); the ETA target is always known because the
/// route-less fallback estimates it from the locked ride speed.
#[derive(Debug, Clone)]
pub struct CachedRoute {
    pub polyline: Option<String>,
    pub eta_target: OffsetDateTime,
}

/// Great-circle distance between two WGS84 points, in whole meters. commerce
/// cannot import `marketplace::geo` (no cross-domain crate dependencies), so
/// the function travels with the rule that needs it — same math as there,
/// same Kigali test.
pub fn haversine_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> i64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lng = (lng2 - lng1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let a =
        (d_lat / 2.0).sin().powi(2) + lat1_rad.cos() * lat2_rad.cos() * (d_lng / 2.0).sin().powi(2);
    (2.0 * EARTH_RADIUS_M * a.sqrt().atan()).round() as i64
}

/// Where a delivery rides from and to — the coordinates the routing
/// adapter (or the ride-speed fallback) needs at handoff. Partial
/// coordinates make the whole cache unusable: the caller checks for them.
pub struct RouteContext {
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
    pub destination_lat: Option<f64>,
    pub destination_lng: Option<f64>,
}

pub async fn route_context(
    conn: &mut PgConnection,
    store_order_id: Uuid,
) -> Result<Option<RouteContext>, sqlx::Error> {
    sqlx::query_as!(
        RouteContext,
        r#"
        SELECT s.lat AS store_lat, s.lng AS store_lng,
               og.address_lat AS destination_lat, og.address_lng AS destination_lng
        FROM commerce.store_orders so
        JOIN marketplace.stores s ON s.id = so.store_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        WHERE so.id = $1
        "#,
        store_order_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The same context, keyed by delivery id — the stray re-route knows the
/// delivery, not the store order.
pub async fn route_context_for_delivery(
    conn: &mut PgConnection,
    delivery_id: Uuid,
) -> Result<Option<RouteContext>, sqlx::Error> {
    sqlx::query_as!(
        RouteContext,
        r#"
        SELECT s.lat AS store_lat, s.lng AS store_lng,
               og.address_lat AS destination_lat, og.address_lng AS destination_lng
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        JOIN marketplace.stores s ON s.id = so.store_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        WHERE d.id = $1
        "#,
        delivery_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The merchant's handoff: assign the rider (validated active by the
/// caller) and hand the order over. Legal from `preparing` (the normal
/// advance to `picked_up`) and from `picked_up` (re-assignment — a wrong
/// rider number is re-run, not a support ticket). Everything else is the
/// state machine refusing the skip. One transaction: status move + the
/// delivery's assignment, route cache, and ETA target.
pub async fn handoff(
    conn: &mut PgConnection,
    store_order_id: Uuid,
    rider_id: Uuid,
    cached: Option<CachedRoute>,
) -> Result<StoreOrder, DeliveryError> {
    let mut tx = conn.begin().await?;
    let current = sqlx::query!(
        r#"
        SELECT status AS "status: OrderStatus" FROM commerce.store_orders
        WHERE id = $1
        "#,
        store_order_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DeliveryError::NotFound)?;

    match current.status {
        OrderStatus::Preparing => {
            // Compare-and-swap: the read above and this write are one tx,
            // but a concurrent customer cancel could still have flipped
            // the row — 0 rows means the handoff loses, not last-write-wins.
            let swapped = sqlx::query!(
                r#"
                UPDATE commerce.store_orders SET status = $2
                WHERE id = $1 AND status = $3
                "#,
                store_order_id,
                OrderStatus::PickedUp as OrderStatus,
                OrderStatus::Preparing as OrderStatus,
            )
            .execute(&mut *tx)
            .await?;
            if swapped.rows_affected() == 0 {
                return Err(DeliveryError::Illegal {
                    from: OrderStatus::Preparing.label(),
                    to: OrderStatus::PickedUp.label(),
                });
            }
        }
        OrderStatus::PickedUp => {}
        other => {
            return Err(DeliveryError::Illegal {
                from: other.label(),
                to: OrderStatus::PickedUp.label(),
            });
        }
    }

    let assigned = sqlx::query!(
        r#"
        UPDATE commerce.deliveries
        SET rider_id = $2, handoff_at = now(), route_polyline = $3, eta_target = $4
        WHERE store_order_id = $1
        "#,
        store_order_id,
        rider_id,
        cached.as_ref().and_then(|route| route.polyline.clone()),
        cached.as_ref().map(|route| route.eta_target),
    )
    .execute(&mut *tx)
    .await?;
    if assigned.rows_affected() == 0 {
        return Err(DeliveryError::NotFound);
    }

    let order = sqlx::query_as!(
        StoreOrder,
        r#"
        SELECT id, order_group_id, merchant_id, store_id, number,
               status AS "status: OrderStatus", subtotal, delivery_fee, total,
               created_at, updated_at
        FROM commerce.store_orders
        WHERE id = $1
        "#,
        store_order_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(order)
}

/// What one location push did: recorded a real move, was throttled, or
/// strayed far enough from the cached route to want a re-route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    Recorded,
    Skipped,
    Strayed,
}

/// The rider's phone checking in. Ownership is enforced here — another
/// rider's delivery is a plain 404 — and only a delivery that is actually
/// out for delivery accepts positions. The ≥25m/15s rule decides whether a
/// breadcrumb is inserted; the first position ever always records.
pub async fn push_location(
    conn: &mut PgConnection,
    delivery_id: Uuid,
    rider_id: Uuid,
    lat: f64,
    lng: f64,
) -> Result<PushOutcome, DeliveryError> {
    let mut tx = conn.begin().await?;
    let row = sqlx::query!(
        r#"
        SELECT d.last_lat, d.last_lng, d.last_location_at, d.route_polyline,
               so.status AS "status: OrderStatus"
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        WHERE d.id = $1 AND d.rider_id = $2
        "#,
        delivery_id,
        rider_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DeliveryError::NotFound)?;

    if row.status != OrderStatus::PickedUp {
        return Err(DeliveryError::NotOutForDelivery);
    }

    let now = OffsetDateTime::now_utc();
    let skip = match (row.last_lat, row.last_lng, row.last_location_at) {
        (Some(last_lat), Some(last_lng), Some(last_at)) => {
            haversine_m(last_lat, last_lng, lat, lng) < BREADCRUMB_MIN_DISTANCE_M
                && (now - last_at).whole_seconds() < BREADCRUMB_MIN_INTERVAL_SECS
        }
        _ => false,
    };
    if skip {
        return Ok(PushOutcome::Skipped);
    }

    // The stray check (tracking doc §4): far enough off the cached route
    // and the caller re-routes. Recorded AND strayed — the position is
    // real regardless of what it does to the plan.
    let strayed = row
        .route_polyline
        .as_deref()
        .map(|polyline| distance_to_route(lat, lng, &decode_route_polyline(polyline)))
        .is_some_and(|distance_m| distance_m >= STRAY_MIN_DISTANCE_M);

    sqlx::query!(
        r#"
        INSERT INTO commerce.delivery_locations (delivery_id, lat, lng)
        VALUES ($1, $2, $3)
        "#,
        delivery_id,
        lat,
        lng,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE commerce.deliveries
        SET last_lat = $2, last_lng = $3, last_location_at = $4
        WHERE id = $1
        "#,
        delivery_id,
        lat,
        lng,
        now,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(if strayed {
        PushOutcome::Strayed
    } else {
        PushOutcome::Recorded
    })
}

/// Persist a re-fetched route on a delivery that strayed. A `None`
/// polyline keeps the existing one (a backend outage must not erase a
/// still-valid route — only the ETA is re-armed); `Some` overwrites in
/// place, the doc's single-column model (the breadcrumb trail preserves
/// the past).
pub async fn update_delivery_route(
    conn: &mut PgConnection,
    delivery_id: Uuid,
    cached: CachedRoute,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE commerce.deliveries
        SET route_polyline = COALESCE($2, route_polyline), eta_target = $3
        WHERE id = $1
        "#,
        delivery_id,
        cached.polyline,
        cached.eta_target,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// The cash state (tracking doc §5): the delivery's allocation settles and
/// the group's payment becomes collected when nothing is pending anymore.
/// Called for whichever real actor marked the delivery — the rider's
/// Delivered action and the merchant's advance are the same event to the
/// ledger. Idempotent: an already-settled allocation stays settled and
/// moves nothing. Only the database can fail — the caller guarantees the
/// order exists.
pub async fn settle_delivery_cash(
    conn: &mut PgConnection,
    store_order_id: Uuid,
) -> Result<(), sqlx::Error> {
    let settled = sqlx::query!(
        r#"
        UPDATE commerce.payment_allocations
        SET status = $2
        WHERE store_order_id = $1 AND status = $3
        "#,
        store_order_id,
        AllocationStatus::Settled as AllocationStatus,
        AllocationStatus::Pending as AllocationStatus,
    )
    .execute(&mut *conn)
    .await?;
    if settled.rows_affected() == 0 {
        return Ok(());
    }
    sqlx::query!(
        r#"
        UPDATE commerce.payments p
        SET status = $2
        WHERE p.status = $3
          AND p.id = (
            SELECT a.payment_id FROM commerce.payment_allocations a
            WHERE a.store_order_id = $1
          )
          AND NOT EXISTS (
            SELECT 1 FROM commerce.payment_allocations a2
            WHERE a2.payment_id = p.id AND a2.status = $4
          )
        "#,
        store_order_id,
        PaymentStatus::Collected as PaymentStatus,
        PaymentStatus::Pending as PaymentStatus,
        AllocationStatus::Pending as AllocationStatus,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// The rider's Delivered action: food handed over + cash received, one
/// real event. Advances `picked_up → delivered` (the machine refuses
/// anything else) and settles the cash in the same transaction.
pub async fn mark_delivered(
    conn: &mut PgConnection,
    delivery_id: Uuid,
    rider_id: Uuid,
) -> Result<StoreOrder, DeliveryError> {
    let mut tx = conn.begin().await?;
    let row = sqlx::query!(
        r#"
        SELECT d.store_order_id, so.status AS "status: OrderStatus"
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        WHERE d.id = $1 AND d.rider_id = $2
        "#,
        delivery_id,
        rider_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(DeliveryError::NotFound)?;

    if !row.status.can_transition_to(OrderStatus::Delivered) {
        return Err(DeliveryError::Illegal {
            from: row.status.label(),
            to: OrderStatus::Delivered.label(),
        });
    }

    // Compare-and-swap: a concurrent cancel must not be overwritten to
    // delivered by the rider's confirm — 0 rows loses the race.
    let swapped = sqlx::query!(
        r#"
        UPDATE commerce.store_orders SET status = $2
        WHERE id = $1 AND status = $3
        "#,
        row.store_order_id,
        OrderStatus::Delivered as OrderStatus,
        OrderStatus::PickedUp as OrderStatus,
    )
    .execute(&mut *tx)
    .await?;
    if swapped.rows_affected() == 0 {
        return Err(DeliveryError::Illegal {
            from: row.status.label(),
            to: OrderStatus::Delivered.label(),
        });
    }
    settle_delivery_cash(&mut tx, row.store_order_id).await?;

    let order = sqlx::query_as!(
        StoreOrder,
        r#"
        SELECT id, order_group_id, merchant_id, store_id, number,
               status AS "status: OrderStatus", subtotal, delivery_fee, total,
               created_at, updated_at
        FROM commerce.store_orders
        WHERE id = $1
        "#,
        row.store_order_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(order)
}

/// One of the rider's active deliveries — the kiosk card's data source:
/// the store to collect from, the destination to ride to, and the customer
/// to call when close (the Kigali protocol — the data already flows).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActiveDelivery {
    pub delivery_id: Uuid,
    pub store_order_id: Uuid,
    pub number: i64,
    /// The cash this stop collects (P8: money at the point of action).
    pub total: i64,
    pub store_name: String,
    pub store_address: Option<String>,
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
    /// The store's phone — the Pick-up stage's Call-the-store action.
    pub store_contact_phone: Option<String>,
    pub destination_address: String,
    pub destination_lat: Option<f64>,
    pub destination_lng: Option<f64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    /// The checkout's rider note ("blue gate, ring the bell") — shown on
    /// the Delivering card; None renders as nothing (P2).
    pub customer_note: Option<String>,
    pub status: OrderStatus,
    pub handoff_at: Option<OffsetDateTime>,
    pub route_polyline: Option<String>,
    pub eta_target: Option<OffsetDateTime>,
    pub last_lat: Option<f64>,
    pub last_lng: Option<f64>,
    pub last_location_at: Option<OffsetDateTime>,
}

/// The rider's live work list: assigned, still out for delivery, newest
/// handoff first. Scoped to the caller's rider row — another rider's
/// deliveries simply do not exist here.
pub async fn active_deliveries_for_rider(
    conn: &mut PgConnection,
    rider_id: Uuid,
) -> Result<Vec<ActiveDelivery>, sqlx::Error> {
    sqlx::query_as!(
        ActiveDelivery,
        r#"
        SELECT d.id AS delivery_id, d.store_order_id,
               so.number, so.total,
               s.name AS store_name, s.address_text AS store_address,
               s.lat AS store_lat, s.lng AS store_lng,
               s.contact_phone AS store_contact_phone,
               og.address_text AS destination_address,
               og.address_lat AS destination_lat, og.address_lng AS destination_lng,
               c.name AS customer_name, u.phone AS customer_phone,
               og.customer_note,
               so.status AS "status: OrderStatus",
               d.handoff_at, d.route_polyline, d.eta_target,
               d.last_lat, d.last_lng, d.last_location_at
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        JOIN marketplace.stores s ON s.id = so.store_id
        LEFT JOIN accounts.customers c ON c.user_id = og.user_id
        LEFT JOIN accounts.users u ON u.id = og.user_id
        WHERE d.rider_id = $1 AND so.status = 'picked_up'
        ORDER BY d.handoff_at DESC NULLS LAST
        "#,
        rider_id,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One delivered stop in the rider's history — the run record the rider
/// app lists under today's tally. `delivered_at` is the store order's
/// last write: the moment the rider confirmed the handover.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeliveredDelivery {
    pub delivery_id: Uuid,
    pub store_order_id: Uuid,
    pub number: i64,
    pub total: i64,
    pub store_name: String,
    pub destination_address: String,
    pub customer_name: Option<String>,
    pub delivered_at: OffsetDateTime,
}

/// The rider's delivered history, newest first. Every delivery they
/// completed — not just today's — so the kiosk can show the whole run.
pub async fn delivered_history_for_rider(
    conn: &mut PgConnection,
    rider_id: Uuid,
    limit: i64,
) -> Result<Vec<DeliveredDelivery>, sqlx::Error> {
    sqlx::query_as!(
        DeliveredDelivery,
        r#"
        SELECT d.id AS delivery_id, d.store_order_id,
               so.number, so.total,
               s.name AS store_name,
               og.address_text AS destination_address,
               c.name AS customer_name,
               so.updated_at AS delivered_at
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        JOIN marketplace.stores s ON s.id = so.store_id
        LEFT JOIN accounts.customers c ON c.user_id = og.user_id
        WHERE d.rider_id = $1 AND so.status = 'delivered'
        ORDER BY so.updated_at DESC
        LIMIT $2
        "#,
        rider_id,
        limit,
    )
    .fetch_all(&mut *conn)
    .await
}

/// One delivery's tracking view — what the customer's map needs. `status`
/// is the store order's own six-value state; the positions and route are
/// the delivery's real data, or nothing.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DeliveryTracking {
    pub delivery_id: Uuid,
    pub store_order_id: Uuid,
    pub store_name: String,
    /// The assigned rider's identity for the tracking card — the first
    /// word of the rider's name plus their vehicle, nullable until a
    /// rider is assigned (P2: hidden until it exists).
    pub rider_name: Option<String>,
    pub rider_vehicle: Option<String>,
    pub rider_plate: Option<String>,
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
    pub store_contact_phone: Option<String>,
    pub status: OrderStatus,
    pub handoff_at: Option<OffsetDateTime>,
    pub route_polyline: Option<String>,
    pub eta_target: Option<OffsetDateTime>,
    pub last_lat: Option<f64>,
    pub last_lng: Option<f64>,
    pub last_location_at: Option<OffsetDateTime>,
    /// Straight-line meters from the rider's freshest fix to the
    /// destination — the customer's "your driver is close" line. Null
    /// until the rider's phone has checked in at least once.
    pub rider_distance_m: Option<i64>,
    pub updated_at: OffsetDateTime,
    /// The delivery row's own last write (P10). A stray re-route touches
    /// ONLY the delivery — re-armed ETA, maybe a new polyline — and the
    /// store order is silent about it; without this stamp in `changed_at`
    /// the customer's next poll would earn a 204 and the map would never
    /// learn the new route/ETA.
    pub delivery_updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TrailPoint {
    pub delivery_id: Uuid,
    pub lat: f64,
    pub lng: f64,
    pub recorded_at: OffsetDateTime,
}

/// The rider's day so far: deliveries marked delivered today (UTC) and
/// the cash that rode with them. The Waiting card's motivation line.
pub async fn rider_today_tally(
    conn: &mut PgConnection,
    rider_id: Uuid,
) -> Result<(i64, i64), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*) AS "deliveries!: i64",
               COALESCE(SUM(so.total), 0) AS "collected!: i64"
        FROM commerce.deliveries d
        JOIN commerce.store_orders so ON so.id = d.store_order_id
        WHERE d.rider_id = $1
          AND so.status = 'delivered'
          AND so.updated_at >= date_trunc('day', now())
        "#,
        rider_id,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((row.deliveries, row.collected))
}

/// The group's tracking snapshot — one entry per store-order delivery,
/// each its own story (the multi-store rule). `changed_at` is the freshest
/// write across the group: the client echoes it as `since` and a poll that
/// finds nothing newer earns a 204.
pub struct GroupTracking {
    pub group_id: Uuid,
    pub group_status: crate::orders::GroupStatus,
    pub payment_status: PaymentStatus,
    pub deliveries: Vec<DeliveryTracking>,
    pub trails: Vec<TrailPoint>,
    pub changed_at: OffsetDateTime,
}

/// The owned group's tracking data, or `None` when the group does not exist
/// or belongs to someone else — anti-probe: the two are indistinguishable.
pub async fn tracking_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
    group_id: Uuid,
) -> Result<Option<GroupTracking>, sqlx::Error> {
    let owned = sqlx::query!(
        r#"SELECT id FROM commerce.order_groups WHERE id = $1 AND user_id = $2"#,
        group_id,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(group) = owned else {
        return Ok(None);
    };

    let deliveries = sqlx::query_as!(
        DeliveryTracking,
        r#"
        SELECT d.id AS delivery_id, so.id AS store_order_id, s.name AS store_name,
               r.name AS rider_name, r.vehicle_type AS rider_vehicle,
               r.plate_number AS rider_plate,
               s.lat AS store_lat, s.lng AS store_lng,
               s.contact_phone AS store_contact_phone,
               so.status AS "status: OrderStatus",
               d.handoff_at, d.route_polyline, d.eta_target,
               d.last_lat, d.last_lng, d.last_location_at,
               CASE WHEN d.last_lat IS NOT NULL AND d.last_lng IS NOT NULL
                         AND og.address_lat IS NOT NULL AND og.address_lng IS NOT NULL
               THEN (6371000.0 * 2.0 * asin(sqrt(
                        power(sin(radians(og.address_lat - d.last_lat) / 2.0), 2)
                      + cos(radians(d.last_lat)) * cos(radians(og.address_lat))
                        * power(sin(radians(og.address_lng - d.last_lng) / 2.0), 2)
                    )))::bigint
               ELSE NULL END AS rider_distance_m,
               so.updated_at,
               d.updated_at AS delivery_updated_at
        FROM commerce.store_orders so
        JOIN commerce.deliveries d ON d.store_order_id = so.id
        JOIN commerce.order_groups og ON og.id = so.order_group_id
        JOIN marketplace.stores s ON s.id = so.store_id
        LEFT JOIN commerce.riders r ON r.id = d.rider_id
        WHERE so.order_group_id = $1
        ORDER BY so.created_at
        "#,
        group_id,
    )
    .fetch_all(&mut *conn)
    .await?;

    // The trail is a tail, not the history — SQL keeps exactly the last
    // TRAIL_TAIL_LEN real points per delivery (a lateral over the
    // (delivery_id, recorded_at) index, re-sorted oldest-first for
    // drawing). Fetching all breadcrumbs and trimming in Rust scanned a
    // whole run's rows on every poll.
    let delivery_ids: Vec<Uuid> = deliveries.iter().map(|d| d.delivery_id).collect();
    let trails: Vec<TrailPoint> = if delivery_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as!(
            TrailPoint,
            r#"
            SELECT t.delivery_id, t.lat, t.lng, t.recorded_at
            FROM unnest($1::uuid[]) AS d(id)
            CROSS JOIN LATERAL (
                SELECT dl.delivery_id, dl.lat, dl.lng, dl.recorded_at
                FROM commerce.delivery_locations dl
                WHERE dl.delivery_id = d.id
                ORDER BY dl.recorded_at DESC
                LIMIT $2
            ) t
            ORDER BY t.delivery_id, t.recorded_at ASC
            "#,
            &delivery_ids,
            TRAIL_TAIL_LEN as i64,
        )
        .fetch_all(&mut *conn)
        .await?
    };
    // unnest preserves the delivery_ids order (the deliveries' own order),
    // so each delivery's trail slice is already contiguous.

    let payment = crate::orders::payment_for_group(&mut *conn, group.id).await?;
    let statuses: Vec<OrderStatus> = deliveries.iter().map(|d| d.status).collect();
    let changed_at = deliveries
        .iter()
        .flat_map(|d| {
            [
                Some(d.updated_at),
                Some(d.delivery_updated_at),
                d.last_location_at,
                d.handoff_at,
            ]
        })
        .flatten()
        .max()
        .unwrap_or_else(OffsetDateTime::now_utc);

    Ok(Some(GroupTracking {
        group_id: group.id,
        group_status: crate::orders::derive_group_status(&statuses),
        payment_status: payment.map(|p| p.status).unwrap_or(PaymentStatus::Pending),
        deliveries,
        trails,
        changed_at,
    }))
}

/// The freshness probe behind the tracking poll's 204: the group's
/// `changed_at` with ONE cheap query instead of the full snapshot
/// (geometry + trails + payment). Equal-or-newer `since` → the caller
/// answers 204 without paying for any of that.
pub async fn tracking_changed_at(
    conn: &mut PgConnection,
    user_id: Uuid,
    group_id: Uuid,
) -> Result<Option<OffsetDateTime>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        SELECT max(t) AS "changed_at: _" FROM (
            SELECT so.updated_at AS t
            FROM commerce.order_groups og
            JOIN commerce.store_orders so ON so.order_group_id = og.id
            WHERE og.id = $1 AND og.user_id = $2
            UNION ALL
            SELECT d.last_location_at
            FROM commerce.order_groups og
            JOIN commerce.store_orders so ON so.order_group_id = og.id
            JOIN commerce.deliveries d ON d.store_order_id = so.id
            WHERE og.id = $1 AND og.user_id = $2
            UNION ALL
            SELECT d.handoff_at
            FROM commerce.order_groups og
            JOIN commerce.store_orders so ON so.order_group_id = og.id
            JOIN commerce.deliveries d ON d.store_order_id = so.id
            WHERE og.id = $1 AND og.user_id = $2
            UNION ALL
            SELECT d.updated_at
            FROM commerce.order_groups og
            JOIN commerce.store_orders so ON so.order_group_id = og.id
            JOIN commerce.deliveries d ON d.store_order_id = so.id
            WHERE og.id = $1 AND og.user_id = $2
        ) stamps
        "#,
        group_id,
        user_id,
    )
    .fetch_one(&mut *conn)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 25m breadcrumb rule's yardstick must be honest: map-verifiable
    /// Kigali coordinates, both directions agree, and the GPS-noise band
    /// (a few meters of jitter) sits under the threshold.
    #[test]
    fn haversine_measures_real_kigali_distances() {
        // UTC (City Center) to Remera KG 7 Ave: the great-circle distance
        // is ~7.5 km (the ~6.4 km road figure rides streets, not air).
        let meters = haversine_m(-1.9499, 30.0622, -1.9620, 30.1290);
        assert!(
            (7_000..=8_000).contains(&meters),
            "UTC→Remera great-circle should be ~7.5 km, got {meters}"
        );
        // Symmetric.
        assert_eq!(meters, haversine_m(-1.9620, 30.1290, -1.9499, 30.0622));
        // A GPS echo is noise, not movement: well under the 25m rule.
        let jitter = haversine_m(-1.9620, 30.1290, -1.96201, 30.12901);
        assert!(jitter < 5, "jitter should be ~1m, got {jitter}");
    }

    #[test]
    fn the_breadcrumb_thresholds_match_the_doc() {
        assert_eq!(BREADCRUMB_MIN_DISTANCE_M, 25);
        assert_eq!(BREADCRUMB_MIN_INTERVAL_SECS, 15);
    }
}
