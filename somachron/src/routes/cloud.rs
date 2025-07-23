use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, Router},
    Extension,
};
use lib_core::{
    extensions::{ReqId, SpaceCtx, UserId},
    storage::FileEntry,
    ApiError, ApiResult, EmptyResponse, Json,
};
use lib_domain::dto::cloud::{
    req::{CreateFolderRequest, SignedUrlRequest, UploadCompleteRequest},
    res::SignedUrlResponse,
};
use tower_http::services::ServeDir;

use crate::app::AppState;

use super::middleware;

pub fn bind_routes(app: AppState, router: Router<AppState>) -> Router<AppState> {
    let fs_path = app.storage().spaces_path();

    let routes = Router::new()
        .nest_service("/f", ServeDir::new(fs_path))
        .layer(axum::middleware::from_fn_with_state(app.clone(), middleware::cloud::validate_path))
        .route("/{*dir}", get(list_directory))
        .route("/{*dir}", delete(delete_path))
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
    responses((status=200, body=Vec<FileEntry>)),
    tag = "Cloud"
)]
pub async fn list_directory(
    State(app): State<AppState>,
    Extension(req_id): Extension<ReqId>,
    Extension(space_ctx): Extension<SpaceCtx>,
    Path(dir): Path<String>,
) -> ApiResult<Vec<FileEntry>> {
    app.storage().list_dir(&space_ctx.id, &dir).await.map(Json).map_err(|err| ApiError(err, req_id))
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
        .generate_download_signed_url(&space_ctx.id, &body.file_path)
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
        .process_upload_skeleton_thumbnail(user_id, space_ctx, app.storage(), body)
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
