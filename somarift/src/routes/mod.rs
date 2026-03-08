use axum::{http::header::AUTHORIZATION, routing::Router};
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

use crate::app::AppState;

mod auth;
mod health;
mod media;
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
    let r = auth::bind_routes(app.clone(), Router::new());
    let r = user::bind_routes(app.clone(), r);
    let r = space::bind_routes(app.clone(), r);
    let r = media::bind_routes(app, r);

    router.merge(health).nest("/v1", r)
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&ApiSecurity),
    info(
        title = "Somarift API Documentation",
        description = r#"API documentation for Somarift Backend"#,
        contact(name = "API Support", email = "shashank.verma2002@gmail.com"),
        license(name = "MIT", url = "https://raw.githubusercontent.com/Somarift/somarift/refs/heads/main/LICENSE"),
    ),
    paths(
        health::health,

        auth::sync,

        user::get_user,

        space::create_space,
        space::get_user_spaces,

        media::initiate_upload,
        media::generate_thumbnail_preview_signed_urls,
        media::media_queue,
        media::list_files,
        media::create_folder,
        media::delete_folder,
    ),
    components(schemas(
        lib_core::EmptyResponse,

        lib_domain::datastore::user_space::SpaceRole,
        lib_domain::dto::Datetime,

        lib_domain::dto::user::res::UserResponse,

        lib_domain::dto::space::req::SpaceCreateRequest,
        lib_domain::dto::space::res::SpaceResponse,
        lib_domain::dto::space::res::UserSpaceResponse,

        lib_domain::dto::cloud::req::InitiateUploadRequest,
        lib_domain::dto::cloud::req::QueueMediaProcessRequest,
        lib_domain::dto::cloud::res::InitiateUploadResponse,
        lib_domain::dto::cloud::res::FileResponse,
        lib_domain::dto::cloud::res::FileMetadataResponse,
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
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new(AUTHORIZATION.as_str()))),
            )
        }
    }
}
