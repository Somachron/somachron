use axum::{
    extract::State,
    http::StatusCode,
    routing::{post, Router},
    Extension,
};
use lib_core::{clerk::webhook::UserUpdateEvent, ApiError, ApiResult, EmptyResponse, Json, ReqId};
use lib_domain::{
    dto::native_app::{req::NativeAppIdentifierRequest, res::NativeAppIdentifierResponse},
    extension::Claims,
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/sync", post(sync))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate_sync))
        .route("/app-v", post(native_app_key))
        .route("/hook", post(webhook));

    router.nest("/auth", routes)
}

#[utoipa::path(
    post,
    path = "/v1/auth/sync",
    responses((status=200, body=EmptyResponse)),
    tag = "Auth"
)]
pub async fn sync(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .exchange_code_routine(claims.0)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Synced")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/auth/hook",
    responses((status=200, body=EmptyResponse)),
    tag = "Auth"
)]
pub async fn webhook(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(data): Json<UserUpdateEvent>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .webhook_update_user(data)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Synced")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/auth/app-v",
    responses((status=200, body=NativeAppIdentifierResponse)),
    tag = "Auth"
)]
pub async fn native_app_key(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(data): Json<NativeAppIdentifierRequest>,
) -> ApiResult<NativeAppIdentifierResponse> {
    app.service()
        .validate_native_app(data.identifier)
        .await
        .map(|_| {
            Json(NativeAppIdentifierResponse {
                data: app.auth().publishable_key().to_owned(),
            })
        })
        .map_err(|err| ApiError(err, req_id))
}
