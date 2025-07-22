use std::path::Path;

use crate::{AppResult, ErrType};

/// Get media type [`infer::MatcherType::Image`] or [`infer::MatcherType::Video`]
/// based on `ext` extension
pub(super) fn get_media_type(ext: &str) -> infer::MatcherType {
    match ext {
        // images
        "jpg" | "jpeg" | "JPG" | "JPEG" => infer::MatcherType::Image,
        "png" | "PNG" => infer::MatcherType::Image,
        "gif" | "GIF" => infer::MatcherType::Image,
        "bmp" | "BMP" => infer::MatcherType::Image,
        "heif" | "HEIF" => infer::MatcherType::Image,
        "heic" | "HEIC" => infer::MatcherType::Image,
        "avif" | "AVIF" => infer::MatcherType::Image,

        // videos
        "mp4" | "MP4" => infer::MatcherType::Video,
        "m4v" | "M4V" => infer::MatcherType::Video,
        "mkv" | "MKV" => infer::MatcherType::Video,
        "mov" | "MOV" => infer::MatcherType::Video,
        "avi" | "AVI" => infer::MatcherType::Video,
        "mpg" | "MPG" | "mpeg" | "MPEG" => infer::MatcherType::Video,

        // unknown
        _ => infer::MatcherType::Custom,
    }
}

/// Extract metadata from image path
pub(super) fn extract_metadata(tmp_path: impl AsRef<Path>) -> AppResult<serde_json::Value> {
    let mut tool = exiftool::ExifTool::new().map_err(|err| ErrType::MediaError.err(err, "Failed to init exif tool"))?;

    let mut result = tool
        .json(tmp_path.as_ref(), &[])
        .map_err(|err| ErrType::MediaError.err(err, "Failed to extract metadata data"))?;

    if let Some(value) = result.get_mut("SourceFile") {
        *value = serde_json::Value::String("".into());
    }
    if let Some(value) = result.get_mut("Directory") {
        *value = serde_json::Value::String("".into());
    }

    Ok(result)
}
