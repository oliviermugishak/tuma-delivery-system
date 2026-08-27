//! JWT sessions: one token format for every client.
//!
//! Claims carry identity + role and nothing else. Business facts (a
//! merchant's store, a user's active state) are looked up from the database
//! at request time, so tokens never go stale with data.

use crate::UserRole;
use jsonwebtoken::{DecodingKey, EncodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ISSUER: &str = "tuma";

/// Mobile apps hold one long-lived token; re-authenticating is a 10-second
/// OTP flow, so expiry is a non-event for the user.
pub const MOBILE_TOKEN_TTL_SECS: usize = 30 * 24 * 3600;
/// Browsers get a short-lived access cookie; the refresh cookie silently
/// renews it (see the auth-context middleware).
pub const WEB_ACCESS_TTL_SECS: usize = 15 * 60;
pub const WEB_REFRESH_TTL_SECS: usize = 30 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Issuer, always [`ISSUER`].
    pub iss: String,
    /// Subject: the user id.
    pub sub: Uuid,
    pub role: UserRole,
    pub iat: usize,
    pub exp: usize,
}

impl Claims {
    pub fn new(user_id: Uuid, role: UserRole, ttl_secs: usize) -> Self {
        let now = time::OffsetDateTime::now_utc().unix_timestamp() as usize;
        Self {
            iss: ISSUER.to_string(),
            sub: user_id,
            role,
            iat: now,
            exp: now + ttl_secs,
        }
    }
}

pub fn generate(claims: &Claims, secret: &[u8]) -> Result<String, jsonwebtoken::errors::Error> {
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        claims,
        &EncodingKey::from_secret(secret),
    )
}

pub fn verify(token: &str, secret: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.set_issuer(&[ISSUER]);
    jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)
        .map(|data| data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(ttl_secs: usize) -> Claims {
        Claims::new(Uuid::new_v4(), UserRole::Customer, ttl_secs)
    }

    #[test]
    fn roundtrip_succeeds_with_matching_secret() {
        let token = generate(&claims(3600), b"secret").unwrap();
        let decoded = verify(&token, b"secret").unwrap();
        assert_eq!(decoded.iss, ISSUER);
        assert_eq!(decoded.role, UserRole::Customer);
    }

    #[test]
    fn rejects_expired_token() {
        let mut claims = claims(3600);
        claims.exp = claims.iat - 3600; // expired an hour ago, beyond the 60s leeway
        let token = generate(&claims, b"secret").unwrap();
        assert!(verify(&token, b"secret").is_err());
    }

    #[test]
    fn rejects_wrong_secret() {
        let token = generate(&claims(3600), b"secret").unwrap();
        assert!(verify(&token, b"other-secret").is_err());
    }

    #[test]
    fn rejects_wrong_issuer() {
        let mut claims = claims(3600);
        claims.iss = "not-tuma".to_string();
        let token = generate(&claims, b"secret").unwrap();
        assert!(verify(&token, b"secret").is_err());
    }
}
