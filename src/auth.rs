use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::db::Database;

/// Authenticated user extracted from the `Authorization: Basic` header.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
}

/// Extractor that parses and validates HTTP Basic Auth credentials.
///
/// Usage: add `auth: AuthUser` as a parameter in any handler that requires authentication.
#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync + 'static,
    Arc<Database>: FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract shared state
        let db = Arc::<Database>::from_ref(state);

        // 1. Extract the Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .ok_or(AuthError::MissingCredentials)?
            .to_str()
            .map_err(|_| AuthError::InvalidHeader)?;

        // 2. Check that it starts with "Basic "
        let encoded = auth_header
            .strip_prefix("Basic ")
            .ok_or(AuthError::InvalidScheme)?;

        // 3. Base64-decode the credentials
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            encoded,
        )
        .map_err(|_| AuthError::InvalidEncoding)?;

        let credential_str =
            String::from_utf8(decoded).map_err(|_| AuthError::InvalidEncoding)?;

        // 4. Split into username:password (only first colon matters)
        let (username, password) = credential_str
            .split_once(':')
            .ok_or(AuthError::InvalidFormat)?;

        // 5. Look up user in the database and verify password
        let user = db
            .verify_user(username, password)
            .map_err(|_| AuthError::InternalError)?;

        let (user_id, _) = user.ok_or(AuthError::Unauthorized)?;

        Ok(AuthUser { user_id })
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AuthError {
    MissingCredentials,
    InvalidHeader,
    InvalidScheme,
    InvalidEncoding,
    InvalidFormat,
    Unauthorized,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingCredentials
            | AuthError::InvalidHeader
            | AuthError::InvalidScheme => {
                (StatusCode::UNAUTHORIZED, "Missing or malformed Authorization header")
            }
            AuthError::InvalidEncoding | AuthError::InvalidFormat => {
                (StatusCode::UNAUTHORIZED, "Invalid credentials format")
            }
            AuthError::Unauthorized => (StatusCode::UNAUTHORIZED, "Invalid username or password"),
            AuthError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}