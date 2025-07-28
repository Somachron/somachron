use axum::routing::Router;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

use crate::app::AppState;

mod auth;
mod cloud;
mod health;
mod middleware;
mod space;
mod user;

/// Function to bind routes from:
/// - [`health`]
/// - [`vault`]
pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    // root level routes
    let health = health::bind_routes();

    // api level routes
    let r = auth::bind_routes(Router::new());
    let r = user::bind_routes(app.clone(), r);
    let r = space::bind_routes(app.clone(), r);
    let r = cloud::bind_routes(app, r);

    router.merge(health).nest("/v1", r)
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&ApiSecurity),
    info(
        title = "Somachron API Documentation",
        description = r#"API documentation for Somachron Backend"#,
        contact(name = "API Support", email = "shashank.verma2002@gmail.com"),
        license(name = "MIT", url = "https://raw.githubusercontent.com/Somachron/somachron/refs/heads/main/LICENSE"),
    ),
    paths(
        health::health,

        auth::exchange_code,
        auth::refresh_token,
        auth::revoke_token,

        user::get_user,

        space::create_space,
        space::get_user_spaces,

        cloud::generate_upload_signed_url,
        cloud::generate_download_signed_url,
        cloud::upload_completion,
        cloud::list_directory,
        cloud::create_folder,
        cloud::delete_path,
    ),
    components(schemas(
        lib_core::EmptyResponse,
        lib_core::storage::FileMetadata,
        lib_core::storage::FileEntry,

        lib_domain::datastore::user_space::UserRole,
        lib_domain::dto::Datetime,

        lib_domain::dto::auth::req::ExchangeCodeRequest,
        lib_domain::dto::auth::req::RefreshTokenRequest,
        lib_domain::dto::auth::req::RevokeTokenRequest,
        lib_domain::dto::auth::res::AuthTokenResponse,

        lib_domain::dto::user::res::UserResponse,

        lib_domain::dto::space::req::SpaceCreateRequest,
        lib_domain::dto::space::res::SpaceResponse,
        lib_domain::dto::space::res::UserSpaceResponse,

        lib_domain::dto::cloud::req::SignedUrlRequest,
        lib_domain::dto::cloud::req::UploadCompleteRequest,
        lib_domain::dto::cloud::res::SignedUrlResponse,
    )),
    servers()
)]
pub struct ApiDoc;

struct ApiSecurity;

impl Modify for ApiSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // let server = Server::new(Config::get_backend_base_url());
        // if let Some(servers) = openapi.servers.as_mut() {
        //     servers.push(server)
        // } else {
        //     openapi.servers.get_or_insert(vec![server]);
        // }

        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new(middleware::AUTHORIZATION_HEADER))),
            )
        }
    }
}
