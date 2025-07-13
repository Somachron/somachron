use std::{collections::HashMap, io::Cursor};

use serde::{Deserialize, Serialize};

use crate::{AppError, AppResult, ErrType};

#[derive(Default, Serialize, Deserialize)]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub date_time: Option<String>,
    pub orientation: Option<u32>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub gps_altitude: Option<f64>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<u32>,
    pub exposure_time: Option<String>,
    pub flash: Option<bool>,
    pub white_balance: Option<String>,
    pub lens_make: Option<String>,
    pub lens_model: Option<String>,
    pub software: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub color_space: Option<String>,
    pub custom_fields: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    pub r2_path: Option<String>,
    pub exif: Option<ExifData>,
    pub size: usize,
}

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

pub(super) async fn process_image_thumnail(bytes: Vec<u8>, exif_data: &Option<ExifData>) -> AppResult<Vec<u8>> {
    let kind =
        infer::get(&bytes).ok_or(AppError::new(ErrType::FsError, "Could not detect file type from magic bytes"))?;

    if kind.matcher_type() != infer::MatcherType::Image {
        return Err(AppError::new(
            ErrType::FsError,
            format!("File is not an image, detected as: {} ({})", kind.mime_type(), kind.extension()),
        ));
    }

    let format = infer_to_image_format(&kind)?;

    let img = image::load_from_memory_with_format(&bytes, format)
        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to load image from bytes"))?;

    let img = match exif_data.as_ref().and_then(|d| d.orientation) {
        Some(2) => img.fliph(),
        Some(3) => img.rotate180(),
        Some(4) => img.flipv(),
        Some(5) => img.rotate90().fliph(),
        Some(6) => img.rotate90(),
        Some(7) => img.rotate270().fliph(),
        Some(8) => img.rotate270(),
        _ => img, // No rotation needed for 1 or unknown
    };

    let thumbnail = img.thumbnail(100, 100);

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    let quality = 80;

    match format {
        image::ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            img.write_with_encoder(encoder)
        }
        _ => thumbnail.write_to(&mut cursor, format),
    }
    .map(|_| buffer)
    .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to write image to buffer"))
}

fn infer_to_image_format(kind: &infer::Type) -> AppResult<image::ImageFormat> {
    match kind.mime_type() {
        "image/jpeg" => Ok(image::ImageFormat::Jpeg),
        "image/png" => Ok(image::ImageFormat::Png),
        "image/gif" => Ok(image::ImageFormat::Gif),
        "image/webp" => Ok(image::ImageFormat::WebP),
        "image/bmp" => Ok(image::ImageFormat::Bmp),
        "image/tiff" => Ok(image::ImageFormat::Tiff),
        "image/avif" => Ok(image::ImageFormat::Avif),
        "image/x-icon" => Ok(image::ImageFormat::Ico),
        mime => Err(AppError::new(ErrType::FsError, format!("{} ({})", mime, kind.extension()))),
    }
}

/// Extract [`ExifData`] from image bytes
pub(super) async fn extract_exif_data(image_data: &[u8]) -> AppResult<Option<ExifData>> {
    let cursor = Cursor::new(image_data);
    let ms = nom_exif::AsyncMediaSource::seekable(cursor)
        .await
        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to create seekable source"))?;
    let mut parser = nom_exif::AsyncMediaParser::new();

    if !ms.has_exif() {
        return Ok(None);
    }

    let iter: nom_exif::ExifIter =
        parser.parse(ms).await.map_err(|err| AppError::err(ErrType::FsError, err, "Error parsing exif"))?;

    let exif: nom_exif::Exif = iter.into();

    let mut exif_data = ExifData {
        camera_make: exif.get(nom_exif::ExifTag::Make).and_then(|make| make.as_str()).map(|s| s.to_string()),
        camera_model: exif.get(nom_exif::ExifTag::Model).and_then(|model| model.as_str()).map(|s| s.to_string()),
        date_time: exif.get(nom_exif::ExifTag::DateTimeOriginal).and_then(|dt| dt.as_str()).map(|s| s.to_string()),
        orientation: exif.get(nom_exif::ExifTag::Orientation).and_then(|o| o.as_u32()),
        gps_latitude: None,
        gps_longitude: None,
        gps_altitude: None,
        focal_length: exif.get(nom_exif::ExifTag::FocalLength).and_then(|fl| fl.as_irational()).map(|f| f.as_float()),
        aperture: exif.get(nom_exif::ExifTag::FNumber).and_then(|ap| ap.as_irational()).map(|f| f.as_float()),
        iso: exif.get(nom_exif::ExifTag::ISOSpeedRatings).and_then(|iso| iso.as_u32()),
        exposure_time: exif.get(nom_exif::ExifTag::ExposureTime).and_then(|et| et.as_str()).map(|s| s.to_string()),
        flash: exif.get(nom_exif::ExifTag::Flash).and_then(|fl| fl.as_u32()).map(|v| v != 0),
        white_balance: exif.get(nom_exif::ExifTag::WhiteBalanceMode).and_then(|wb| wb.as_str()).map(|s| s.to_string()),
        lens_make: exif.get(nom_exif::ExifTag::LensMake).and_then(|lm| lm.as_str()).map(|s| s.to_string()),
        lens_model: exif.get(nom_exif::ExifTag::LensModel).and_then(|lm| lm.as_str()).map(|s| s.to_string()),
        software: exif.get(nom_exif::ExifTag::Software).and_then(|sw| sw.as_str()).map(|s| s.to_string()),
        image_width: exif.get(nom_exif::ExifTag::ImageWidth).and_then(|v| v.as_u32()),
        image_height: exif.get(nom_exif::ExifTag::ImageHeight).and_then(|v| v.as_u32()),
        color_space: exif.get(nom_exif::ExifTag::ColorSpace).and_then(|cs| cs.as_str()).map(|s| s.to_string()),
        custom_fields: HashMap::new(),
    };

    // Handle GPS data using the built-in GPS parsing
    if let Ok(Some(gps_info)) = exif.get_gps_info() {
        // Convert GPS coordinates from degrees/minutes/seconds to decimal
        exif_data.gps_latitude = Some(dms_to_decimal(gps_info.latitude, gps_info.latitude_ref));
        exif_data.gps_longitude = Some(dms_to_decimal(gps_info.longitude, gps_info.longitude_ref));
        exif_data.gps_altitude = Some(gps_info.altitude.as_float());
    }

    Ok(Some(exif_data))
}

fn dms_to_decimal(dms: nom_exif::LatLng, reference: char) -> f64 {
    let decimal = dms.0.as_float() + (dms.1.as_float() / 60.0) + (dms.2.as_float() / 3600.0);
    if reference == 'S' || reference == 'W' {
        -decimal
    } else {
        decimal
    }
}
