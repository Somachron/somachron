use axum::{
    extract::State,
    http::StatusCode,
    routing::{post, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, EmptyResponse, Json, ReqId};
use lib_domain::extension::Claims;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/sync", post(sync))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate_sync));

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
