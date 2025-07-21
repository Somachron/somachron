use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use ffmpeg_next as ffmpeg;

use crate::{AppResult, ErrType};

const THUMBNAIL_DIM: u32 = 256;

pub enum ImageFormat {
    General(image::ImageFormat),
    Heic,
}

pub enum ThumbnailType {
    Bytes(Vec<u8>),
    Path(PathBuf),
}

pub struct MediaProcessor {
    heif: libheif_rs::LibHeif,
}

impl MediaProcessor {
    pub(super) fn new() -> Self {
        let heif = libheif_rs::LibHeif::new_checked().expect("Failed to init libheif");

        ffmpeg::init().expect("Failed to init ffmpeg");

        Self {
            heif,
        }
    }

    /// Get media type [`infer::MatcherType::Image`] or [`infer::MatcherType::Video`]
    /// based on `ext` extension
    pub(super) fn get_media_type(&self, ext: &str) -> infer::MatcherType {
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
        &self,
        data: ThumbnailType,
        format: image::ImageFormat,
        dest_path: PathBuf,
        metadata: &serde_json::Value,
    ) -> AppResult<()> {
        let orientation = metadata.get("Orientation").and_then(|v| v.as_u64());
        let rotation = metadata.get("Rotation").and_then(|v| v.as_u64()).unwrap_or(0);

        let img = match data {
            ThumbnailType::Bytes(bytes) => image::load_from_memory_with_format(&bytes, format)
                .map_err(|err| ErrType::MediaError.err(err, "Failed to load image from bytes"))?,
            ThumbnailType::Path(tmp_path) => {
                let mut rd = image::ImageReader::open(tmp_path)
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
        // let thumbnail = img;

        let quality = 80;
        let mut file =
            std::fs::File::create(dest_path).map_err(|err| ErrType::FsError.err(err, "Failed to open dest file"))?;

        match format {
            image::ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
                thumbnail.write_with_encoder(encoder)
            }
            _ => thumbnail.write_to(&mut file, format),
        }
        .map_err(|err| ErrType::FsError.err(err, "Failed to write image to buffer"))
    }

    pub(super) fn infer_to_image_format(&self, tmp_path: impl AsRef<Path>) -> AppResult<ImageFormat> {
        let kind = infer::get_from_path(tmp_path)
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

    pub(super) fn convert_heif_to_jpeg(&self, tmp_path: &str) -> AppResult<Vec<u8>> {
        let ctx = libheif_rs::HeifContext::read_from_file(tmp_path)
            .map_err(|err| ErrType::MediaError.err(err, "Failed to create HeifContext"))?;
        let handle = ctx
            .primary_image_handle()
            .map_err(|err| ErrType::MediaError.err(err, "Failed to get heif primary handle"))?;

        let image = self
            .heif
            .decode(&handle, libheif_rs::ColorSpace::Rgb(libheif_rs::RgbChroma::Rgb), None)
            .map_err(|err| ErrType::MediaError.err(err, "Failed to decode from heif handle"))?;
        let planes = image.planes();
        let interleaved =
            planes.interleaved.ok_or(ErrType::MediaError.new("Interleaved planes not found in heif image"))?;

        let img_buffer: image::RgbImage =
            image::ImageBuffer::from_raw(interleaved.width, interleaved.height, interleaved.data.to_vec()).unwrap();

        let img = image::DynamicImage::ImageRgb8(img_buffer);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        let encoder = image::codecs::jpeg::JpegEncoder::new(&mut cursor);
        img.write_with_encoder(encoder).map_err(|err| ErrType::MediaError.err(err, "Failed to encode heif to jpeg"))?;

        Ok(buffer)
    }

    pub(super) fn process_video_thumbnail(
        &self,
        tmp_path: impl AsRef<Path>,
        dest_path: PathBuf,
        metadata: &serde_json::Value,
    ) -> AppResult<()> {
        let mut input = ffmpeg::format::input(tmp_path.as_ref())
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
                        let data = encoded_packet
                            .data()
                            .ok_or(ErrType::MediaError.new("Empty draining encoded packet data"))?;
                        thumbnail.extend_from_slice(data);
                    }

                    return self.create_thumbnail(
                        ThumbnailType::Bytes(thumbnail),
                        image::ImageFormat::Jpeg,
                        dest_path,
                        metadata,
                    );
                }
            }
        }

        Ok(())
    }

    /// Extract [`Metadata`] from image byte
    pub(super) fn extract_metadata(&self, tmp_path: impl AsRef<Path>) -> AppResult<serde_json::Value> {
        let mut tool =
            exiftool::ExifTool::new().map_err(|err| ErrType::MediaError.err(err, "Failed to init exif tool"))?;

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
}
