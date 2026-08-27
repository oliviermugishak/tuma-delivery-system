#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server is healthy", body = String),
    ),
    tag = "health"
)]
#[tracing::instrument(name = "Health check")]
pub async fn health_check() -> &'static str {
    "OK"
}
