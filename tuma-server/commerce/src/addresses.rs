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
    /// Home / work / other — the save screen's label chips.
    pub kind: String,
    /// The rider note that travels with the address.
    pub note: Option<String>,
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
               kind, note, created_at, updated_at
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
               kind, note, created_at, updated_at
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
/// The editable surface of a new address.
pub struct NewAddress<'a> {
    pub label: &'a str,
    pub address_text: &'a str,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub is_default: bool,
    pub kind: &'a str,
    pub note: Option<&'a str>,
}

pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    input: NewAddress<'_>,
) -> Result<Address, AddressError> {
    let label = input.label;
    let address_text = input.address_text;
    let is_default = input.is_default;
    let kind = input.kind;
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
            (user_id, label, address_text, lat, lng, is_default, kind, note)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, user_id, label, address_text, lat, lng, is_default,
                  kind, note, created_at, updated_at
        "#,
        user_id,
        label,
        address_text,
        input.lat,
        input.lng,
        is_default || force_default,
        kind,
        input.note,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(address)
}

/// Replace label/address/coordinates, with the provided-overwrites,
/// absent-keeps semantics every edit endpoint shares.
/// The editable patch for one address: provided overwrites, absent keeps.
pub struct AddressPatch<'a> {
    pub label: Option<&'a str>,
    pub address_text: Option<&'a str>,
    pub lat: Option<Option<f64>>,
    pub lng: Option<Option<f64>>,
    pub is_default: Option<bool>,
    pub kind: Option<&'a str>,
    pub note: Option<&'a str>,
}

pub async fn update(
    conn: &mut PgConnection,
    user_id: Uuid,
    id: Uuid,
    patch: AddressPatch<'_>,
) -> Result<Address, AddressError> {
    let current = by_id(conn, user_id, id)
        .await?
        .ok_or(AddressError::NotFound)?;
    let label = patch.label.map(str::trim).unwrap_or(&current.label);
    if label.is_empty() {
        return Err(AddressError::EmptyLabel);
    }
    let address_text = patch
        .address_text
        .map(str::trim)
        .unwrap_or(&current.address_text);
    if address_text.is_empty() {
        return Err(AddressError::EmptyAddress);
    }
    let lat = patch.lat.unwrap_or(current.lat);
    let lng = patch.lng.unwrap_or(current.lng);
    let kind = patch
        .kind
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .unwrap_or(&current.kind);
    let make_default = patch.is_default.unwrap_or(false);

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
        SET label = $3, address_text = $4, lat = $5, lng = $6, is_default = $7,
            kind = $8, note = $9
        WHERE id = $1 AND user_id = $2
        RETURNING id, user_id, label, address_text, lat, lng, is_default,
                  kind, note, created_at, updated_at
        "#,
        id,
        user_id,
        label,
        address_text,
        lat,
        lng,
        make_default || current.is_default,
        kind,
        patch.note,
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
