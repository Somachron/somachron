use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{AppResult, ErrType};

const THUMBNAIL_EXE: &str = "thumbnailer";

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum EitherValue<A, B> {
    Either(A),
    Or(B),
}
impl<A, B> Default for EitherValue<A, B>
where
    A: Serialize + DeserializeOwned + ToSchema,
    B: Serialize + DeserializeOwned + Default + ToSchema,
{
    fn default() -> Self {
        Self::Or(B::default())
    }
}

#[derive(Default, Serialize, Deserialize, ToSchema)]
pub struct MediaMetadata {
    #[serde(rename = "Make")]
    make: Option<String>,
    #[serde(rename = "Model")]
    model: Option<String>,
    #[serde(rename = "Software")]
    software: Option<EitherValue<String, f64>>,

    #[serde(rename = "ImageHeight")]
    image_height: usize,
    #[serde(rename = "ImageWidth")]
    image_width: usize,

    #[serde(rename = "Duration")]
    duration: Option<String>,
    #[serde(rename = "MediaDuration")]
    media_duration: Option<String>,
    #[serde(rename = "VideoFrameRate")]
    frame_rate: Option<f32>,

    #[serde(rename = "DateTimeOriginal")]
    date_time: Option<String>,
    #[serde(rename = "Orientation")]
    orientation: Option<String>,
    #[serde(rename = "Rotation")]
    rotation: Option<EitherValue<String, u64>>,

    #[serde(rename = "ISO")]
    iso: Option<usize>,
    #[serde(rename = "ShutterSpeed")]
    shutter_speed: Option<String>,
    #[serde(rename = "Aperture")]
    aperture: Option<f32>,
    #[serde(rename = "FNumber")]
    f_number: Option<f32>,
    #[serde(rename = "ExposureTime")]
    exposure_time: Option<EitherValue<String, u32>>,

    latitude: Option<f64>,
    longitude: Option<f64>,
}

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
pub(super) async fn extract_metadata(tmp_path: &PathBuf) -> AppResult<MediaMetadata> {
    let output = tokio::process::Command::new("exiftool")
        .args(&["-j", tmp_path.to_str().unwrap()])
        .kill_on_drop(true)
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

    let result: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize metadata"))?;

    let data = if let Some(arr) = result.as_array() {
        arr.into_iter().nth(0).cloned().unwrap_or(serde_json::Value::Null)
    } else {
        result
    };

    let gps_info = extract_gps_info(&data);

    let mut metadata: MediaMetadata = serde_json::from_value(data).map_err(|err| {
        ErrType::MediaError.err(err, format!("Failed to deserialize media data: {:?}", tmp_path.file_name()))
    })?;

    if let Some((lat, lng)) = gps_info {
        metadata.latitude = Some(lat);
        metadata.longitude = Some(lng);
    }

    Ok(metadata)
}

fn extract_gps_info(data: &serde_json::Value) -> Option<(f64, f64)> {
    let data_coordinates = data.get("GPSCoordinates").or_else(|| data.get("GPSPosition")).and_then(|v| v.as_str());

    let coordinates = data_coordinates.and_then(|v| {
        let mut tokens = v.split(',');
        let lat = tokens.next().map(|s| s.trim());
        let lng = tokens.next().map(|s| s.trim());
        lat.zip(lng)
    });

    let coordinates = coordinates.or_else(|| {
        let lat = data.get("GPSLatitude").and_then(|s| s.as_str());
        let lng = data.get("GPSLongitude").and_then(|s| s.as_str());
        lat.zip(lng)
    });

    coordinates.and_then(|(lat, lng)| Some((parse_dms_decimal(lat), parse_dms_decimal(lng))))
}

fn parse_dms_decimal(dms: &str) -> f64 {
    let tokens: Vec<&str> = dms.split(' ').filter(|s| !s.is_empty() && *s != "deg").collect();
    let degrees: f64 = tokens[0].trim_end_matches('°').parse().unwrap();
    let minutes: f64 = tokens[1].trim_end_matches('\'').parse().unwrap();
    let seconds: f64 = tokens[2].trim_end_matches('\"').parse().unwrap();

    let decimal = degrees + (minutes / 60.0) + (seconds / 3600.0);

    if dms.ends_with('S') || dms.ends_with('W') {
        -decimal
    } else {
        decimal
    }
}

/// Spawn thumbnailer binary
pub(super) async fn run_thumbnailer(
    src: &PathBuf,
    dst: &PathBuf,
    media_type: infer::MatcherType,
    metadata: &MediaMetadata,
) -> AppResult<bool> {
    let mode = match media_type {
        infer::MatcherType::Image => "image",
        infer::MatcherType::Video => "video",
        _ => "",
    };

    let orientation = metadata.orientation.as_ref().and_then(|v| Some(v.parse().unwrap_or(0)));
    let rotation = metadata
        .rotation
        .as_ref()
        .map(|v| match v {
            EitherValue::Either(s) => s.parse().unwrap_or(0),
            EitherValue::Or(i) => *i,
        })
        .unwrap_or(0);

    let mut command = tokio::process::Command::new(THUMBNAIL_EXE);
    let mut command = command.args(&["-m", mode]);

    if let Some(orientation) = orientation {
        command = command.args(&["-o", &orientation.to_string()]);
    }
    let output = command
        .args(&["-r", &rotation.to_string(), src.to_str().unwrap(), dst.to_str().unwrap()])
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
