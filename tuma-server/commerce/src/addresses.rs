//! The customer's saved delivery addresses (the redesign's
//! saved-address-first checkout and the profile's plural "Delivery
//! locations"). Checkout snapshots the chosen address onto the order
//! group — the row is a source, never a live reference — so editing or
//! deleting an address never rewrites history.

use sqlx::{Acquire, PgConnection};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Address {
    pub id: Uuid,
    pub user_id: Uuid,
    pub label: String,
    pub address_text: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub is_default: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    #[error("address not found")]
    NotFound,
    #[error("label must not be empty")]
    EmptyLabel,
    #[error("address must not be empty")]
    EmptyAddress,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The caller's saved addresses, default first, then newest.
pub async fn list_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<Address>, sqlx::Error> {
    sqlx::query_as!(
        Address,
        r#"
        SELECT id, user_id, label, address_text, lat, lng, is_default,
               created_at, updated_at
        FROM commerce.addresses
        WHERE user_id = $1
        ORDER BY is_default DESC, created_at DESC
        "#,
        user_id,
    )
    .fetch_all(&mut *conn)
    .await
}

pub async fn by_id(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<Address>, sqlx::Error> {
    sqlx::query_as!(
        Address,
        r#"
        SELECT id, user_id, label, address_text, lat, lng, is_default,
               created_at, updated_at
        FROM commerce.addresses
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Create an address; the first one a user saves becomes the default
/// automatically. The `default` flag on the input is honored for
/// subsequent ones — and demotes whatever held the crown.
pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    label: &str,
    address_text: &str,
    lat: Option<f64>,
    lng: Option<f64>,
    is_default: bool,
) -> Result<Address, AddressError> {
    let label = label.trim();
    let address_text = address_text.trim();
    if label.is_empty() {
        return Err(AddressError::EmptyLabel);
    }
    if address_text.is_empty() {
        return Err(AddressError::EmptyAddress);
    }
    let mut tx = conn.begin().await?;
    if is_default {
        sqlx::query!(
            r#"
            UPDATE commerce.addresses SET is_default = FALSE
            WHERE user_id = $1 AND is_default
            "#,
            user_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM commerce.addresses WHERE user_id = $1"#,
        user_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    let force_default = count == 0;
    let address = sqlx::query_as!(
        Address,
        r#"
        INSERT INTO commerce.addresses
            (user_id, label, address_text, lat, lng, is_default)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, user_id, label, address_text, lat, lng, is_default,
                  created_at, updated_at
        "#,
        user_id,
        label,
        address_text,
        lat,
        lng,
        is_default || force_default,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(address)
}

/// Replace label/address/coordinates, with the provided-overwrites,
/// absent-keeps semantics every edit endpoint shares.
pub async fn update(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: Uuid,
    label: Option<&str>,
    address_text: Option<&str>,
    lat: Option<Option<f64>>,
    lng: Option<Option<f64>>,
    is_default: Option<bool>,
) -> Result<Address, AddressError> {
    let current = by_id(conn, user_id, id)
        .await?
        .ok_or(AddressError::NotFound)?;
    let label = label.map(str::trim).unwrap_or(&current.label);
    if label.is_empty() {
        return Err(AddressError::EmptyLabel);
    }
    let address_text = address_text.map(str::trim).unwrap_or(&current.address_text);
    if address_text.is_empty() {
        return Err(AddressError::EmptyAddress);
    }
    let lat = lat.unwrap_or(current.lat);
    let lng = lng.unwrap_or(current.lng);
    let make_default = is_default.unwrap_or(false);

    let mut tx = conn.begin().await?;
    if make_default && !current.is_default {
        sqlx::query!(
            r#"
            UPDATE commerce.addresses SET is_default = FALSE
            WHERE user_id = $1 AND is_default
            "#,
            user_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let updated = sqlx::query_as!(
        Address,
        r#"
        UPDATE commerce.addresses
        SET label = $3, address_text = $4, lat = $5, lng = $6, is_default = $7
        WHERE id = $1 AND user_id = $2
        RETURNING id, user_id, label, address_text, lat, lng, is_default,
                  created_at, updated_at
        "#,
        id,
        user_id,
        label,
        address_text,
        lat,
        lng,
        make_default || current.is_default,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(updated)
}

/// Hard-delete one of the caller's addresses. The last address may be
/// deleted; the "default" concept simply waits for the next save.
pub async fn delete(conn: &mut PgConnection, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM commerce.addresses
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}
