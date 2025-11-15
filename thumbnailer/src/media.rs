use ffmpeg_next as ffmpeg;
use std::path::PathBuf;

use super::err::{AppResult, ErrType};

const THUMBNAIL_DIM: u32 = 256;

enum ImageFormat {
    General(image::ImageFormat),
    Heif,
}

enum ThumbnailType {
    Bytes(Vec<u8>),
    Path(PathBuf),
}

pub fn handle_image(src: PathBuf, rotation: Option<u64>) -> AppResult<(u32, u32, Option<Vec<String>>)> {
    match infer_to_image_format(&src)? {
        ImageFormat::General(image_format) => {
            let (w, h) = create_thumbnail(ThumbnailType::Path(src.clone()), image_format, src, rotation)?;
            Ok((w, h, None))
        }
        ImageFormat::Heif => {
            let paths = convert_heif_to_jpeg(&src)?;

            let file_name = src
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or(ErrType::FsError.msg(format!("Failed to get file name for {src:?}")))?;

            let mut heif_paths = Vec::with_capacity(paths.len());
            let (mut w, mut h) = (0, 0);
            for (i, src) in paths.into_iter().enumerate() {
                let mut dst = src.clone();
                dst.set_file_name(format!("{file_name}_{i}.jpeg"));

                heif_paths.push(src.to_str().unwrap().to_owned());

                let (_w, _h) = create_thumbnail(ThumbnailType::Path(src), image::ImageFormat::Jpeg, dst, rotation)?;
                w = w.max(_w);
                h = h.max(_h);
            }

            Ok((w, h, Some(heif_paths)))
        }
    }
}

pub fn handle_video(src: PathBuf, dst: PathBuf, rotation: Option<u64>) -> AppResult<(u32, u32)> {
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

                let mut thumbnail = Vec::<u8>::new();
                let mut encoded_packet = ffmpeg::Packet::empty();
                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    let data = encoded_packet.data().ok_or(ErrType::MediaError.msg("Empty encoded packet data"))?;
                    thumbnail.extend_from_slice(data);
                }

                encoder.send_eof().map_err(|err| ErrType::MediaError.err(err, "Failed to send EOF to encoder"))?;

                while encoder.receive_packet(&mut encoded_packet).is_ok() {
                    let data =
                        encoded_packet.data().ok_or(ErrType::MediaError.msg("Empty draining encoded packet data"))?;
                    thumbnail.extend_from_slice(data);
                }

                return create_thumbnail(ThumbnailType::Bytes(thumbnail), image::ImageFormat::Jpeg, dst, rotation);
            }
        }
    }

    Ok((THUMBNAIL_DIM, THUMBNAIL_DIM))
}

fn create_thumbnail(
    data: ThumbnailType,
    format: image::ImageFormat,
    dst: PathBuf,
    rotation: Option<u64>,
) -> AppResult<(u32, u32)> {
    let rotation = rotation.unwrap_or(0);

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

    let img = match rotation {
        2 => img.rotate90(),
        3 => img.rotate180(),
        4 => img.rotate270(),
        5 => img.fliph(),
        6 => img.flipv(),
        7 => img.fliph().rotate270(),
        8 => img.fliph().rotate90(),
        _ => img, // No rotation needed
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
    .map_err(|err| ErrType::FsError.err(err, "Failed to write image to buffer"))?;

    Ok((thumbnail.width(), thumbnail.height()))
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

fn convert_heif_to_jpeg(path: &PathBuf) -> AppResult<Vec<PathBuf>> {
    let heif =
        libheif_rs::LibHeif::new_checked().map_err(|err| ErrType::MediaError.err(err, "Failed to init libheif"))?;

    let ctx = libheif_rs::HeifContext::read_from_file(path.to_str().unwrap())
        .map_err(|err| ErrType::MediaError.err(err, "Failed to create HeifContext"))?;

    // heif contains multiple images
    let image_handles = ctx.top_level_image_handles();

    // each image handle will have it's own new path now
    let mut updated_paths = Vec::with_capacity(image_handles.len());

    for (i, handle) in image_handles.into_iter().enumerate() {
        // prepare different path for image
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap();
        let path = path.with_file_name(format!("{file_name}_{i}"));

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

        // write to file
        let file = std::fs::File::create(&path)
            .map_err(|err| ErrType::FsError.err(err, "Failed to create heif convert file"))?;

        let encoder = image::codecs::jpeg::JpegEncoder::new(file);
        img.write_with_encoder(encoder).map_err(|err| ErrType::MediaError.err(err, "Failed to encode heif to jpeg"))?;

        // insert new path
        updated_paths.push(path);
    }

    // remove original path
    let _ = std::fs::remove_file(path);

    Ok(updated_paths)
}
