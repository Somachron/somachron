use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{AppResult, ErrType};

const THUMBNAIL_EXE: &str = "thumbnailer";

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Clone)]
pub struct MediaDatetime(pub DateTime<Utc>);
impl<'de> Deserialize<'de> for MediaDatetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        DateTime::<Utc>::from_str(&s)
            .or_else(|_| {
                let naive_dt =
                    NaiveDateTime::parse_from_str(&s, "%Y:%m:%d %H:%M:%S").map_err(serde::de::Error::custom)?;
                Ok::<_, D::Error>(DateTime::from_naive_utc_and_offset(naive_dt, Utc))
            })
            .map(MediaDatetime)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaOrientation {
    None = 1,       // Normal
    R90CW = 2,      // Rotate 90° CW
    R180 = 3,       // Rotate 180°
    R270CW = 4,     // Rotate 270° CW (90° CCW)
    FlipH = 5,      // Mirror horizontal
    FlipV = 6,      // Mirror vertical (flip vertical)
    Transpose = 7,  // Mirror horizontal + Rotate 270° CW
    Transverse = 8, // Mirror horizontal + Rotate 90° CW
}
impl MediaOrientation {
    pub fn get_value(self) -> u64 {
        self as u64
    }
    pub fn from_rotation(rotation: u64) -> Self {
        match rotation {
            90 => Self::R90CW,
            180 => Self::R180,
            270 => Self::R270CW,
            _ => Self::None,
        }
    }
}
impl<'de> Deserialize<'de> for MediaOrientation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().trim() {
            // Standard rotation formats
            "none" | "0" | "rotate 0" => Ok(MediaOrientation::None),
            "rotate 90 cw" | "90 cw" | "90" | "rotate90cw" => Ok(MediaOrientation::R90CW),
            "rotate 180" | "180" | "rotate180" => Ok(MediaOrientation::R180),
            "rotate 270 cw" | "rotate 90 ccw" | "270 cw" | "90 ccw" | "270" => Ok(MediaOrientation::R270CW),

            // EXIF orientation formats
            "horizontal (normal)" | "normal" | "1" => Ok(MediaOrientation::None),
            "mirror horizontal" | "flip horizontal" | "2" => Ok(MediaOrientation::FlipH),
            "3" => Ok(MediaOrientation::R180),
            "mirror vertical" | "flip vertical" | "4" => Ok(MediaOrientation::FlipV),
            "mirror horizontal and rotate 270 cw" | "transpose" | "5" => Ok(MediaOrientation::Transpose),
            "6" => Ok(MediaOrientation::R90CW),
            "mirror horizontal and rotate 90 cw" | "transverse" | "7" => Ok(MediaOrientation::Transverse),
            "8" => Ok(MediaOrientation::R270CW),

            _ => Err(serde::de::Error::custom(format!("Invalid rotation value: {s}"))),
        }
    }
}

#[derive(Default, Deserialize, Clone)]
pub struct MediaMetadata {
    #[serde(rename = "Make")]
    pub make: Option<String>,
    #[serde(rename = "Model")]
    pub model: Option<String>,
    #[serde(rename = "Software")]
    pub software: Option<EitherValue<String, f64>>,

    #[serde(rename = "ImageHeight")]
    pub image_height: usize,
    #[serde(rename = "ImageWidth")]
    pub image_width: usize,

    #[serde(rename = "Duration")]
    pub duration: Option<String>,
    #[serde(rename = "MediaDuration")]
    pub media_duration: Option<String>,
    #[serde(rename = "VideoFrameRate")]
    pub frame_rate: Option<f64>,

    #[serde(rename = "DateTimeOriginal")]
    pub date_time: Option<MediaDatetime>,
    #[serde(rename = "Orientation")]
    pub orientation: Option<MediaOrientation>,
    #[serde(rename = "Rotation")]
    pub rotation: Option<EitherValue<MediaOrientation, u64>>,

    #[serde(rename = "ISO")]
    pub iso: Option<usize>,
    #[serde(rename = "ShutterSpeed")]
    pub shutter_speed: Option<EitherValue<String, f64>>,
    #[serde(rename = "Aperture")]
    pub aperture: Option<f64>,
    #[serde(rename = "FNumber")]
    pub f_number: Option<f64>,
    #[serde(rename = "ExposureTime")]
    pub exposure_time: Option<EitherValue<String, f64>>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub struct HeifPath {
    pub tmp_path: String,
    pub thumbnail_path: PathBuf,
}

#[derive(Deserialize)]
struct ThumbnailOut {
    heif_paths: Option<Vec<String>>,
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
pub(super) async fn extract_metadata(tmp_path: &Path) -> AppResult<MediaMetadata> {
    let output = tokio::process::Command::new("exiftool")
        .args(["-j", tmp_path.to_str().unwrap()])
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|err| ErrType::MediaError.err(err, "Failed to get exif data"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrType::MediaError.msg(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data = stdout.into_owned();

    let result: serde_json::Value =
        serde_json::from_str(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize metadata"))?;

    let data = if let Some(arr) = result.as_array() {
        arr.iter().next().cloned().unwrap_or(serde_json::Value::Null)
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

    coordinates.map(|(lat, lng)| (parse_dms_decimal(lat), parse_dms_decimal(lng)))
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
    src: &Path,
    media_type: infer::MatcherType,
    metadata: &MediaMetadata,
) -> AppResult<Option<Vec<String>>> {
    let mode = match media_type {
        infer::MatcherType::Image => "image",
        infer::MatcherType::Video => "video",
        _ => "",
    };

    let rotation = metadata
        .orientation
        .map(|o| o.get_value())
        .or_else(|| {
            metadata.rotation.as_ref().map(|v| match v {
                EitherValue::Either(m) => m.get_value(),
                EitherValue::Or(i) => MediaOrientation::from_rotation(*i).get_value(),
            })
        })
        .unwrap_or(0);

    let mut command = tokio::process::Command::new(THUMBNAIL_EXE);
    let output = command
        .args(["-m", mode, "-r", &rotation.to_string(), src.to_str().unwrap()])
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|err| ErrType::MediaError.err(err, "Failed to spawn command"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrType::MediaError.msg(stderr.into_owned()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.into_owned();

    let value: ThumbnailOut = serde_json::from_str(&stdout)
        .map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize heif paths"))?;

    Ok(value.heif_paths)
}
