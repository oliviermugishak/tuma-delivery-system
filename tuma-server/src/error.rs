use crate::app::AppError;
use accounts::merchants::DeleteError as MerchantDeleteError;
use accounts::otp::VerifyError;
use accounts::{ChangePasswordError, CreateAccountError};
use commerce::CheckoutError;
use commerce::TransitionError;
use marketplace::catalog::{ProductError, ProductImageError, StoreProductError};
use marketplace::stores::StoreError;
use serde::Serialize;
use storage::StorageError;
use validator::{ValidationErrors, ValidationErrorsKind};

/// Uniform JSON error body returned by every failing endpoint.
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<FieldError>>,
}

#[derive(Debug, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

pub fn validation_errors_to_field_errors(errors: ValidationErrors) -> Vec<FieldError> {
    let mut out = Vec::new();
    flatten_errors(&errors, &mut out, "");
    out
}

/// Walk the error tree to the leaves. `nested` validation (a Vec of
/// validated structs, e.g. checkout lines) stores its children under
/// `ValidationErrorsKind::List` keyed by index, and nested structs under
/// `Struct` — `field_errors()` alone would drop them all on the floor.
fn flatten_errors(errors: &ValidationErrors, out: &mut Vec<FieldError>, prefix: &str) {
    for (field, kind) in errors.errors() {
        let path = if prefix.is_empty() {
            field.to_string()
        } else {
            format!("{prefix}.{field}")
        };
        match kind {
            ValidationErrorsKind::Field(list) => {
                for error in list {
                    out.push(FieldError {
                        field: path.clone(),
                        message: error
                            .message
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| format!("invalid value for {path}")),
                    });
                }
            }
            ValidationErrorsKind::Struct(nested) => flatten_errors(nested, out, &path),
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    flatten_errors(nested, out, &format!("{path}[{index}]"));
                }
            }
        }
    }
}

// Domain error → AppError conversions live here, one impl per domain error
// type (kanombe-sda house culture). Handlers just use `?` — a new domain
// error gets its impl added here once, never map_err chains in routes.
// The client-facing message is the domain error's Display text, so wording
// is owned by the domain crate, not the HTTP layer.

impl From<CreateAccountError> for AppError {
    fn from(error: CreateAccountError) -> Self {
        match error {
            CreateAccountError::EmailTaken | CreateAccountError::PhoneTaken => {
                AppError::Conflict(error.to_string())
            }
            CreateAccountError::Password(error) => AppError::Internal(error.to_string()),
            CreateAccountError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<ChangePasswordError> for AppError {
    fn from(error: ChangePasswordError) -> Self {
        match error {
            ChangePasswordError::NoPassword => AppError::BadRequest(error.to_string()),
            ChangePasswordError::WrongCurrentPassword => {
                AppError::Authentication(error.to_string())
            }
            ChangePasswordError::Password(error) => AppError::Internal(error.to_string()),
            ChangePasswordError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<VerifyError> for AppError {
    fn from(error: VerifyError) -> Self {
        match error {
            VerifyError::InvalidCode => AppError::BadRequest(error.to_string()),
            VerifyError::PhoneTaken => AppError::Conflict(error.to_string()),
            VerifyError::Database(error) => AppError::Database(error),
        }
    }
}

// No From<otp::RequestError>: TooSoon must be swallowed by the handler (the
// response must look identical whether a code was issued or cooldown blocked
// it), so that one stays a deliberate match, never `?`.

impl From<ProductError> for AppError {
    fn from(error: ProductError) -> Self {
        match error {
            ProductError::NotFound => AppError::NotFound(error.to_string()),
            // Order history is immutable — the delete names its remedy.
            ProductError::HasOrderHistory => AppError::Conflict(error.to_string()),
            ProductError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<ProductImageError> for AppError {
    fn from(error: ProductImageError) -> Self {
        match error {
            // Missing, foreign, or mismatched — the same 404 either way.
            ProductImageError::NotFound => AppError::NotFound(error.to_string()),
            ProductImageError::GalleryFull => AppError::Conflict(error.to_string()),
            ProductImageError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<StoreProductError> for AppError {
    fn from(error: StoreProductError) -> Self {
        match error {
            // Missing resources and cross-merchant probes look identical;
            // a duplicate attachment is a genuine conflict.
            StoreProductError::NotFound | StoreProductError::ProductNotFound => {
                AppError::NotFound(error.to_string())
            }
            StoreProductError::AlreadyAttached => AppError::Conflict(error.to_string()),
            // Order history is immutable — the delete names its remedy.
            StoreProductError::HasOrderHistory => AppError::Conflict(error.to_string()),
            // A lost read-write race is a conflict with the other writer.
            StoreProductError::Stale => AppError::Conflict(error.to_string()),
            StoreProductError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<CheckoutError> for AppError {
    fn from(error: CheckoutError) -> Self {
        match error {
            // The world changed since the cart was built: a store closed,
            // an item vanished, or the shelf ran dry. A conflict with
            // current state — the details name the offenders.
            CheckoutError::Unavailable { .. }
            | CheckoutError::StoreClosed { .. }
            | CheckoutError::InsufficientStock { .. } => AppError::Conflict(error.to_string()),
            CheckoutError::EmptyCart | CheckoutError::DuplicateItems => {
                AppError::BadRequest(error.to_string())
            }
            // The idempotent-retry arms are resolved by the handler before
            // this conversion ever runs.
            CheckoutError::AlreadyPlaced(_) | CheckoutError::IdempotencyRace { .. } => {
                AppError::Internal("unresolved idempotent retry".into())
            }
            CheckoutError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<TransitionError> for AppError {
    fn from(error: TransitionError) -> Self {
        match error {
            // Another merchant's order (or an unknown one) is
            // indistinguishable from missing — ownership is the only
            // thing that matters.
            TransitionError::OrderNotFound => AppError::NotFound(error.to_string()),
            // The status string is nonsense or the transition is illegal
            // (e.g. placed → delivered). A bad request — the client asked
            // for something the state machine won't allow.
            TransitionError::Illegal { .. } => AppError::BadRequest(error.to_string()),
            // picked_up belongs to the handoff action (rider number); a
            // bare advance would create a riderless delivery.
            TransitionError::HandoffRequired => AppError::BadRequest(error.to_string()),
            TransitionError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => AppError::NotFound(error.to_string()),
            // Order history is immutable — the delete names its remedy.
            StoreError::HasOrderHistory => AppError::Conflict(error.to_string()),
            // A lost read-write race is a conflict with the other writer.
            StoreError::Stale => AppError::Conflict(error.to_string()),
            StoreError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<MerchantDeleteError> for AppError {
    fn from(error: MerchantDeleteError) -> Self {
        match error {
            // Order history is immutable — the delete names its remedy.
            MerchantDeleteError::HasOrderHistory => AppError::Conflict(error.to_string()),
            MerchantDeleteError::Database(db) => AppError::Database(db),
        }
    }
}

impl From<StorageError> for AppError {
    fn from(error: StorageError) -> Self {
        match error {
            // The bytes are not an image we accept — a 415, not a 400:
            // the request was well-formed, the media is not.
            StorageError::NotAnImage => AppError::UnsupportedMediaType(error.to_string()),
            StorageError::TooLarge => AppError::PayloadTooLarge(error.to_string()),
            StorageError::Decode(_) | StorageError::Store(_) => {
                AppError::Internal(error.to_string())
            }
            StorageError::Configuration(message) => AppError::Internal(message),
        }
    }
}

impl From<accounts::riders::RiderError> for AppError {
    fn from(error: accounts::riders::RiderError) -> Self {
        match error {
            // Unknown and foreign rider ids are the same 404.
            accounts::riders::RiderError::NotFound => AppError::NotFound(error.to_string()),
            // Assignment history is operationally real: deactivate instead.
            accounts::riders::RiderError::HasDeliveries => AppError::Conflict(error.to_string()),
            accounts::riders::RiderError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<commerce::deliveries::DeliveryError> for AppError {
    fn from(error: commerce::deliveries::DeliveryError) -> Self {
        use commerce::deliveries::DeliveryError;
        match error {
            // Anti-probe: a foreign delivery is indistinguishable from a
            // missing one.
            DeliveryError::NotFound => AppError::NotFound(error.to_string()),
            // The delivery exists but is not moving — pushing positions to
            // a delivery that is not out for delivery is a conflict, not a
            // lie.
            DeliveryError::NotOutForDelivery => AppError::Conflict(error.to_string()),
            // The six-state machine refusing a skip is the client asking
            // for something impossible.
            DeliveryError::Illegal { .. } => AppError::BadRequest(error.to_string()),
            DeliveryError::Database(error) => AppError::Database(error),
        }
    }
}
