use crate::app::AppError;
use crate::domain::stores::{ProductError, StoreError};
use accounts::otp::VerifyError;
use accounts::{ChangePasswordError, CreateAccountError};
use serde::Serialize;
use validator::ValidationErrors;

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
    errors
        .field_errors()
        .iter()
        .flat_map(|(field, errors)| {
            errors.iter().map(move |error| FieldError {
                field: field.to_string(),
                message: error
                    .message
                    .as_ref()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| format!("invalid value for {}", field)),
            })
        })
        .collect()
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

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => AppError::NotFound(error.to_string()),
            StoreError::Database(error) => AppError::Database(error),
        }
    }
}

impl From<ProductError> for AppError {
    fn from(error: ProductError) -> Self {
        match error {
            // StoreNotFound is a missing resource (the target store), not
            // bad input.
            ProductError::StoreNotFound | ProductError::NotFound => {
                AppError::NotFound(error.to_string())
            }
            ProductError::Database(error) => AppError::Database(error),
        }
    }
}
