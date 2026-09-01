//! The customer's saved delivery addresses (redesign: checkout is
//! saved-address-first; the profile's "Delivery locations" manages the
//! list). Checkout snapshots the chosen address onto the order group, so
//! these rows are sources, never live references.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;
use validator::Validate;

use crate::app::{AppError, AppResult, AppState, UserContext, ValidatedJson};
use commerce::addresses::{self, Address, AddressError};

fn address_id(context: &UserContext) -> AppResult<Uuid> {
    context
        .user_id()
        .ok_or_else(|| AppError::Authentication("Access denied".into()))
}

/// The wire shape of a saved address (timestamps as RFC-3339).
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AddressResponse {
    pub id: Uuid,
    pub label: String,
    pub address_text: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub is_default: bool,
    /// Home / work / other.
    pub kind: String,
    /// The rider note that travels with the address.
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
}

fn address_response(address: Address) -> AddressResponse {
    AddressResponse {
        id: address.id,
        label: address.label,
        address_text: address.address_text,
        lat: address.lat,
        lng: address.lng,
        is_default: address.is_default,
        kind: address.kind,
        note: address.note,
        created_at: address.created_at,
    }
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateAddressInput {
    #[validate(length(min = 1, max = 60, message = "label must be 1-60 characters"))]
    pub label: String,
    #[validate(length(min = 1, max = 300, message = "address must be 1-300 characters"))]
    pub address_text: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    #[serde(default)]
    pub is_default: bool,
    /// Home / work / other — the save screen's label chips.
    #[validate(length(max = 20, message = "kind must be at most 20 characters"))]
    #[serde(default)]
    pub kind: Option<String>,
    /// The rider note — rides to the rider's Delivering card.
    #[validate(length(max = 140, message = "note must be at most 140 characters"))]
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateAddressInput {
    #[validate(length(min = 1, max = 60, message = "label must be 1-60 characters"))]
    pub label: Option<String>,
    #[validate(length(min = 1, max = 300, message = "address must be 1-300 characters"))]
    pub address_text: Option<String>,
    pub lat: Option<Option<f64>>,
    pub lng: Option<Option<f64>>,
    pub is_default: Option<bool>,
    #[validate(length(max = 20, message = "kind must be at most 20 characters"))]
    pub kind: Option<String>,
    #[validate(length(max = 140, message = "note must be at most 140 characters"))]
    pub note: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v1/addresses",
    responses(
        (status = 200, description = "The caller's saved addresses, default first", body = Vec<AddressResponse>),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "addresses"
)]
#[tracing::instrument(name = "List addresses", skip_all)]
pub async fn list_addresses(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
) -> AppResult<Json<Vec<AddressResponse>>> {
    let user_id = address_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let list = addresses::list_for_user(&mut conn, user_id)
        .await?
        .into_iter()
        .map(address_response)
        .collect::<Vec<_>>();
    Ok(Json(list))
}

#[utoipa::path(
    post,
    path = "/v1/addresses",
    request_body = CreateAddressInput,
    responses(
        (status = 201, description = "The saved address (first one becomes the default)", body = AddressResponse),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "addresses"
)]
#[tracing::instrument(name = "Create address", skip_all)]
pub async fn create_address(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    ValidatedJson(input): ValidatedJson<CreateAddressInput>,
) -> AppResult<(StatusCode, Json<AddressResponse>)> {
    let user_id = address_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let address = addresses::create(
        &mut conn,
        user_id,
        addresses::NewAddress {
            label: &input.label,
            address_text: &input.address_text,
            lat: input.lat,
            lng: input.lng,
            is_default: input.is_default,
            kind: input.kind.as_deref().unwrap_or("other"),
            note: input.note.as_deref(),
        },
    )
    .await
    .map_err(|error| match error {
        AddressError::NotFound => AppError::NotFound("address not found".into()),
        AddressError::EmptyLabel | AddressError::EmptyAddress => {
            AppError::BadRequest(error.to_string())
        }
        AddressError::Database(db) => AppError::Database(db),
    })?;
    Ok((StatusCode::CREATED, Json(address_response(address))))
}

#[utoipa::path(
    patch,
    path = "/v1/addresses/{id}",
    request_body = UpdateAddressInput,
    params(("id" = Uuid, Path, description = "Address id")),
    responses(
        (status = 200, description = "The updated address", body = AddressResponse),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Address not found (or not yours)"),
    ),
    tag = "addresses"
)]
#[tracing::instrument(name = "Update address", skip_all)]
pub async fn update_address(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
    ValidatedJson(input): ValidatedJson<UpdateAddressInput>,
) -> AppResult<Json<AddressResponse>> {
    let user_id = address_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    let address = addresses::update(
        &mut conn,
        user_id,
        id,
        addresses::AddressPatch {
            label: input.label.as_deref(),
            address_text: input.address_text.as_deref(),
            lat: input.lat,
            lng: input.lng,
            is_default: input.is_default,
            kind: input.kind.as_deref(),
            note: input.note.as_deref(),
        },
    )
    .await
    .map_err(|error| match error {
        AddressError::NotFound => AppError::NotFound("address not found".into()),
        AddressError::EmptyLabel | AddressError::EmptyAddress => {
            AppError::BadRequest(error.to_string())
        }
        AddressError::Database(db) => AppError::Database(db),
    })?;
    Ok(Json(address_response(address)))
}

#[utoipa::path(
    delete,
    path = "/v1/addresses/{id}",
    params(("id" = Uuid, Path, description = "Address id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Address not found (or not yours)"),
    ),
    tag = "addresses"
)]
#[tracing::instrument(name = "Delete address", skip_all)]
pub async fn delete_address(
    State(app): State<AppState>,
    Extension(context): Extension<UserContext>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user_id = address_id(&context)?;
    let mut conn = app.db_pool.acquire().await?;
    if addresses::delete(&mut conn, user_id, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("address not found".into()))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/addresses", get(list_addresses).post(create_address))
        .route(
            "/addresses/{id}",
            axum::routing::patch(update_address).delete(delete_address),
        )
}
