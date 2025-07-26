use std::path::PathBuf;

use crate::{AppResult, ErrType};

const THUMBNAIL_EXE: &str = "thumbnailer";

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
// pub(super) async fn extract_metadata(tmp_path: &PathBuf) -> AppResult<sonic_rs::Value> {
//     let output = tokio::process::Command::new("exiftool")
//         .args(&["-j", tmp_path.to_str().unwrap()])
//         .kill_on_drop(true)
//         .stdout(std::process::Stdio::piped())
//         .stderr(std::process::Stdio::piped())
//         .output()
//         .await
//         .map_err(|err| ErrType::MediaError.err(err, "Failed to get exif data"))?;

//     if !output.status.success() {
//         let stderr = String::from_utf8_lossy(&output.stderr);
//         return Err(ErrType::MediaError.new(stderr));
//     }

//     let stdout = String::from_utf8_lossy(&output.stdout);
//     let data = stdout.into_owned();

//     let result: sonic_rs::Value =
//         sonic_rs::from_str(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize metadata"))?;

//     let mut data = if result.is_array() {
//         let arr = result.into_array().unwrap();
//         arr.into_iter().nth(0).unwrap_or(sonic_rs::Value::default())
//     } else {
//         result
//     };

//     if let Some(value) = data.get_mut("SourceFile") {
//         *value = sonic_rs::Value::from_static_str("");
//     }
//     if let Some(value) = data.get_mut("Directory") {
//         *value = sonic_rs::Value::from_static_str("");
//     }

//     Ok(data)
// }

/// Spawn thumbnailer binary
pub(super) async fn run_thumbnailer(
    src: &PathBuf,
    dst: &PathBuf,
    media_type: infer::MatcherType,
    file_path: &str,
    r2_path: &str,
    metadata_path: PathBuf,
    thumbnail_filename: PathBuf,
    user_id: &str,
) -> AppResult<bool> {
    let mode = match media_type {
        infer::MatcherType::Image => "image",
        infer::MatcherType::Video => "video",
        _ => "",
    };

    let mut command = tokio::process::Command::new(THUMBNAIL_EXE);
    let output = command
        .args(&["-m", mode])
        .args(&[
            src.to_str().unwrap(),
            dst.to_str().unwrap(),
            file_path,
            r2_path,
            metadata_path.to_str().unwrap(),
            thumbnail_filename.to_str().unwrap(),
            user_id,
        ])
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|err| ErrType::MediaError.err(err, "Failed to spawn command"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrType::MediaError.new(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.into_owned();
    match stdout.trim() {
        "true" => Ok(true),
        _ => Ok(false),
    }
}
