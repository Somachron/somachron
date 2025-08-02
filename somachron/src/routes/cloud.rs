use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, EmptyResponse, ErrType, Json, ReqId};
use lib_domain::{
    dto::cloud::{
        req::{CreateFolderRequest, SignedUrlRequest, UploadCompleteRequest},
        res::{FileEntryResponse, SignedUrlResponse},
    },
    extension::{IdStr, SpaceCtx, UserId},
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/{*dir}", get(list_directory))
        .route("/{*dir}", delete(delete_path))
        .route("/f/{*path}", get(get_file))
        .route("/d", post(create_folder))
        .route("/stream", post(generate_download_signed_url))
        .route("/upload", post(generate_upload_signed_url))
        .route("/upload/complete", post(upload_completion))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::space::validate_user_space))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/media", routes)
}

#[utoipa::path(
    delete,
    path = "/v1/media/{dir}",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn delete_path(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(dir): Path<String>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .delete_path(space_ctx, app.storage(), dir)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Path deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/{dir}",
    responses((status=200, body=Vec<FileEntryResponse>)),
    tag = "Cloud"
)]
pub async fn list_directory(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(dir): Path<String>,
) -> ApiResult<Vec<FileEntryResponse>> {
    app.service().list_dir(space_ctx, app.storage(), dir).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(get, path = "/v1/media/f/{path}", tag = "Cloud")]
pub async fn get_file(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(path): Path<String>,
) -> axum::response::Result<impl IntoResponse, ApiError> {
    let (buffer, ext) =
        app.storage().get_file(&space_ctx.space_id.id(), &path).await.map_err(|err| ApiError(err, req_id.clone()))?;

    let size = buffer.len();
    let body = Body::from(buffer);

    let response = Response::builder()
        .header(header::CONTENT_TYPE, format!("image/{ext}"))
        .header(header::CONTENT_LENGTH, size)
        .body(body)
        .map_err(|err| ApiError(ErrType::ServerError.err(err, "Failed to create response body"), req_id))?;

    Ok(response)
}

#[utoipa::path(
    post,
    path = "/v1/media/upload",
    responses((status=200, body=SignedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_upload_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<SignedUrlRequest>,
) -> ApiResult<SignedUrlResponse> {
    app.service()
        .generate_upload_signed_url(space_ctx, app.storage(), body.file_path)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/stream",
    responses((status=200, body=SignedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_download_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<SignedUrlRequest>,
) -> ApiResult<SignedUrlResponse> {
    app.storage()
        .generate_download_signed_url(&space_ctx.space_id.id(), &body.file_path)
        .await
        .map(|url| {
            Json(SignedUrlResponse {
                url,
            })
        })
        .map_err(|err| ApiError(err, req_id))
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
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<UploadCompleteRequest>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .process_upload_completion(user_id, space_ctx, app.storage(), body)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Processing completion")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/d",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn create_folder(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<CreateFolderRequest>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .create_folder(space_ctx, app.storage(), body.folder_path)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Folder created")))
        .map_err(|err| ApiError(err, req_id))
}
