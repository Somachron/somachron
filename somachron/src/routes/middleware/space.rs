use std::str::FromStr;

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
    Extension,
};
use lib_core::{ApiError, ErrType, ReqId};
use lib_domain::{
    datastore::user_space::UserSpaceDs,
    extension::{SpaceCtx, UserId},
};
use uuid::Uuid;

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
        .ok_or(ApiError(ErrType::BadRequest.msg("Missing space ID"), req_id.clone()))?;

    let space_id = Uuid::from_str(space_id)
        .map_err(|err| ApiError(ErrType::BadRequest.err(err, "Invalid space id format"), req_id.clone()))?;

    let space_member = app
        .service()
        .ds()
        .get_user_space(&user_id.0, &space_id)
        .await
        .map_err(|err| ApiError(err, req_id.clone()))?
        .ok_or(ApiError(ErrType::Unauthorized.msg("User not member of space"), req_id))?;

    let space_ctx = SpaceCtx {
        membership_id: space_member.id,
        space_id: space_member.space_id,
        role: space_member.role,
    };

    req.extensions_mut().insert(space_ctx);

    Ok(next.run(req).await)
}
