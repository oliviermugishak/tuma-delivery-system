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
        crate::routes::openapi_json,
        crate::routes::me::me,
        crate::routes::me::update_me,
        crate::routes::auth::otp_request,
        crate::routes::auth::otp_verify,
        crate::routes::auth::logout,
    )
)]
pub struct ApiDoc;
