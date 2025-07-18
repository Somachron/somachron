use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post, Router},
    Extension,
};
use lib_core::{
    extensions::{ReqId, UserId},
    storage::FileEntry,
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
        .route("/{dir}", get(list_directory))
        .route("/f/{*path}", get(get_file))
        .route("/upload", post(generate_upload_signed_url))
        .route("/upload/complete", post(upload_completion))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/media", routes)
}

#[utoipa::path(
    get,
    path = "/v1/media/{dir}",
    responses((status=200, body=Vec<FileEntry>)),
    tag = "Cloud"
)]
pub async fn list_directory(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Path(dir): Path<String>,
) -> ApiResult<Vec<FileEntry>> {
    app.storage().list_dir(&user_id.0, &dir).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(get, path = "/v1/media/f/{path}", tag = "Cloud")]
pub async fn get_file(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Path(path): Path<String>,
) -> axum::response::Result<impl IntoResponse, ApiError> {
    let (stream, ext) = app.storage().get_file(&user_id.0, &path).await.map_err(|err| ApiError(err, req_id))?;

    let body = Body::from_stream(stream);

    Ok(([(header::CONTENT_TYPE, format!("image/{ext}"))], body).into_response())
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
        .generate_upload_signed_url(&user_id.0, &body.file_path)
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
    app.storage()
        .process_upload_skeleton_thumbnail_media(&user_id.0, &body.file_path, body.file_size)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Processing completion")))
        .map_err(|err| ApiError(err, req_id))
}
