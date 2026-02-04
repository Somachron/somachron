use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, StatusCode},
    response::{sse::Event, Sse},
    routing::{get, post},
    Extension, Router,
};
use futures_util::{stream, StreamExt};
use lib_core::{ApiError, ApiResult, EmptyResponse, ErrType, Json, ReqId};
use smq_dto::req::ProcessMediaRequest;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};
use uuid::Uuid;

use crate::mq::{MediaQueue, QueueEvent};

pub fn bind_routes(mq: MediaQueue, router: Router<MediaQueue>) -> Router<MediaQueue> {
    // root level routes
    let health = health::bind_routes();

    // api level routes
    let routes = Router::new()
        .route("/queue", post(queue_media))
        .route("/subscribe/{id}", get(subscribe_queue))
        .layer(axum::middleware::from_fn_with_state(mq, middleware::authenticate));

    router.merge(health).nest("/v1", routes)
}

pub mod health {
    use axum::{routing::get, Router};

    use crate::mq::MediaQueue;

    pub fn bind_routes() -> Router<MediaQueue> {
        Router::new().route("/health", get(health))
    }

    #[utoipa::path(
    get,
    path = "/health",
    responses((status=200, description="Health check API")),
    tag = "Health"
)]
    pub async fn health() -> &'static str {
        "Server is up and running 🚀🚀"
    }
}

pub mod middleware {
    use axum::{
        extract::{Request, State},
        http::{header::AUTHORIZATION, HeaderMap},
        middleware::Next,
        response::Response,
        Extension,
    };
    use lib_core::{ApiError, AppResult, ErrType, ReqId};

    use crate::mq::MediaQueue;

    fn extract_bearer(headers: &HeaderMap) -> AppResult<&str> {
        let bearer_value = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .ok_or(ErrType::Unauthorized.msg("Missing authorization token"))?;

        bearer_value.split(' ').next_back().ok_or(ErrType::Unauthorized.msg("Missing bearer"))
    }

    pub async fn authenticate(
        headers: HeaderMap,
        State(mq): State<MediaQueue>,
        Extension(req_id): Extension<ReqId>,
        req: Request,
        next: Next,
    ) -> Result<Response, ApiError> {
        let token = extract_bearer(&headers).map_err(|err| ApiError(err, req_id.clone()))?;

        mq.interconnect().validate_token(token).map_err(|err| ApiError(err, req_id.clone()))?;

        Ok(next.run(req).await)
    }
}

#[utoipa::path(
    post,
    path = "/v1/queue",
    // responses((status=200, body=SpaceResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn queue_media(
    State(mq): State<MediaQueue>,
    Extension(req_id): Extension<ReqId>,
    Json(dto): Json<ProcessMediaRequest>,
) -> ApiResult<EmptyResponse> {
    mq.queue_job(dto)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Media queued for processing")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/subscribe/{id}",
    // responses((status=200, body=SpaceResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn subscribe_queue(
    State(mq): State<MediaQueue>,
    Extension(req_id): Extension<ReqId>,
    Path(file_id): Path<Uuid>,
) -> axum::response::Result<Sse<impl stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let recv = mq
        .subscribe_job(&file_id)
        .await
        .ok_or_else(|| ApiError(ErrType::NotFound.msg("Requested file id not present in queue"), req_id))?;

    // A `Stream` that repeats an event every second
    //
    // You can also create streams from tokio channels using the wrappers in
    // https://docs.rs/tokio-stream
    // let stream = stream::repeat_with(|| Event::default().data("hi!")).map(Ok);

    let stream = BroadcastStream::new(recv).map(|res| match res {
        Ok(event) => Ok(event.event()),
        Err(err) => Ok(Event::default().event("error").data(format!("stream lagged: {:?}", err))),
    });

    Ok(Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(3)).text("keep-alive-text")))
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&ApiSecurity),
    info(
        title = "Somachron Media Queue API Documentation",
        description = r#"API documentation for Somachron Media Queue"#,
        contact(name = "API Support", email = "shashank.verma2002@gmail.com"),
        license(name = "MIT", url = "https://raw.githubusercontent.com/Somachron/somachron/refs/heads/main/LICENSE"),
    ),
    paths(
        health::health,
    ),
    components(schemas(
        lib_core::EmptyResponse,

        smq_dto::res::ProcessedImage,
        smq_dto::res::ImageData,
        smq_dto::req::ProcessMediaRequest,
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
