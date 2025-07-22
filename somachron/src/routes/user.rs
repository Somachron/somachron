use axum::{
    extract::State,
    routing::{get, Router},
    Extension,
};
use lib_core::{
    extensions::{ReqId, UserId},
    ApiError, ApiResult, Json,
};
use lib_domain::dto::user::res::{UserResponse, _UserResponse};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/", get(get_user))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/user", routes)
}

#[utoipa::path(
    get,
    path = "/v1/user",
    responses((status=200, body=UserResponse)),
    tag = "User",
    security(("api_key" = []))
)]
pub async fn get_user(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
) -> ApiResult<_UserResponse> {
    app.service().get_user(user_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}
