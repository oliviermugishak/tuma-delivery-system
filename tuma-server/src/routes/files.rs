//! Public file serving: `GET /api/v1/files/{*key}` streams an object from
//! the configured backend. This is the dev/LAN read path; in production
//! `storage.public_base_url` points at the R2 public URL (Cloudflare CDN)
//! and this route idles as a fallback. Unauthenticated by design — keys
//! are unguessable content-UUIDs, and images are public merchandise.

use crate::app::{AppError, AppResult, AppState};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

#[utoipa::path(
    get,
    path = "/v1/files/{key}",
    params(
        ("key" = String, Path, description = "Storage key of the object"),
    ),
    responses(
        (status = 200, description = "The stored object's bytes", content_type = "application/octet-stream"),
        (status = 404, description = "No object at that key"),
    ),
    tag = "files"
)]
#[tracing::instrument(name = "Serve stored file", skip_all, fields(key = %key))]
pub async fn get_file(State(app): State<AppState>, Path(key): Path<String>) -> AppResult<Response> {
    let path =
        storage::object_path(&key).map_err(|_| AppError::NotFound("file not found".into()))?;

    match app.storage.store.get(&path).await {
        Ok(result) => {
            let size = result.meta.size;
            let content_type = storage::content_type_for(&key).to_string();
            let body = Body::from_stream(result.into_stream());
            // Keys are content-UUIDs that are never overwritten, so every
            // object is safe to cache forever — the whole point of the
            // key design. (Subsequent uploads write NEW keys.)
            Ok((
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CONTENT_LENGTH, size.to_string()),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=31536000, immutable".to_string(),
                    ),
                ],
                body,
            )
                .into_response())
        }
        Err(error) if storage::is_not_found(&error) => {
            Err(AppError::NotFound("file not found".into()))
        }
        Err(error) => {
            tracing::debug!("File read failed: {}", error);
            Err(AppError::Internal("could not read the file".into()))
        }
    }
}
