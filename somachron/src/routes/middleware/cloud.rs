use std::path::PathBuf;

use axum::{extract::Request, middleware::Next, response::Response, Extension};
use lib_core::{extensions::ReqId, ApiError, ErrType};

pub async fn validate_path(
    Extension(req_id): Extension<ReqId>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let uri_path = req.uri().path();
    let path = PathBuf::from(uri_path);

    // reject dirs
    if path.is_dir() {
        return Err(ApiError(ErrType::NotFound.new("Not found"), req_id.clone()));
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(ApiError(ErrType::FsError.new("No file name"), req_id.clone()))?;

    // reject hidden files
    if file_name.starts_with('.') {
        return Err(ApiError(ErrType::NotFound.new("Not found"), req_id.clone()));
    }

    // validate extensions
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(ApiError(ErrType::FsError.new("No exention found"), req_id.clone()))?;
    match ext {
        "jpeg" | "jpg" | "JPEG" | "JPG" => (),
        "png" | "PNG" => (),
        _ => return Err(ApiError(ErrType::NotFound.new("Not found"), req_id)),
    };

    Ok(next.run(req).await)
}
