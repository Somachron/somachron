use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
    Extension,
};
use lib_core::{
    extensions::{ReqId, UserId},
    ApiError, AppResult, ErrType,
};

use crate::app::AppState;

fn extract_bearer(headers: &HeaderMap) -> AppResult<&str> {
    let bearer_value = headers
        .get(super::AUTHORIZATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .ok_or(ErrType::Unauthorized.new("Missing authorization token"))?;

    bearer_value.split(' ').last().ok_or(ErrType::Unauthorized.new("Missing bearer"))
}

pub async fn authenticate(
    headers: HeaderMap,
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = extract_bearer(&headers).map_err(|err| ApiError(err, req_id.clone()))?;

    let claims = app.auth().validate_token_for_claims(token).await.map_err(|err| ApiError(err, req_id.clone()))?;
    let user_id = app
        .service()
        .ds()
        .get_user_id(&claims.email)
        .await
        .map(|id| id.ok_or(ApiError(ErrType::Unauthorized.new("User not found"), req_id.clone())))
        .map_err(|err| ApiError(err, req_id.clone()))??;

    let user_id = UserId(user_id.into());

    req.extensions_mut().insert(user_id);

    Ok(next.run(req).await)
}
