use axum::routing::Router;
use utoipa::OpenApi;

use crate::app::AppState;

mod auth;
mod health;

/// Function to bind routes from:
/// - [`health`]
/// - [`vault`]
pub fn bind_routes(router: Router<AppState>) -> Router<AppState> {
    // root level routes
    let health = health::bind_routes();

    // api level routes
    let r = auth::bind_routes(Router::new());

    router.merge(health).nest("/v1", r)
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Somachron API Documentation",
        description = r#"API documentation for Somachron Backend"#,
        contact(name = "API Support", email = "shashank.verma2002@gmail.com"),
        license(name = "MIT", url = "https://raw.githubusercontent.com/Somachron/somachron/refs/heads/main/LICENSE"),
    ),
    paths(health::health, auth::exchange_code, auth::refresh_token),
    components(schemas(
        lib_core::EmptyResponse,
        lib_domain::dto::auth::req::ExchangeCodeRequest,
        lib_domain::dto::auth::req::RefreshTokenRequest,
        lib_domain::dto::auth::res::AuthTokenResponse,
    )),
    servers()
)]
pub struct ApiDoc;
