use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
    Extension,
};
use lib_core::{ApiError, AppResult, ErrType, ReqId};
use lib_domain::{
    datastore::user::UserDs,
    extension::{Claims, UserId},
};

use crate::app::AppState;

fn extract_bearer(headers: &HeaderMap) -> AppResult<&str> {
    let bearer_value = headers
        .get(super::AUTHORIZATION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .ok_or(ErrType::Unauthorized.msg("Missing authorization token"))?;

    bearer_value.split(' ').next_back().ok_or(ErrType::Unauthorized.msg("Missing bearer"))
}

pub async fn authenticate(
    headers: HeaderMap,
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = extract_bearer(&headers).map_err(|err| ApiError(err, req_id.clone()))?;

    let claims = app.auth().validate_token_for_claims(token).map_err(|err| ApiError(err, req_id.clone()))?;
    let user = app
        .service()
        .ds()
        .get_user_by_clerk_id(&claims.sub)
        .await
        .map(|id| id.ok_or(ApiError(ErrType::Unauthorized.msg("User not found"), req_id.clone())))
        .map_err(|err| ApiError(err, req_id.clone()))??;

    if !user.allowed {
        return Err(ApiError(ErrType::Unauthorized.msg("Not allowed"), req_id));
    }

    let user_id = UserId(user.id);

    req.extensions_mut().insert(user_id);

    Ok(next.run(req).await)
}

pub async fn authenticate_sync(
    headers: HeaderMap,
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = extract_bearer(&headers).map_err(|err| ApiError(err, req_id.clone()))?;

    let claims = app.auth().validate_token_for_claims(token).map_err(|err| ApiError(err, req_id.clone()))?;
    req.extensions_mut().insert(Claims(claims));

    Ok(next.run(req).await)
}
