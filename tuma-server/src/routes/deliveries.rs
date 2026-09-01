//! The rider audience namespace (build order #4, slice D2): the rider's
//! phone checking in and handing over, plus the work list. Riders
//! authenticate with the exact customer OTP flow; the guard resolves the
//! rider profile fresh per request. Every endpoint verifies ownership —
//! another rider's delivery is indistinguishable from a missing one (404).

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use commerce::deliveries;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

fn rider_id(context: &UserContext) -> AppResult<Uuid> {
    context
        .rider_id()
        .ok_or_else(|| AppError::Internal("rider route without a rider profile".into()))
}

// ---------------------------------------------------------------------------
// Rider endpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LocationInput {
    #[validate(range(min = -90.0, max = 90.0, message = "lat must be between -90 and 90"))]
    pub lat: f64,
    #[validate(range(min = -180.0, max = 180.0, message = "lng must be between -180 and 180"))]
    pub lng: f64,
}

/// The rider's phone checking in (~every 5s while delivering). The
/// ≥25m/15s GPS-noise rule lives in the domain — a throttled push answers
/// the same 204 as a recorded one, so the client cannot tell and need not
/// care.
#[utoipa::path(
    post,
    path = "/v1/deliveries/{id}/location",
    params(("id" = Uuid, Path, description = "Delivery id")),
    request_body = LocationInput,
    responses(
        (status = 204, description = "Position accepted (recorded or throttled — same answer)"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a rider"),
        (status = 404, description = "Not one of this rider's deliveries"),
        (status = 409, description = "The delivery is not out for delivery"),
        (status = 422, description = "Invalid coordinates"),
    ),
    tag = "deliveries"
)]
#[tracing::instrument(name = "Push delivery location", skip_all)]
pub async fn push_location(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<LocationInput>,
) -> AppResult<StatusCode> {
    let rider = rider_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let outcome = deliveries::push_location(&mut conn, id, rider, input.lat, input.lng).await?;

    // The rider strayed off the cached route (≥200m — tracking doc §4):
    // re-route from where they actually are to the destination. A backend
    // failure or a no-route answer keeps the old polyline — losing a
    // still-valid route to an outage would be worse — and only re-arms
    // the ETA from the new position.
    if outcome == deliveries::PushOutcome::Strayed {
        let points = deliveries::route_context_for_delivery(&mut conn, id)
            .await?
            .filter(|points| points.destination_lat.is_some() && points.destination_lng.is_some());
        if let Some(points) = points {
            let now = time::OffsetDateTime::now_utc();
            let to = routing::Coord {
                lat: points.destination_lat.unwrap(),
                lng: points.destination_lng.unwrap(),
            };
            let here = routing::Coord {
                lat: input.lat,
                lng: input.lng,
            };
            let cached = match app.routing.route(here, to).await {
                Ok(Some(route)) => commerce::CachedRoute {
                    polyline: Some(route.polyline),
                    eta_target: now + time::Duration::seconds(route.duration_secs),
                },
                other => {
                    if let Err(error) = other {
                        tracing::warn!(
                            error = %error,
                            "re-route after stray failed — re-arming the ETA estimate only"
                        );
                    }
                    let distance =
                        marketplace::geo::haversine_m(here.lat, here.lng, to.lat, to.lng);
                    commerce::CachedRoute {
                        polyline: None,
                        eta_target: now
                            + time::Duration::minutes(marketplace::geo::ride_minutes(distance)),
                    }
                }
            };
            deliveries::update_delivery_route(&mut conn, id, cached).await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The rider's Delivered action: food handed over + cash received, one
/// real event. Advances the store order and settles that delivery's
/// payment allocation (the group's payment becomes collected when every
/// allocation is settled — the tracking doc's cash state).
#[utoipa::path(
    post,
    path = "/v1/deliveries/{id}/delivered",
    params(("id" = Uuid, Path, description = "Delivery id")),
    responses(
        (status = 200, description = "The delivered store order", body = DeliveredResponse),
        (status = 400, description = "The delivery is not in a deliverable state"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a rider"),
        (status = 404, description = "Not one of this rider's deliveries"),
    ),
    tag = "deliveries"
)]
#[tracing::instrument(name = "Mark delivery delivered", skip_all)]
pub async fn mark_delivered(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DeliveredResponse>> {
    let rider = rider_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let order = deliveries::mark_delivered(&mut conn, id, rider).await?;
    Ok(Json(DeliveredResponse {
        store_order_id: order.id,
        status: order.status,
        delivered_at: order.updated_at,
    }))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeliveredResponse {
    pub store_order_id: Uuid,
    #[schema(value_type = String)]
    pub status: commerce::OrderStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub delivered_at: OffsetDateTime,
}

/// The rider's active work list — one card per delivery still out for
/// delivery: the store to collect from, the destination, and the customer
/// to call when close (the Kigali protocol — the data already flows).
#[utoipa::path(
    get,
    path = "/v1/deliveries",
    responses(
        (status = 200, description = "This rider's deliveries that are out for delivery, newest handoff first", body = Vec<RiderDeliveryResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a rider"),
    ),
    tag = "deliveries"
)]
#[tracing::instrument(name = "List rider deliveries", skip_all)]
pub async fn list_rider_deliveries(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
) -> AppResult<Json<Vec<RiderDeliveryResponse>>> {
    let rider = rider_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let rows = deliveries::active_deliveries_for_rider(&mut conn, rider).await?;
    Ok(Json(
        rows.into_iter().map(RiderDeliveryResponse::from).collect(),
    ))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RiderDeliveryResponse {
    pub delivery_id: Uuid,
    pub store_order_id: Uuid,
    pub store_name: String,
    pub store_address: Option<String>,
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
    pub destination_address: String,
    pub destination_lat: Option<f64>,
    pub destination_lng: Option<f64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    #[schema(value_type = String)]
    pub status: commerce::OrderStatus,
    #[serde(with = "time::serde::rfc3339::option")]
    pub handoff_at: Option<OffsetDateTime>,
    pub route_polyline: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub eta_target: Option<OffsetDateTime>,
    pub last_lat: Option<f64>,
    pub last_lng: Option<f64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_location_at: Option<OffsetDateTime>,
}

impl From<deliveries::ActiveDelivery> for RiderDeliveryResponse {
    fn from(row: deliveries::ActiveDelivery) -> Self {
        Self {
            delivery_id: row.delivery_id,
            store_order_id: row.store_order_id,
            store_name: row.store_name,
            store_address: row.store_address,
            store_lat: row.store_lat,
            store_lng: row.store_lng,
            destination_address: row.destination_address,
            destination_lat: row.destination_lat,
            destination_lng: row.destination_lng,
            customer_name: row.customer_name,
            customer_phone: row.customer_phone,
            status: row.status,
            handoff_at: row.handoff_at,
            route_polyline: row.route_polyline,
            eta_target: row.eta_target,
            last_lat: row.last_lat,
            last_lng: row.last_lng,
            last_location_at: row.last_location_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Customer tracking (registered on the /orders nest — see app.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct TrackingQuery {
    /// The `changed_at` the client last saw. When nothing in the group is
    /// newer, the answer is 204 — battery and payload win.
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub since: Option<OffsetDateTime>,
}

/// The customer's tracking snapshot for one order group: one delivery
/// entry per store order (each its own story), the real last positions,
/// the cached route, the ETA target, and the freshness marker the next
/// poll echoes back as `since`. 204 when nothing changed since.
#[utoipa::path(
    get,
    path = "/v1/orders/{id}/tracking",
    params(("id" = Uuid, Path, description = "Order group id"),
           ("since" = Option<OffsetDateTime>, Query, description = "RFC-3339 timestamp; 204 when the group has nothing newer")),
    responses(
        (status = 200, description = "The tracking snapshot", body = TrackingResponse),
        (status = 204, description = "Nothing changed since `since`"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Not a customer"),
        (status = 404, description = "Another customer's order — indistinguishable from a missing one"),
    ),
    tag = "orders"
)]
#[tracing::instrument(name = "Order tracking", skip_all)]
pub async fn order_tracking(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<TrackingQuery>,
) -> AppResult<Response> {
    let user = context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))?;
    let mut conn = app.db_pool.acquire().await?;

    // The freshness contract: the client echoes `changed_at` back as
    // `since`; equal-or-newer means nothing moved — 204, decided by a
    // ONE-query probe before any geometry, trail, or payment is loaded.
    // Every heartbeat poll takes this path, so the cheap answer is the
    // common one.
    let changed_at = commerce::deliveries::tracking_changed_at(&mut conn, user, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;
    if query.since.is_some_and(|since| since >= changed_at) {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let tracking = commerce::deliveries::tracking_for_user(&mut conn, user, id)
        .await?
        .ok_or_else(|| AppError::NotFound("order not found".into()))?;

    let entries = tracking
        .deliveries
        .iter()
        .map(|delivery| DeliveryTrackingResponse {
            store_order_id: delivery.store_order_id,
            store_name: delivery.store_name.clone(),
            store_lat: delivery.store_lat,
            store_lng: delivery.store_lng,
            store_contact_phone: delivery.store_contact_phone.clone(),
            status: delivery.status,
            handoff_at: delivery.handoff_at,
            route_polyline: delivery.route_polyline.clone(),
            eta_target: delivery.eta_target,
            last_lat: delivery.last_lat,
            last_lng: delivery.last_lng,
            last_location_at: delivery.last_location_at,
            updated_at: delivery.updated_at,
            trail: tracking
                .trails
                .iter()
                .filter(|point| point.delivery_id == delivery.delivery_id)
                .map(|point| TrailResponse {
                    lat: point.lat,
                    lng: point.lng,
                    recorded_at: point.recorded_at,
                })
                .collect(),
        })
        .collect();

    Ok(Json(TrackingResponse {
        group_id: tracking.group_id,
        group_status: tracking.group_status,
        payment_status: tracking.payment_status,
        changed_at: tracking.changed_at,
        deliveries: entries,
    })
    .into_response())
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TrackingResponse {
    pub group_id: Uuid,
    #[schema(value_type = String)]
    pub group_status: commerce::GroupStatus,
    #[schema(value_type = String)]
    pub payment_status: commerce::PaymentStatus,
    /// Echo this back as `since` on the next poll.
    #[serde(with = "time::serde::rfc3339")]
    pub changed_at: OffsetDateTime,
    pub deliveries: Vec<DeliveryTrackingResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeliveryTrackingResponse {
    /// The per-store card key — the client joins it to the group detail's
    /// store orders.
    pub store_order_id: Uuid,
    pub store_name: String,
    pub store_lat: Option<f64>,
    pub store_lng: Option<f64>,
    /// The overdue customer's "call the store" action (D5 surfaces it).
    pub store_contact_phone: Option<String>,
    #[schema(value_type = String)]
    pub status: commerce::OrderStatus,
    #[serde(with = "time::serde::rfc3339::option")]
    pub handoff_at: Option<OffsetDateTime>,
    /// The road route cached at handoff, or null when no routing backend
    /// is configured — never invented geometry.
    pub route_polyline: Option<String>,
    /// Server-provided target the client decays against.
    #[serde(with = "time::serde::rfc3339::option")]
    pub eta_target: Option<OffsetDateTime>,
    pub last_lat: Option<f64>,
    pub last_lng: Option<f64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_location_at: Option<OffsetDateTime>,
    /// The delivery's last write — for a settled delivery, the delivered
    /// moment the customer's "Delivered · time" line shows (D5).
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    /// The recent real positions (tail), oldest first.
    pub trail: Vec<TrailResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TrailResponse {
    pub lat: f64,
    pub lng: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub recorded_at: OffsetDateTime,
}
