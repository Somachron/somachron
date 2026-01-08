use ffmpeg_next as ffmpeg;
use image::DynamicImage;
use std::path::PathBuf;
use thumbnail_output::{ImageData, ProcessedImage};

use super::err::{AppResult, ErrType};

const THUMNAIL_HEIGHT: u32 = 176;
const PREVIEW_HEIGHT: u32 = 1080;

enum ImageFormat {
    General(image::ImageFormat),
    Heif,
}

#[derive(Debug, Clone)]
enum ImageType {
    Bytes(Vec<u8>),
    Path(PathBuf),
    Img(DynamicImage),
}

impl ImageType {
    fn get_img(self, format: image::ImageFormat) -> AppResult<DynamicImage> {
        match self {
            ImageType::Bytes(bytes) => image::load_from_memory_with_format(&bytes, format)
                .map_err(|err| ErrType::MediaError.err(err, "Failed to load image from bytes")),
            ImageType::Path(path) => {
                let mut rd = image::ImageReader::open(path)
                    .map_err(|err| ErrType::FsError.err(err, "Failed to load image from path"))?;
                rd.set_format(format);

                rd.decode().map_err(|err| ErrType::MediaError.err(err, "Failed to decode image"))
            }
            ImageType::Img(img) => Ok(img),
        }
    }
}

pub fn handle_image(src: PathBuf, rotation: Option<u64>) -> AppResult<ProcessedImage> {
    let file_name = src
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(ErrType::FsError.msg(format!("Failed to get file name for {src:?}")))?;

    let mut preview_dst = src.clone();
    preview_dst.set_file_name(format!("preview_{file_name}.jpeg"));

    let mut thumbnail_dst = src.clone();
    thumbnail_dst.set_file_name(format!("thumbnail_{file_name}.jpeg"));

    let (image_format, img_ty, rotation) = match infer_to_image_format(&src)? {
        ImageFormat::General(image_format) => (image_format, ImageType::Path(src), rotation.unwrap_or_default()),
        ImageFormat::Heif => {
            let heif_img = convert_heif_to_jpeg(&src)?;
            (image::ImageFormat::Jpeg, ImageType::Img(heif_img), 0)
        }
    };

    let preview = create_preview(img_ty.clone(), image_format, preview_dst, rotation)?;
    let thumbnail = create_thumbnail(img_ty, image_format, thumbnail_dst, rotation)?;

    Ok(ProcessedImage {
        thumbnail,
        preview,
    })
}

pub fn handle_video(src: PathBuf, dst: PathBuf, rotation: Option<u64>) -> AppResult<ProcessedImage> {
    ffmpeg::init().map_err(|err| ErrType::MediaError.err(err, "Failed to init ffmpeg"))?;

    let mut input = ffmpeg::format::input(&src).map_err(|err| ErrType::MediaError.err(err, "Failed to input bytes"))?;

    let video_stream =
        input.streams().best(ffmpeg::media::Type::Video).ok_or(ErrType::MediaError.msg("No video stream found"))?;

    let stream_index = video_stream.index();
    let context_decoder = ffmpeg::codec::Context::from_parameters(video_stream.parameters())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create context decoder"))?;
    let mut decoder =
        context_decoder.decoder().video().map_err(|err| ErrType::MediaError.err(err, "Failed to get decoder"))?;

    let codec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::MJPEG).ok_or(ErrType::MediaError.msg("MJPEG codec not found"))?;
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
            if decoder.receive_frame(&mut frame).is_ok() {
                scaler
                    .run(&frame, &mut scaled_frame)
                    .map_err(|err| ErrType::MediaError.err(err, "Failed to scale frame"))?;

                encoder
                    .send_frame(&scaled_frame)
                    .map_err(|err| ErrType::MediaError.err(err, "Failed to send scaled frame to encoder"))?;

                let mut bytes = Vec::<u8>::new();
                let mut encoded_packet = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    let data = encoded_packet.data().ok_or(ErrType::MediaError.msg("Empty encoded packet data"))?;
                    bytes.extend_from_slice(data);
                }

                encoder.send_eof().map_err(|err| ErrType::MediaError.err(err, "Failed to send EOF to encoder"))?;

                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    let data =
                        encoded_packet.data().ok_or(ErrType::MediaError.msg("Empty draining encoded packet data"))?;
                    bytes.extend_from_slice(data);
                }

                let thumbnail = create_thumbnail(
                    ImageType::Bytes(bytes.clone()),
                    image::ImageFormat::Jpeg,
                    dst.clone(),
                    rotation.unwrap_or_default(),
                )?;
                let preview = create_preview(
                    ImageType::Bytes(bytes),
                    image::ImageFormat::Jpeg,
                    dst,
                    rotation.unwrap_or_default(),
                )?;

                return Ok(ProcessedImage {
                    thumbnail,
                    preview,
                });
            }
        }
    }

    Err(ErrType::MediaError.msg("No frames found to process"))
}

fn create_thumbnail(data: ImageType, format: image::ImageFormat, dst: PathBuf, rotation: u64) -> AppResult<ImageData> {
    let img = data.get_img(format)?;
    process_image(img, dst, THUMNAIL_HEIGHT, rotation, 60)
}

fn create_preview(data: ImageType, format: image::ImageFormat, dst: PathBuf, rotation: u64) -> AppResult<ImageData> {
    let img = data.get_img(format)?;
    process_image(img, dst, PREVIEW_HEIGHT, rotation, 80)
}

fn process_image(img: DynamicImage, dst: PathBuf, height: u32, rotation: u64, quality: u8) -> AppResult<ImageData> {
    let img = rotate_image(img, rotation);

    // calculate proportional width based on fixed height ratio
    let hratio = f64::from(height) / f64::from(img.height());
    let width = (f64::from(img.width()) * hratio).round() as u32;

    let p_image = img.resize(width, height, image::imageops::FilterType::Lanczos3);
    drop(img);

    let file = std::fs::File::create(&dst).map_err(|err| ErrType::FsError.err(err, "Failed to open dest file"))?;

    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
    p_image.write_with_encoder(encoder).map_err(|err| ErrType::FsError.err(err, "Failed to write image to buffer"))?;

    Ok(ImageData {
        width: p_image.width(),
        height: p_image.height(),
        path: dst,
    })
}

fn infer_to_image_format(path: &PathBuf) -> AppResult<ImageFormat> {
    let kind = infer::get_from_path(path)
        .map_err(|err| ErrType::FsError.err(err, "Failed to process path"))?
        .ok_or(ErrType::MediaError.msg("Could not detect file type from magic bytes"))?;

    if kind.matcher_type() != infer::MatcherType::Image {
        return Err(ErrType::MediaError.msg(format!(
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
        "image/heif" => Ok(ImageFormat::Heif),
        mime => Err(ErrType::MediaError.msg(format!("{} ({})", mime, kind.extension()))),
    }
}

fn convert_heif_to_jpeg(path: &PathBuf) -> AppResult<DynamicImage> {
    let heif =
        libheif_rs::LibHeif::new_checked().map_err(|err| ErrType::MediaError.err(err, "Failed to init libheif"))?;

    let ctx = libheif_rs::HeifContext::read_from_file(path.to_str().unwrap())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create HeifContext"))?;

    // heif contains multiple images
    let image_handles = ctx.top_level_image_handles();

    let handle = ctx
        .primary_image_handle()
        .ok()
        .or(image_handles.into_iter().next())
        .ok_or(ErrType::MediaError.msg("No image handle found for heif"))?;

    // get image
    let image = heif
        .decode(&handle, libheif_rs::ColorSpace::Rgb(libheif_rs::RgbChroma::Rgb), None)
        .map_err(|err| ErrType::MediaError.err(err, "Failed to decode from heif handle"))?;
    let planes = image.planes();
    let interleaved =
        planes.interleaved.ok_or(ErrType::MediaError.msg("Interleaved planes not found in heif image"))?;

    // get buffer
    let img_buffer: image::RgbImage =
        image::ImageBuffer::from_raw(interleaved.width, interleaved.height, interleaved.data.to_vec()).unwrap();

    // create dynamic image
    let img = image::DynamicImage::ImageRgb8(img_buffer);
    Ok(img)
}

fn rotate_image(img: DynamicImage, rotation: u64) -> DynamicImage {
    match rotation {
        2 => img.rotate90(),
        3 => img.rotate180(),
        4 => img.rotate270(),
        5 => img.fliph(),
        6 => img.flipv(),
        7 => img.fliph().rotate270(),
        8 => img.fliph().rotate90(),
        _ => img, // No rotation needed
    }
}
