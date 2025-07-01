use axum::{
    extract::State,
    routing::{post, Router},
    Extension,
};
use lib_core::{interceptor::ReqId, ApiError, ApiResult, Json};
use lib_domain::dto::auth::{
    req::{ExchangeCodeRequest, RefreshTokenRequest},
    res::_AuthTokenResponse,
};

use crate::app::AppState;

pub fn bind_routes(router: Router<AppState>) -> Router<AppState> {
    let routes =
        Router::new().route("/exchange-code", post(exchange_code)).route("/refresh-token", post(refresh_token));

    router.nest("/auth", routes)
}

#[utoipa::path(
    post,
    path = "/v1/auth/exchange-code",
    responses((status=200, description="")),
    tag = "Auth"
)]
pub async fn exchange_code(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<ExchangeCodeRequest>,
) -> ApiResult<_AuthTokenResponse> {
    let auth_code = app.auth().exchange_code(body.code).await.map_err(|err| ApiError(err, req_id.clone()))?;
    let claims =
        app.auth().validate_token_for_claims(&auth_code.id_token).await.map_err(|err| ApiError(err, req_id.clone()))?;

    app.service().exchange_code_routine(claims).await.map_err(|err| ApiError(err, req_id.clone()))?;

    Ok(Json(_AuthTokenResponse(auth_code)))
}

#[utoipa::path(
    post,
    path = "/v1/auth/refresh-token",
    responses((status=200, description="")),
    tag = "Auth"
)]
pub async fn refresh_token(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<RefreshTokenRequest>,
) -> ApiResult<_AuthTokenResponse> {
    let auth_code = app.auth().refresh_token(body.refresh_token).await.map_err(|err| ApiError(err, req_id.clone()))?;
    Ok(Json(_AuthTokenResponse(auth_code)))
}
