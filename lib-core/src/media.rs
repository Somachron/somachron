use std::path::PathBuf;

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
pub(super) async fn extract_metadata(tmp_path: &PathBuf) -> AppResult<serde_json::Value> {
    let output = tokio::process::Command::new("exiftool")
        .args(&["-j", tmp_path.to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|err| ErrType::MediaError.err(err, "Failed to get exif data"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrType::MediaError.new(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data = stdout.into_owned();

    let mut result: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize metadata"))?;

    if let Some(value) = result.get_mut("SourceFile") {
        *value = serde_json::Value::String("".into());
    }
    if let Some(value) = result.get_mut("Directory") {
        *value = serde_json::Value::String("".into());
    }

    Ok(result)
}
