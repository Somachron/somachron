use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, EmptyResponse, Json, ReqId};
use lib_domain::{
    dto::cloud::{
        req::{CreateFolderRequest, InitiateUploadRequest, UploadCompleteRequest},
        res::{
            FileMetaResponse, FolderResponse, InitiateUploadResponse, StreamedUrlsResponse, _FileMetaResponseVec,
            _FolderResponseVec,
        },
    },
    extension::{SpaceCtx, UserId},
};
use uuid::Uuid;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/ls/{id}", get(list_files))
        .route("/lf/{id}", get(list_folders))
        .route("/rm/{id}", delete(delete_folder))
        .route("/rmf/{id}", delete(delete_file))
        .route("/mkdir", post(create_folder))
        .route("/stream/{id}", get(generate_download_signed_url))
        .route("/upload", post(initiate_upload))
        .route("/upload/complete", post(upload_completion))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::space::validate_user_space))
        .layer(axum::middleware::from_fn_with_state(app, middleware::auth::authenticate));

    router.nest("/media", routes)
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
        .initiate_upload(space_ctx, app.storage(), body.folder_id.0, body.file_name)
        .await
        .map(Json)
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/stream/{id}",
    responses((status=200, body=StreamedUrlsResponse)),
    tag = "Cloud"
)]
pub async fn generate_download_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StreamedUrlsResponse> {
    app.service()
        .generate_download_signed_url(space_ctx, app.storage(), file_id)
        .await
        .map(Json)
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
        .create_folder(space_ctx, body.parent_folder_id.0, body.folder_name)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Folder created")))
        .map_err(|err| ApiError(err, req_id))
}
