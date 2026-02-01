use std::str::FromStr;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, Router},
    Extension,
};
use lib_core::{smq_dto::res::MediaData, ApiError, ApiResult, EmptyResponse, ErrType, Json, ReqId, X_SPACE_HEADER};
use lib_domain::{
    dto::cloud::{
        req::{CreateFolderRequest, InitiateUploadRequest, QueueMediaProcessRequest},
        res::{
            DownloadUrlResponse, FileMetaResponse, FolderResponse, InitiateUploadResponse, StreamedUrlResponse,
            _FileMetaResponseVec, _FolderResponse, _FolderResponseVec,
        },
    },
    extension::{SpaceCtx, UserId},
};
use uuid::Uuid;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/lg", get(list_files_gallery))
        .route("/ls/{id}", get(list_files))
        .route("/lf/{id}", get(list_folders))
        .route("/d/{id}", get(get_folder))
        .route("/rm/{id}", delete(delete_folder))
        .route("/rmf/{id}", delete(delete_file))
        .route("/mkdir", post(create_folder))
        .route("/stream/{id}", get(generate_thumbnail_preview_signed_urls))
        .route("/download/{id}", get(generate_download_signed_url))
        .route("/upload", post(initiate_upload))
        .route("/queue", post(media_queue))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::space::validate_user_space))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::auth::authenticate));

    let interconnect_routes = Router::new()
        .route("/queue/complete", post(complete_media_queue))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate_interconnect));

    router.nest("/media", routes).nest("/media", interconnect_routes)
}

#[utoipa::path(
    delete,
    path = "/v1/media/p/{dir}",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn delete_folder(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(folder_id): Path<Uuid>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .delete_folder(space_ctx, app.storage(), folder_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Path deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    delete,
    path = "/v1/media/f/{id}",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn delete_file(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .delete_file(space_ctx, app.storage(), file_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "File deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/l/{id}",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_files(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(folder_id): Path<Uuid>,
) -> ApiResult<_FileMetaResponseVec> {
    app.service().list_files(space_ctx, folder_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/lg",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_files_gallery(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<_FileMetaResponseVec> {
    app.service().list_files_gallery(space_ctx).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/lf/{id}",
    responses((status=200, body=Vec<FolderResponse>)),
    tag = "Cloud"
)]
pub async fn list_folders(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(folder_id): Path<Uuid>,
) -> ApiResult<_FolderResponseVec> {
    app.service().list_folders(space_ctx, folder_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/d/{id}",
    responses((status=200, body=Vec<FolderResponse>)),
    tag = "Cloud"
)]
pub async fn get_folder(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(folder_id): Path<Uuid>,
) -> ApiResult<_FolderResponse> {
    app.service().get_folder(space_ctx, folder_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/upload",
    responses((status=200, body=InitiateUploadResponse)),
    tag = "Cloud"
)]
pub async fn initiate_upload(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<InitiateUploadRequest>,
) -> ApiResult<InitiateUploadResponse> {
    app.service()
        .initiate_upload(space_ctx, app.storage(), body.folder_id, body.file_name)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/stream/{id}",
    responses((status=200, body=StreamedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_thumbnail_preview_signed_urls(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StreamedUrlResponse> {
    app.service()
        .generate_thumbnail_preview_signed_urls(space_ctx, app.storage(), file_id)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/download/{id}",
    responses((status=200, body=DownloadUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_download_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<DownloadUrlResponse> {
    app.service()
        .generate_download_signed_url(space_ctx, app.storage(), file_id)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/queue",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn media_queue(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<QueueMediaProcessRequest>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .queue_media_process(user_id, space_ctx, app.storage(), app.interconnect(), body)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Media queued")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/queue/complete",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn complete_media_queue(
    headers: HeaderMap,
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Json(body): Json<MediaData>,
) -> ApiResult<EmptyResponse> {
    let space_id = headers
        .get(X_SPACE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .ok_or(ApiError(ErrType::BadRequest.msg("Missing space ID"), req_id.clone()))?;

    let space_id = Uuid::from_str(space_id)
        .map_err(|err| ApiError(ErrType::BadRequest.err(err, "Invalid space id format"), req_id.clone()))?;

    app.service()
        .complete_media_queue(space_id, body)
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
        .create_folder(space_ctx, body.parent_folder_id, body.folder_name)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Folder created")))
        .map_err(|err| ApiError(err, req_id))
}
