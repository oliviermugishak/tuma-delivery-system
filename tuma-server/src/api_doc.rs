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
        crate::routes::auth::login,
        crate::routes::auth::logout,
        crate::routes::auth::change_password,
        crate::routes::admin::create_merchant,
        crate::routes::admin::list_merchants,
        crate::routes::admin::get_merchant,
        crate::routes::admin::update_merchant,
        crate::routes::admin::delete_merchant,
        crate::routes::admin::list_customers,
        crate::routes::admin::update_customer,
        crate::routes::admin::delete_customer,
        crate::routes::admin::summary,
        crate::routes::stores::create_own_store,
        crate::routes::stores::list_own_stores,
        crate::routes::stores::get_own_store,
        crate::routes::stores::update_own_store,
        crate::routes::stores::delete_own_store,
        crate::routes::stores::list_own_products,
        crate::routes::stores::create_product,
        crate::routes::stores::update_product,
        crate::routes::stores::delete_product,
        crate::routes::stores::list_stores,
        crate::routes::stores::get_store,
    )
)]
pub struct ApiDoc;
