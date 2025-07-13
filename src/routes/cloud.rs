use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{post, Router},
    Extension,
};
use lib_core::{
    extensions::{ReqId, UserId},
    ApiError, ApiResult, EmptyResponse, Json,
};
use lib_domain::dto::cloud::{
    req::{UploadCompleteRequest, UploadSignedUrlRequest},
    res::UploadSignedUrlResponse,
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/upload", post(generate_upload_signed_url))
        .route("/upload/complete", post(upload_completion))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/media", routes)
}

#[utoipa::path(
    post,
    path = "/v1/media/upload",
    responses((status=200, body=UploadSignedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_upload_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Json(body): Json<UploadSignedUrlRequest>,
) -> ApiResult<UploadSignedUrlResponse> {
    let url = app
        .storage()
        .generate_upload_signed_url(&user_id.0, &body.file_path.to_lowercase())
        .await
        .map_err(|err| ApiError(err, req_id))?;

    Ok(Json(UploadSignedUrlResponse {
        url,
    }))
}

#[utoipa::path(
    post,
    path = "/v1/media/upload/complete",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn upload_completion(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Json(body): Json<UploadCompleteRequest>,
) -> ApiResult<EmptyResponse> {
    tokio::runtime::Handle::current().spawn(async move {
        if let Err(err) =
            app.storage().process_upload_skeleton_thumbnail_media(&user_id.0, body.file_path, body.file_size).await
        {
            let _ = ApiError(err, req_id).into_response();
        }
    });

    Ok(Json(EmptyResponse::new(StatusCode::OK, "Processing completion")))
}
