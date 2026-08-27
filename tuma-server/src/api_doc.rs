use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Tuma API",
        version = "0.1.0",
        description = "Backend API for the Tuma commerce + delivery platform"
    ),
    paths(
        crate::routes::health_check,
    )
)]
pub struct ApiDoc;
