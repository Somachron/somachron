use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, Router},
    Extension,
};
use lib_core::{ApiError, ApiResult, EmptyResponse, Json, ReqId};
use lib_domain::{
    datastore::space::Folder,
    dto::cloud::{
        req::{CreateFolderRequest, InitiateUploadRequest, UploadCompleteRequest},
        res::{FileMetaResponse, InitiateUploadResponse, StreamedUrlsResponse, _FileMetaResponseVec},
    },
    extension::{SpaceCtx, UserId},
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/ls/{hash}", get(list_files))
        .route("/lf", get(list_folders))
        .route("/rm/{hash}", delete(delete_folder))
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
    Path(folder_hash): Path<String>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .delete_folder(space_ctx, app.storage(), folder_hash)
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
    Path(file_id): Path<String>,
) -> ApiResult<EmptyResponse> {
    app.service()
        .delete_file(space_ctx, app.storage(), file_id)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "File deleted")))
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/l/{hash}",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_files(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(folder_hash): Path<String>,
) -> ApiResult<_FileMetaResponseVec> {
    app.service().list_files(space_ctx, folder_hash).await.map(Json).map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/lf/{hash}",
    responses((status=200, body=Vec<FileMetaResponse>)),
    tag = "Cloud"
)]
pub async fn list_folders(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
) -> ApiResult<Folder> {
    app.service().list_folders(space_ctx).await.map(Json).map_err(|err| ApiError(err, req_id))
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
        .initiate_upload(space_ctx, app.storage(), body.folder_hash, body.file_name)
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
    Path(file_id): Path<String>,
) -> ApiResult<StreamedUrlsResponse> {
    app.service()
        .generate_download_signed_url(app.storage(), file_id)
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
        .create_folder(space_ctx, app.storage(), body.parent_folder_hash, body.folder_name)
        .await
        .map(|_| Json(EmptyResponse::new(StatusCode::OK, "Folder created")))
        .map_err(|err| ApiError(err, req_id))
}
