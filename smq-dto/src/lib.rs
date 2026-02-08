use std::str::FromStr;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use utoipa::{
    openapi::{schema::SchemaType, KnownFormat, ObjectBuilder, RefOr, Schema, SchemaFormat, Type},
    PartialSchema, ToSchema,
};

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
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

#[derive(Debug, Serialize, Clone)]
pub struct MediaDatetime(pub DateTime<Utc>);
impl ToSchema for MediaDatetime {}

impl PartialSchema for MediaDatetime {
    fn schema() -> RefOr<Schema> {
        RefOr::T(Schema::Object(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(Type::String))
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
                .build(),
        ))
    }
}

impl<'de> Deserialize<'de> for MediaDatetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        DateTime::<Utc>::from_str(&s)
            .or_else(|_| {
                let dt = s.split('+').next().unwrap_or(&s);

                let naive_dt =
                    NaiveDateTime::parse_from_str(dt, "%Y:%m:%d %H:%M:%S").map_err(serde::de::Error::custom)?;
                Ok::<_, D::Error>(DateTime::from_naive_utc_and_offset(naive_dt, Utc))
            })
            .map(MediaDatetime)
    }
}

#[derive(Debug, Serialize, Clone, Copy, ToSchema)]
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
            "r90cw" | "rotate 90 cw" | "90 cw" | "90" | "rotate90cw" => Ok(MediaOrientation::R90CW),
            "r180" | "rotate 180" | "180" | "rotate180" => Ok(MediaOrientation::R180),
            "r270cw" | "rotate 270 cw" | "rotate 90 ccw" | "270 cw" | "90 ccw" | "270" => Ok(MediaOrientation::R270CW),

            // EXIF orientation formats
            "horizontal (normal)" | "normal" | "1" => Ok(MediaOrientation::None),
            "fliph" | "mirror horizontal" | "flip horizontal" | "2" => Ok(MediaOrientation::FlipH),
            "3" => Ok(MediaOrientation::R180),
            "flipv" | "mirror vertical" | "flip vertical" | "4" => Ok(MediaOrientation::FlipV),
            "mirror horizontal and rotate 270 cw" | "transpose" | "5" => Ok(MediaOrientation::Transpose),
            "6" => Ok(MediaOrientation::R90CW),
            "mirror horizontal and rotate 90 cw" | "transverse" | "7" => Ok(MediaOrientation::Transverse),
            "8" => Ok(MediaOrientation::R270CW),

            _ => Err(serde::de::Error::custom(format!("Invalid rotation value: {s}"))),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, ToSchema)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    Video,
}

pub mod res {
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;
    use uuid::Uuid;
    use validator::Validate;

    use crate::{MediaDatetime, MediaMetadata, MediaType};

    #[derive(Debug, Default, Clone, Serialize, Deserialize, ToSchema)]
    pub struct ImageData {
        pub width: i32,
        pub height: i32,
        pub file_name: String,
    }

    #[derive(Debug, Serialize, Deserialize, ToSchema)]
    pub struct ProcessedImage {
        pub thumbnail: ImageData,
        pub preview: ImageData,
        pub file_name: String,
    }

    #[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
    pub struct MediaData {
        pub file_id: Uuid,
        pub folder_id: Uuid,
        pub updated_date: MediaDatetime,
        pub file_data: FileData,
    }

    #[derive(Debug, Serialize, Deserialize, ToSchema)]
    pub struct FileData {
        pub file_name: String,
        pub thumbnail: ImageData,
        pub preview: ImageData,
        pub metadata: MediaMetadata,
        pub size: i64,
        pub media_type: MediaType,
    }
}

pub mod req {
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;
    use uuid::Uuid;
    use validator::Validate;

    use crate::MediaDatetime;

    #[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
    pub struct ProcessMediaRequest {
        pub file_id: Uuid,
        pub updated_date: MediaDatetime,
        pub space_id: Uuid,
        pub folder_id: Uuid,
        pub s3_file_path: String,
    }
}
