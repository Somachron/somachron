use axum::{
    extract::State,
    routing::{get, post, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, Json, ReqId};
use lib_domain::{
    dto::space::{
        req::SpaceCreateRequest,
        res::{SpaceResponse, UserSpaceResponse, _SpaceResponse, _UserSpaceResponseVec},
    },
    extension::UserId,
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/", post(create_space))
        .route("/", get(get_user_spaces))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/space", routes)
}

#[utoipa::path(
    post,
    path = "/v1/space",
    responses((status=200, body=SpaceResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn create_space(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Json(dto): Json<SpaceCreateRequest>,
) -> ApiResult<_SpaceResponse> {
    app.service().create_user_space(user_id, app.storage(), dto).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/space",
    responses((status=200, body=Vec<UserSpaceResponse>)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn get_user_spaces(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
) -> ApiResult<_UserSpaceResponseVec> {
    app.service().get_user_spaces(user_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}
