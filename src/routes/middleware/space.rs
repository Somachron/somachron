use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
    Extension,
};
use lib_core::{
    extensions::{ReqId, SpaceCtx, UserId},
    ApiError, AppError, ErrType,
};

use crate::app::AppState;

pub async fn validate_user_space(
    headers: HeaderMap,
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let space_id = headers
        .get(super::X_SPACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .ok_or(ApiError(AppError::new(ErrType::BadRequest, "Missing space ID"), req_id.clone()))?;

    let user_space = app
        .service()
        .ds()
        .get_user_space(&user_id.0, space_id)
        .await
        .map_err(|err| ApiError(err, req_id.clone()))?
        .ok_or(ApiError(AppError::new(ErrType::Unauthorized, "User not member of space"), req_id))?;

    let space_ctx = SpaceCtx {
        id: user_space.id.into(),
        role: user_space.role.into(),
    };

    req.extensions_mut().insert(space_ctx);

    Ok(next.run(req).await)
}
