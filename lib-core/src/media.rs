use std::{collections::HashMap, io::Cursor, path::Path};

use chrono::{DateTime, FixedOffset};
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};

use crate::{AppResult, ErrType};

#[derive(Serialize, Deserialize)]
pub enum Metadata {
    ExifData {
        camera_make: Option<String>,
        camera_model: Option<String>,
        date_time: Option<DateTime<FixedOffset>>,
        orientation: Option<u32>,
        focal_length: Option<f64>,
        aperture: Option<f64>,
        iso: Option<u32>,
        exposure_time: Option<String>,
        flash: Option<bool>,
        white_balance: Option<String>,
        lens_make: Option<String>,
        lens_model: Option<String>,
        software: Option<String>,
        image_width: Option<u32>,
        image_height: Option<u32>,
        color_space: Option<String>,
        custom_fields: HashMap<String, String>,
        gps_latitude: Option<f64>,
        gps_longitude: Option<f64>,
    },
    TrackData {
        camera_make: Option<String>,
        camera_model: Option<String>,
        date_time: Option<DateTime<FixedOffset>>,
        duration: Option<u64>,
        width: Option<u32>,
        height: Option<u32>,
        author: Option<String>,
        gps_latitude: Option<f64>,
        gps_longitude: Option<f64>,
    },
    Generic(serde_json::Value),
}
impl Metadata {
    pub fn set_gps_info(&mut self, latitude: Option<f64>, longitude: Option<f64>) {
        match self {
            Metadata::ExifData {
                gps_latitude,
                gps_longitude,
                ..
            } => {
                *gps_latitude = latitude;
                *gps_longitude = longitude;
            }
            Metadata::TrackData {
                gps_latitude,
                gps_longitude,
                ..
            } => {
                *gps_latitude = latitude;
                *gps_longitude = longitude;
            }
            Metadata::Generic(_) => {}
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    pub r2_path: Option<String>,
    pub metadata: Metadata,
    pub size: usize,
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

pub(super) fn create_thumbnail(
    bytes: Vec<u8>,
    format: Option<image::ImageFormat>,
    metadata: &Metadata,
) -> AppResult<Vec<u8>> {
    let format = match format {
        Some(f) => f,
        None => {
            let kind = infer::get(&bytes).ok_or(ErrType::FsError.new("Could not detect file type from magic bytes"))?;

            if kind.matcher_type() != infer::MatcherType::Image {
                return Err(ErrType::FsError.new(format!(
                    "File is not an image, detected as: {} ({})",
                    kind.mime_type(),
                    kind.extension()
                )));
            }

            infer_to_image_format(&kind)?
        }
    };

    let orientation = match metadata {
        Metadata::ExifData {
            orientation,
            ..
        } => orientation,
        _ => &None,
    };

    let img = image::load_from_memory_with_format(&bytes, format)
        .map_err(|err| ErrType::FsError.err(err, "Failed to load image from bytes"))?;

    let img = match orientation {
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

    let quality = 80;
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    match format {
        image::ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            img.write_with_encoder(encoder)
        }
        _ => thumbnail.write_to(&mut cursor, format),
    }
    .map(|_| buffer)
    .map_err(|err| ErrType::FsError.err(err, "Failed to write image to buffer"))
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
        mime => Err(ErrType::MediaError.new(format!("{} ({})", mime, kind.extension()))),
    }
}

pub(super) fn process_video_thumbnail(tmp_bytes_path: impl AsRef<Path>) -> AppResult<Option<Vec<u8>>> {
    ffmpeg::init().map_err(|err| ErrType::MediaError.err(err, "Failed to init ffmpeg"))?;

    let mut input = ffmpeg::format::input(tmp_bytes_path.as_ref())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to input bytes"))?;

    let video_stream =
        input.streams().best(ffmpeg::media::Type::Video).ok_or(ErrType::MediaError.new("No video stream found"))?;

    let stream_index = video_stream.index();
    let context_decoder = ffmpeg::codec::Context::from_parameters(video_stream.parameters())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create context decoder"))?;
    let mut decoder =
        context_decoder.decoder().video().map_err(|err| ErrType::MediaError.err(err, "Failed to get decoder"))?;

    let codec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::MJPEG).ok_or(ErrType::MediaError.new("MJPEG codec not found"))?;
    let mut encoder = ffmpeg::codec::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(|err| ErrType::MediaError.err(err, "Failed to init MJPEG encoder"))?;

    encoder.set_width(decoder.width());
    encoder.set_height(decoder.height());
    encoder.set_format(ffmpeg::format::Pixel::YUVJ420P);
    encoder.set_time_base(ffmpeg::Rational(1, 1));

    let mut encoder = encoder.open().map_err(|err| ErrType::MediaError.err(err, "Failed to open encoder"))?;

    let mut frame = ffmpeg::frame::Video::empty();
    let mut scaled_frame = ffmpeg::frame::Video::empty();

    // Create scaler once
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::YUVJ420P,
        decoder.width(),
        decoder.height(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|err| ErrType::MediaError.err(err, "Failed to create scaler"))?;

    // Read frames until we get one we can use
    for (stream, packet) in input.packets() {
        if stream.index() == stream_index {
            decoder
                .send_packet(&packet)
                .map_err(|err| ErrType::MediaError.err(err, "Failed to send packet to decoder"))?;

            if let Ok(_) = decoder.receive_frame(&mut frame) {
                scaler
                    .run(&frame, &mut scaled_frame)
                    .map_err(|err| ErrType::MediaError.err(err, "Failed to scale frame"))?;

                encoder
                    .send_frame(&scaled_frame)
                    .map_err(|err| ErrType::MediaError.err(err, "Failed to send scaled frame to encoder"))?;

                let mut thumbnail = Vec::<u8>::new();
                let mut encoded_packet = ffmpeg::Packet::empty();
                while let Ok(_) = encoder.receive_packet(&mut encoded_packet) {
                    let data = encoded_packet.data().ok_or(ErrType::MediaError.new("Empty encoded packet data"))?;
                    thumbnail.extend_from_slice(data);
                }

                encoder.send_eof().map_err(|err| ErrType::MediaError.err(err, "Failed to send EOF to encoder"))?;

                while let Ok(_) = encoder.receive_packet(&mut encoded_packet) {
                    let data =
                        encoded_packet.data().ok_or(ErrType::MediaError.new("Empty draining encoded packet data"))?;
                    thumbnail.extend_from_slice(data);
                }

                return create_thumbnail(
                    thumbnail,
                    Some(image::ImageFormat::Jpeg),
                    &Metadata::Generic(serde_json::Value::Null),
                )
                .map(Some);
            }
        }
    }

    Ok(None)
}

/// Extract [`Metadata`] from image byte
///
/// First attempts to extract EXIF(image) or Track(video) data.
/// On failure, extracts generic metadata
pub(super) async fn extract_metadata(tmp_path: impl AsRef<Path>) -> AppResult<Metadata> {
    match extract_exif_track(tmp_path.as_ref()).await? {
        Some(data) => Ok(data),
        None => {
            let mut tool =
                exiftool::ExifTool::new().map_err(|err| ErrType::MediaError.err(err, "Failed to init exif tool"))?;

            tool.json(tmp_path.as_ref(), &[])
                .map(Metadata::Generic)
                .map_err(|err| ErrType::MediaError.err(err, "Failed to extract metadata data"))
        }
    }
}

async fn extract_exif_track(tmp_path: impl AsRef<Path>) -> AppResult<Option<Metadata>> {
    let ms = nom_exif::AsyncMediaSource::file_path(tmp_path.as_ref())
        .await
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create media source"))?;
    let mut parser = nom_exif::AsyncMediaParser::new();

    if ms.has_exif() {
        let iter: nom_exif::ExifIter =
            parser.parse(ms).await.map_err(|err| ErrType::MediaError.err(err, "Error parsing exif"))?;

        let exif: nom_exif::Exif = iter.into();

        let mut exif_data = Metadata::ExifData {
            camera_make: exif.get(nom_exif::ExifTag::Make).and_then(|make| make.as_str()).map(|s| s.to_string()),
            camera_model: exif.get(nom_exif::ExifTag::Model).and_then(|model| model.as_str()).map(|s| s.to_string()),
            date_time: exif.get(nom_exif::ExifTag::DateTimeOriginal).and_then(|dt| dt.as_time()),
            orientation: exif.get(nom_exif::ExifTag::Orientation).and_then(|o| o.as_u32()),
            focal_length: exif
                .get(nom_exif::ExifTag::FocalLength)
                .and_then(|fl| fl.as_irational())
                .map(|f| f.as_float()),
            aperture: exif.get(nom_exif::ExifTag::FNumber).and_then(|ap| ap.as_irational()).map(|f| f.as_float()),
            iso: exif.get(nom_exif::ExifTag::ISOSpeedRatings).and_then(|iso| iso.as_u32()),
            exposure_time: exif.get(nom_exif::ExifTag::ExposureTime).and_then(|et| et.as_str()).map(|s| s.to_string()),
            flash: exif.get(nom_exif::ExifTag::Flash).and_then(|fl| fl.as_u32()).map(|v| v != 0),
            white_balance: exif
                .get(nom_exif::ExifTag::WhiteBalanceMode)
                .and_then(|wb| wb.as_str())
                .map(|s| s.to_string()),
            lens_make: exif.get(nom_exif::ExifTag::LensMake).and_then(|lm| lm.as_str()).map(|s| s.to_string()),
            lens_model: exif.get(nom_exif::ExifTag::LensModel).and_then(|lm| lm.as_str()).map(|s| s.to_string()),
            software: exif.get(nom_exif::ExifTag::Software).and_then(|sw| sw.as_str()).map(|s| s.to_string()),
            image_width: exif.get(nom_exif::ExifTag::ImageWidth).and_then(|v| v.as_u32()),
            image_height: exif.get(nom_exif::ExifTag::ImageHeight).and_then(|v| v.as_u32()),
            color_space: exif.get(nom_exif::ExifTag::ColorSpace).and_then(|cs| cs.as_str()).map(|s| s.to_string()),
            custom_fields: HashMap::new(),
            gps_latitude: None,
            gps_longitude: None,
        };

        // Handle GPS data using the built-in GPS parsing
        if let Ok(Some(gps_info)) = exif.get_gps_info() {
            // Convert GPS coordinates from degrees/minutes/seconds to decimal
            let gps_latitude = Some(dms_to_decimal(&gps_info.latitude, gps_info.latitude_ref));
            let gps_longitude = Some(dms_to_decimal(&gps_info.longitude, gps_info.longitude_ref));
            exif_data.set_gps_info(gps_latitude, gps_longitude);
        }

        Ok(Some(exif_data))
    } else if ms.has_track() {
        let track: nom_exif::TrackInfo =
            parser.parse(ms).await.map_err(|err| ErrType::MediaError.err(err, "Error parsing track"))?;

        let mut track_data = Metadata::TrackData {
            camera_make: track.get(nom_exif::TrackInfoTag::Make).and_then(|make| make.as_str()).map(|s| s.to_string()),
            camera_model: track
                .get(nom_exif::TrackInfoTag::Model)
                .and_then(|model| model.as_str())
                .map(|s| s.to_string()),
            date_time: track.get(nom_exif::TrackInfoTag::CreateDate).and_then(|date| date.as_time()),
            duration: track.get(nom_exif::TrackInfoTag::DurationMs).and_then(|duration| duration.as_u64()),
            width: track.get(nom_exif::TrackInfoTag::ImageWidth).and_then(|w| w.as_u32()),
            height: track.get(nom_exif::TrackInfoTag::ImageHeight).and_then(|h| h.as_u32()),
            author: track.get(nom_exif::TrackInfoTag::Author).and_then(|a| a.as_str()).map(|s| s.to_string()),
            gps_latitude: None,
            gps_longitude: None,
        };

        if let Some(gps_info) = track.get_gps_info() {
            // Convert GPS coordinates from degrees/minutes/seconds to decimal
            let gps_latitude = Some(dms_to_decimal(&gps_info.latitude, gps_info.latitude_ref));
            let gps_longitude = Some(dms_to_decimal(&gps_info.longitude, gps_info.longitude_ref));
            track_data.set_gps_info(gps_latitude, gps_longitude);
        }

        Ok(Some(track_data))
    } else {
        Ok(None)
    }
}

fn dms_to_decimal(dms: &nom_exif::LatLng, reference: char) -> f64 {
    let decimal = dms.0.as_float() + (dms.1.as_float() / 60.0) + (dms.2.as_float() / 3600.0);
    if reference == 'S' || reference == 'W' {
        -decimal
    } else {
        decimal
    }
}
