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
        req::{CreateAlbumRequest, InitiateUploadRequest, QueueMediaProcessRequest, UpdateAlbumFilesRequest},
        res::{
            _AlbumResponse, _AlbumResponseVec, _FileMetaResponseVec, AlbumResponse, DownloadUrlResponse,
            FileMetaResponse, InitiateUploadResponse, StreamedUrlResponse,
        },
    },
    extension::{SpaceCtx, UserId},
    service::media::MediaService,
};
use uuid::Uuid;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/files/gallery", get(list_files_gallery))
        .route("/albums", post(create_album))
        .route("/albums", get(list_albums))
        .route("/albums/{id}", get(get_album))
        .route("/albums/{id}", delete(delete_album))
        .route("/albums/{id}/files", get(list_files))
        .route("/albums/{id}/files/link", post(link_album_files))
        .route("/albums/{id}/files/unlink", post(unlink_album_files))
        .route("/files/{id}", delete(delete_file))
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
    path = "/v1/media/albums/{id}",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn delete_album(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(album_id): Path<Uuid>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .media_service()
        .delete_album(space_ctx, album_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Album deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    delete,
    path = "/v1/media/files/{id}",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn delete_file(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .media_service()
        .delete_file(space_ctx, app.storage(), file_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "File deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/albums/{id}/files",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_files(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(album_id): Path<Uuid>,
) -> ApiResult<_FileMetaResponseVec> {
    app.services().media_service().list_files(space_ctx, album_id).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/files/gallery",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_files_gallery(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<_FileMetaResponseVec> {
    app.services().media_service().list_files_gallery(space_ctx).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/albums",
    responses((status=200, body=Vec<AlbumResponse>)),
    tag = "Cloud"
)]
pub async fn list_albums(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<_AlbumResponseVec> {
    app.services().media_service().list_albums(space_ctx).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/albums/{id}",
    responses((status=200, body=AlbumResponse)),
    tag = "Cloud"
)]
pub async fn get_album(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(album_id): Path<Uuid>,
) -> ApiResult<_AlbumResponse> {
    app.services().media_service().get_album(space_ctx, album_id).await.map(Json).map_err(|err| ApiError(err, req_id))
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
    app.services()
        .media_service()
        .initiate_upload(space_ctx, app.storage(), body)
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
    app.services()
        .media_service()
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
    app.services()
        .media_service()
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
    app.services()
        .media_service()
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

    app.services()
        .media_service()
        .complete_media_queue(space_id, body)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Processing completion")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/albums",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn create_album(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(user_id): Extension<UserId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Json(body): Json<CreateAlbumRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .media_service()
        .create_album(user_id, space_ctx, body.name)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Album created")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/albums/{id}/files/link",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn link_album_files(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(album_id): Path<Uuid>,
    Json(body): Json<UpdateAlbumFilesRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .media_service()
        .link_album_files(space_ctx, album_id, body.file_ids)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Files linked")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    post,
    path = "/v1/media/albums/{id}/files/unlink",
    responses((status=200, body=EmptyResponse)),
    tag = "Cloud"
)]
pub async fn unlink_album_files(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(album_id): Path<Uuid>,
    Json(body): Json<UpdateAlbumFilesRequest>,
) -> ApiResult<EmptyResponse> {
    app.services()
        .media_service()
        .unlink_album_files(space_ctx, album_id, body.file_ids)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Files unlinked")))
        .map_err(|err| ApiError(err, req_id))
}
