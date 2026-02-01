// use std::path::PathBuf;
// use std::str::FromStr;
//
// use chrono::{DateTime, NaiveDateTime, Utc};
// use serde::de::DeserializeOwned;
// use serde::{Deserialize, Serialize};
// use smq_dto::res::ProcessedImage;
// use utoipa::ToSchema;
//
// use super::{AppResult, ErrType};
//
// const THUMBNAIL_EXE: &str = "thumbnailer";
// const EXIFTOOL_EXE: &str = "exiftool";

// #[derive(Debug, Deserialize, Clone)]
// #[serde(untagged)]
// pub enum EitherValue<A, B> {
//     Either(A),
//     Or(B),
// }
// impl<A, B> Default for EitherValue<A, B>
// where
//     A: Serialize + DeserializeOwned + ToSchema,
//     B: Serialize + DeserializeOwned + Default + ToSchema,
// {
//     fn default() -> Self {
//         Self::Or(B::default())
//     }
// }
//
// #[derive(Debug, Clone)]
// pub struct MediaDatetime(pub DateTime<Utc>);
// impl<'de> Deserialize<'de> for MediaDatetime {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let s = String::deserialize(deserializer)?;
//
//         DateTime::<Utc>::from_str(&s)
//             .or_else(|_| {
//                 let dt = s.split('+').next().unwrap_or(&s);
//
//                 let naive_dt =
//                     NaiveDateTime::parse_from_str(dt, "%Y:%m:%d %H:%M:%S").map_err(serde::de::Error::custom)?;
//                 Ok::<_, D::Error>(DateTime::from_naive_utc_and_offset(naive_dt, Utc))
//             })
//             .map(MediaDatetime)
//     }
// }
//
// #[derive(Debug, Clone, Copy)]
// pub enum MediaOrientation {
//     None = 1,       // Normal
//     R90CW = 2,      // Rotate 90° CW
//     R180 = 3,       // Rotate 180°
//     R270CW = 4,     // Rotate 270° CW (90° CCW)
//     FlipH = 5,      // Mirror horizontal
//     FlipV = 6,      // Mirror vertical (flip vertical)
//     Transpose = 7,  // Mirror horizontal + Rotate 270° CW
//     Transverse = 8, // Mirror horizontal + Rotate 90° CW
// }
// impl MediaOrientation {
//     pub fn get_value(self) -> u64 {
//         self as u64
//     }
//     pub fn from_rotation(rotation: u64) -> Self {
//         match rotation {
//             90 => Self::R90CW,
//             180 => Self::R180,
//             270 => Self::R270CW,
//             _ => Self::None,
//         }
//     }
// }
// impl<'de> Deserialize<'de> for MediaOrientation {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let s = String::deserialize(deserializer)?;
//         match s.to_lowercase().trim() {
//             // Standard rotation formats
//             "none" | "0" | "rotate 0" => Ok(MediaOrientation::None),
//             "rotate 90 cw" | "90 cw" | "90" | "rotate90cw" => Ok(MediaOrientation::R90CW),
//             "rotate 180" | "180" | "rotate180" => Ok(MediaOrientation::R180),
//             "rotate 270 cw" | "rotate 90 ccw" | "270 cw" | "90 ccw" | "270" => Ok(MediaOrientation::R270CW),
//
//             // EXIF orientation formats
//             "horizontal (normal)" | "normal" | "1" => Ok(MediaOrientation::None),
//             "mirror horizontal" | "flip horizontal" | "2" => Ok(MediaOrientation::FlipH),
//             "3" => Ok(MediaOrientation::R180),
//             "mirror vertical" | "flip vertical" | "4" => Ok(MediaOrientation::FlipV),
//             "mirror horizontal and rotate 270 cw" | "transpose" | "5" => Ok(MediaOrientation::Transpose),
//             "6" => Ok(MediaOrientation::R90CW),
//             "mirror horizontal and rotate 90 cw" | "transverse" | "7" => Ok(MediaOrientation::Transverse),
//             "8" => Ok(MediaOrientation::R270CW),
//
//             _ => Err(serde::de::Error::custom(format!("Invalid rotation value: {s}"))),
//         }
//     }
// }
//
// #[derive(Default, Deserialize, Clone)]
// pub struct MediaMetadata {
//     #[serde(rename = "Make")]
//     pub make: Option<String>,
//     #[serde(rename = "Model")]
//     pub model: Option<String>,
//     #[serde(rename = "Software")]
//     pub software: Option<EitherValue<String, f64>>,
//
//     #[serde(rename = "ImageHeight")]
//     pub image_height: usize,
//     #[serde(rename = "ImageWidth")]
//     pub image_width: usize,
//
//     #[serde(rename = "Duration")]
//     pub duration: Option<String>,
//     #[serde(rename = "MediaDuration")]
//     pub media_duration: Option<String>,
//     #[serde(rename = "VideoFrameRate")]
//     pub frame_rate: Option<f64>,
//
//     #[serde(rename = "DateTimeOriginal")]
//     pub date_time: Option<MediaDatetime>,
//     #[serde(rename = "Orientation")]
//     pub orientation: Option<MediaOrientation>,
//     #[serde(rename = "Rotation")]
//     pub rotation: Option<EitherValue<MediaOrientation, u64>>,
//
//     #[serde(rename = "ISO")]
//     pub iso: Option<usize>,
//     #[serde(rename = "ShutterSpeed")]
//     pub shutter_speed: Option<EitherValue<String, f64>>,
//     #[serde(rename = "Aperture")]
//     pub aperture: Option<f64>,
//     #[serde(rename = "FNumber")]
//     pub f_number: Option<f64>,
//     #[serde(rename = "ExposureTime")]
//     pub exposure_time: Option<EitherValue<String, f64>>,
//
//     pub latitude: Option<f64>,
//     pub longitude: Option<f64>,
// }
//
// #[derive(Serialize, Deserialize, ToSchema)]
// #[serde(rename_all = "snake_case")]
// pub enum MediaType {
//     Image,
//     Video,
// }

// pub enum MediaProcessType {
//     Image {
//         path: PathBuf,
//         file_size: i64,
//     },
//     Video {
//         url: String,
//         name: String,
//         tmp_path: PathBuf,
//         file_size: i64,
//     },
// }

// Get media type [`infer::MatcherType::Image`] or [`infer::MatcherType::Video`]
// based on `ext` extension
// pub fn get_media_type(ext: &str) -> AppResult<MediaType> {
//     match ext {
//         // images
//         "jpg" | "jpeg" | "JPG" | "JPEG" => Ok(MediaType::Image),
//         "png" | "PNG" => Ok(MediaType::Image),
//         "gif" | "GIF" => Ok(MediaType::Image),
//         "bmp" | "BMP" => Ok(MediaType::Image),
//         "heif" | "HEIF" => Ok(MediaType::Image),
//         "heic" | "HEIC" => Ok(MediaType::Image),
//         "avif" | "AVIF" => Ok(MediaType::Image),
//
//         // videos
//         "mp4" | "MP4" => Ok(MediaType::Video),
//         "m4v" | "M4V" => Ok(MediaType::Video),
//         "mkv" | "MKV" => Ok(MediaType::Video),
//         "mov" | "MOV" => Ok(MediaType::Video),
//         "avi" | "AVI" => Ok(MediaType::Video),
//         "hevc" | "HEVC" => Ok(MediaType::Video),
//         "mpg" | "MPG" | "mpeg" | "MPEG" => Ok(MediaType::Video),
//
//         // unknown
//         ext => Err(ErrType::MediaError.msg(format!("Invalid media format: {ext}"))),
//     }
// }

// #[derive(Debug, Default, Serialize, Deserialize)]
// pub struct ImageMeta {
//     pub width: i32,
//     pub height: i32,
//     pub file_name: String,
// }

// pub struct ProcessedMeta {
//     pub thumbnail: ImageMeta,
//     pub preview: ImageMeta,
//     pub file_name: String,
// }

// Spawn thumbnailer binary
// pub(super) async fn run_thumbnailer(
//     process_type: &MediaProcessType,
//     metadata: &MediaMetadata,
// ) -> AppResult<ProcessedImage> {
//     let rotation = metadata
//         .orientation
//         .map(|o| o.get_value())
//         .or_else(|| {
//             metadata.rotation.as_ref().map(|v| match v {
//                 EitherValue::Either(m) => m.get_value(),
//                 EitherValue::Or(i) => MediaOrientation::from_rotation(*i).get_value(),
//             })
//         })
//         .unwrap_or(0)
//         .to_string();

//     let args: &[&str] = match process_type {
//         MediaProcessType::Image {
//             path,
//             ..
//         } => &["-r", rotation.as_str(), "image", path.to_str().unwrap()],
//         MediaProcessType::Video {
//             url,
//             name,
//             ..
//         } => &["-r", rotation.as_str(), "video", url, name.as_str()],
//     };

//     let output = tokio::process::Command::new(THUMBNAIL_EXE)
//         .args(args)
//         .kill_on_drop(true)
//         .stdout(std::process::Stdio::piped())
//         .stderr(std::process::Stdio::piped())
//         .output()
//         .await
//         .map_err(|err| ErrType::MediaError.err(err, "Failed to spawn command"))?;

//     if !output.status.success() {
//         let stderr = String::from_utf8_lossy(&output.stderr);
//         return Err(ErrType::MediaError.msg(stderr.into_owned()));
//     }

//     let stdout = String::from_utf8_lossy(&output.stdout);
//     let stdout = stdout.into_owned();

//     let value: ProcessedImage = serde_json::from_str(&stdout)
//         .map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize heif paths"))?;

//     Ok(value)
// }
