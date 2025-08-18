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
        req::{CreateFolderRequest, SignedUrlRequest, UploadCompleteRequest},
        res::{FileMetaResponse, SignedUrlResponse, _FileMetaResponseVec},
    },
    extension::{SpaceCtx, UserId},
};

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let routes = Router::new()
        .route("/l/{hash}", get(list_files))
        .route("/ld", get(list_folders))
        .route("/p/{hash}", delete(delete_folder))
        .route("/f/{id}", delete(delete_file))
        .route("/d", post(create_folder))
        .route("/stream/{id}", get(generate_download_signed_url))
        .route("/stream/th/{id}", get(generate_thumbnail_download_signed_url))
        .route("/upload", post(generate_upload_signed_url))
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
    path = "/v1/media/ld",
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
    get,
    path = "/v1/media/stream/{id}",
    responses((status=200, body=SignedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_download_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Path(file_id): Path<String>,
) -> ApiResult<SignedUrlResponse> {
    app.service()
        .generate_download_signed_url(app.storage(), file_id, false)
        .await
        .map(|url| {
            Json(SignedUrlResponse {
                url,
            })
        })
        .map_err(|err| ApiError(err, req_id))
}

#[utoipa::path(
    get,
    path = "/v1/media/stream/th/{id}",
    responses((status=200, body=SignedUrlResponse)),
    tag = "Cloud"
)]
pub async fn generate_thumbnail_download_signed_url(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Path(file_id): Path<String>,
) -> ApiResult<SignedUrlResponse> {
    app.service()
        .generate_download_signed_url(app.storage(), file_id, true)
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
