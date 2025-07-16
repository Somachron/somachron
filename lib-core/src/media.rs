use std::{io::Cursor, path::Path};

use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};

use crate::{AppResult, ErrType};

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    pub r2_path: Option<String>,
    pub metadata: serde_json::Value,
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
    _metadata: &serde_json::Value,
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

    // TODO: get orientation from metadata "Orientation" tag
    let orientation = None;

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

    let thumbnail = img.thumbnail(256, 256);

    let quality = 80;
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    match format {
        image::ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            thumbnail.write_with_encoder(encoder)
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

                return create_thumbnail(thumbnail, Some(image::ImageFormat::Jpeg), &serde_json::Value::Null).map(Some);
            }
        }
    }

    Ok(None)
}

/// Extract [`Metadata`] from image byte
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
