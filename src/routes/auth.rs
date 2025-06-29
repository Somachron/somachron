use axum::{
    extract::State,
    routing::{post, Router},
    Extension,
};
use lib_core::{interceptor::ReqId, ApiResponse, Json};
use lib_domain::dto::auth::{req::ExchangeCodeRequest, res::_AuthTokenResponse};

use crate::app::AppState;

pub fn bind_routes(router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new().route("/exchange-code", post(exchange_code));

    router.nest("/auth", routes)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/exchange_code",
    responses((status=200, description="")),
    tag = "Auth"
)]
pub async fn exchange_code(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<ExchangeCodeRequest>,
) -> ApiResponse<_AuthTokenResponse> {
    let auth_code = match app.auth().exchange_code(body.code).await {
        Ok(code) => code,
        Err(err) => return ApiResponse::Err(err, req_id),
    };

    ApiResponse::map(app.service().exchange_code_routine(auth_code).await, req_id)
}
