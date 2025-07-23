use axum::{
    extract::State,
    http::StatusCode,
    routing::{post, Router},
    Extension,
};
use lib_core::{extensions::ReqId, ApiError, ApiResult, EmptyResponse, Json};
use lib_domain::dto::auth::{
    req::{ExchangeCodeRequest, RefreshTokenRequest, RevokeTokenRequest},
    res::{AuthTokenResponse, _AuthTokenResponse},
};

use crate::app::AppState;

pub fn bind_routes(router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/exchange-code", post(exchange_code))
        .route("/refresh-token", post(refresh_token))
        .route("/revoke-token", post(revoke_token));

    router.nest("/auth", routes)
}

#[utoipa::path(
    post,
    path = "/v1/auth/exchange-code",
    responses((status=200, body=AuthTokenResponse)),
    tag = "Auth"
)]
pub async fn exchange_code(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<ExchangeCodeRequest>,
) -> ApiResult<_AuthTokenResponse> {
    let auth_code = app.auth().exchange_code(body.code).await.map_err(|err| ApiError(err, req_id.clone()))?;
    let claims =
        app.auth().validate_token_for_claims(&auth_code.id_token).map_err(|err| ApiError(err, req_id.clone()))?;

    let user_id = app.service().exchange_code_routine(claims).await.map_err(|err| ApiError(err, req_id.clone()))?;
    app.storage().validate_user_drive(&user_id).await.map_err(|err| ApiError(err, req_id))?;

    Ok(Json(_AuthTokenResponse(auth_code)))
}

#[utoipa::path(
    post,
    path = "/v1/auth/refresh-token",
    responses((status=200, body=AuthTokenResponse)),
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

#[utoipa::path(
    post,
    path = "/v1/auth/revoke-token",
    responses((status=200, body=EmptyResponse)),
    tag = "Auth"
)]
pub async fn revoke_token(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<RevokeTokenRequest>,
) -> ApiResult<EmptyResponse> {
    app.auth().revoke_token(&body.token).await.map_err(|err| ApiError(err, req_id.clone()))?;
    Ok(Json(EmptyResponse::new(StatusCode::OK, "Token revoked")))
}
