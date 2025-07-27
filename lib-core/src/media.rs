use std::path::PathBuf;

use ffmpeg_next as ffmpeg;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{AppResult, ErrType};

const THUMBNAIL_DIM: u32 = 256;

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
    exposure_time: Option<String>,

    latitude: Option<f64>,
    longitude: Option<f64>,
}

enum ImageFormat {
    General(image::ImageFormat),
    Heic,
}

enum ThumbnailType {
    Bytes(Vec<u8>),
    Path(PathBuf),
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

    let mut metadata: MediaMetadata =
        serde_json::from_value(data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize media data"))?;

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
    src: PathBuf,
    dst: PathBuf,
    media_type: infer::MatcherType,
    metadata: &MediaMetadata,
) -> AppResult<bool> {
    let orientation = metadata.orientation.as_ref().and_then(|v| Some(v.parse().unwrap_or(0)));
    let rotation = metadata
        .rotation
        .as_ref()
        .map(|r| match r {
            EitherValue::Either(s) => s.parse().unwrap_or(0),
            EitherValue::Or(i) => *i,
        })
        .unwrap_or(0);

    match media_type {
        infer::MatcherType::Image => handle_image(src, dst, orientation, rotation),
        infer::MatcherType::Video => handle_video(src, dst, rotation).map(|_| false),
        _ => return Err(ErrType::MediaError.new("Invalid media_type")),
    }
}

fn handle_image(src: PathBuf, mut dst: PathBuf, orientation: Option<u64>, rotation: u64) -> AppResult<bool> {
    let mut has_heic = false;
    let image_format = match infer_to_image_format(&src)? {
        ImageFormat::General(image_format) => image_format,
        ImageFormat::Heic => {
            has_heic = true;

            convert_heif_to_jpeg(&src)?;
            dst.set_extension("jpeg");

            image::ImageFormat::Jpeg
        }
    };

    create_thumbnail(ThumbnailType::Path(src), image_format, dst, orientation, rotation)?;

    Ok(has_heic)
}

fn handle_video(src: PathBuf, dst: PathBuf, rotation: u64) -> AppResult<()> {
    ffmpeg::init().map_err(|err| ErrType::MediaError.err(err, "Failed to init ffmpeg"))?;

    let mut input = ffmpeg::format::input(&src).map_err(|err| ErrType::MediaError.err(err, "Failed to input bytes"))?;

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
        ffmpeg::format::Pixel::YUV420P,
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

            // Found a frame to use as thumbnail
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
                    ThumbnailType::Bytes(thumbnail),
                    image::ImageFormat::Jpeg,
                    dst,
                    None,
                    rotation,
                );
            }
        }
    }

    Ok(())
}

fn create_thumbnail(
    data: ThumbnailType,
    format: image::ImageFormat,
    dst: PathBuf,
    orientation: Option<u64>,
    rotation: u64,
) -> AppResult<()> {
    let img = match data {
        ThumbnailType::Bytes(bytes) => image::load_from_memory_with_format(&bytes, format)
            .map_err(|err| ErrType::MediaError.err(err, "Failed to load image from bytes"))?,
        ThumbnailType::Path(path) => {
            let mut rd = image::ImageReader::open(path)
                .map_err(|err| ErrType::FsError.err(err, "Failed to load image from path"))?;
            rd.set_format(format);

            rd.decode().map_err(|err| ErrType::MediaError.err(err, "Failed to decode image"))?
        }
    };

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

    let img = match rotation {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img, // No rotation needed for 1 or unknown
    };

    let thumbnail = img.resize(THUMBNAIL_DIM, THUMBNAIL_DIM, image::imageops::FilterType::Lanczos3);
    drop(img);

    let quality = 80;
    let mut file = std::fs::File::create(dst).map_err(|err| ErrType::FsError.err(err, "Failed to open dest file"))?;

    match format {
        image::ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
            thumbnail.write_with_encoder(encoder)
        }
        _ => thumbnail.write_to(&mut file, format),
    }
    .map_err(|err| ErrType::FsError.err(err, "Failed to write image to buffer"))
}

fn infer_to_image_format(path: &PathBuf) -> AppResult<ImageFormat> {
    let kind = infer::get_from_path(path)
        .map_err(|err| ErrType::FsError.err(err, "Failed to process path"))?
        .ok_or(ErrType::MediaError.new("Could not detect file type from magic bytes"))?;

    if kind.matcher_type() != infer::MatcherType::Image {
        return Err(ErrType::MediaError.new(format!(
            "File is not an image, detected as: {} ({})",
            kind.mime_type(),
            kind.extension()
        )));
    }

    match kind.mime_type() {
        "image/jpeg" => Ok(ImageFormat::General(image::ImageFormat::Jpeg)),
        "image/png" => Ok(ImageFormat::General(image::ImageFormat::Png)),
        "image/gif" => Ok(ImageFormat::General(image::ImageFormat::Gif)),
        "image/webp" => Ok(ImageFormat::General(image::ImageFormat::WebP)),
        "image/bmp" => Ok(ImageFormat::General(image::ImageFormat::Bmp)),
        "image/tiff" => Ok(ImageFormat::General(image::ImageFormat::Tiff)),
        "image/avif" => Ok(ImageFormat::General(image::ImageFormat::Avif)),
        "image/x-icon" => Ok(ImageFormat::General(image::ImageFormat::Ico)),
        "image/heif" => Ok(ImageFormat::Heic),
        mime => Err(ErrType::MediaError.new(format!("{} ({})", mime, kind.extension()))),
    }
}

fn convert_heif_to_jpeg(path: &PathBuf) -> AppResult<()> {
    let heif =
        libheif_rs::LibHeif::new_checked().map_err(|err| ErrType::MediaError.err(err, "Failed to init libheif"))?;

    let ctx = libheif_rs::HeifContext::read_from_file(path.to_str().unwrap())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create HeifContext"))?;
    let handle =
        ctx.primary_image_handle().map_err(|err| ErrType::MediaError.err(err, "Failed to get heif primary handle"))?;

    let image = heif
        .decode(&handle, libheif_rs::ColorSpace::Rgb(libheif_rs::RgbChroma::Rgb), None)
        .map_err(|err| ErrType::MediaError.err(err, "Failed to decode from heif handle"))?;
    let planes = image.planes();
    let interleaved =
        planes.interleaved.ok_or(ErrType::MediaError.new("Interleaved planes not found in heif image"))?;

    let img_buffer: image::RgbImage =
        image::ImageBuffer::from_raw(interleaved.width, interleaved.height, interleaved.data.to_vec()).unwrap();

    let img = image::DynamicImage::ImageRgb8(img_buffer);

    let file =
        std::fs::File::create(path).map_err(|err| ErrType::FsError.err(err, "Failed to create heif convert file"))?;

    let encoder = image::codecs::jpeg::JpegEncoder::new(file);
    img.write_with_encoder(encoder).map_err(|err| ErrType::MediaError.err(err, "Failed to encode heif to jpeg"))?;

    Ok(())
}
