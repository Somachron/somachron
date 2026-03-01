use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, EmptyResponse, Json, ReqId};
use lib_domain::{
    dto::space::{
        req::{SpaceCreateRequest, SpaceMemberRequest, UpdateSpaceMemberRoleRequest},
        res::{
            SpaceResponse, SpaceUserResponse, UserSpaceResponse, _SpaceResponse, _SpaceUserResponseVec,
            _UserSpaceResponseVec,
        },
    },
    extension::{SpaceCtx, UserId},
    service::{space::SpaceService, user_space::UserSpaceService},
};
use uuid::Uuid;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/users", get(get_space_users))
        .route("/users", post(add_user_to_space))
        .route("/users", delete(remove_user_from_space))
        .route("/users", put(update_user_space_role))
        .route("/users/self", delete(leave_space))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::space::validate_user_space))
        .route("/", post(create_space))
        .route("/", get(get_user_spaces))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::auth::authenticate));

    let special_routes = Router::new()
        .route("/default/{user_id}", post(get_or_setup_default_space))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate_interconnect));

    router.nest("/space", routes).nest("/space", special_routes)
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
    app.services()
        .space_service()
        .create_user_space(user_id, app.storage(), dto)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
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
    app.services()
        .user_space_service()
        .get_spaces_for_user(user_id)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/space/users",
    responses((status=200, body=Vec<SpaceUserResponse>)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn get_space_users(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<_SpaceUserResponseVec> {
    app.services()
        .user_space_service()
        .get_users_for_space(space_ctx)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/space/users",
    responses((status=200, body=EmptyResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn add_user_to_space(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(dto): Json<SpaceMemberRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .user_space_service()
        .add_user_to_space(space_ctx, dto.user_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "User added to space")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    put,
    path = "/v1/space/users",
    responses((status=200, body=EmptyResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn update_user_space_role(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(dto): Json<UpdateSpaceMemberRoleRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .user_space_service()
        .update_user_space_role(user_id, space_ctx, dto.user_id, dto.role)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "User role updated")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    delete,
    path = "/v1/space/users",
    responses((status=200, body=EmptyResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn remove_user_from_space(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(dto): Json<SpaceMemberRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .user_space_service()
        .remove_user_from_space(space_ctx, dto.user_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "User removed from space")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    delete,
    path = "/v1/space/users/self",
    responses((status=200, body=EmptyResponse)),
    tag = "Space",
    security(("api_key" = []))
)]
pub async fn leave_space(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .user_space_service()
        .leave_space(space_ctx)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "User removed from space")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/space/default/{user_id}",
    responses((status=200, body=SpaceResponse)),
    tag = "Default Space",
    security(("api_key" = []))
)]
pub async fn get_or_setup_default_space(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Path(user_id): Path<Uuid>,
) -> ApiResult<_SpaceResponse> {
    app.services()
        .space_service()
        .get_or_setup_default_space(user_id, app.storage())
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}
