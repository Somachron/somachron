use std::{path::PathBuf, sync::Arc};

use axum::response::sse;
use futures_util::TryFutureExt;
use lib_core::{
    interconnect::ServiceInterconnect, storage::s3::S3Storage, AppError, AppResult, ErrType, X_SPACE_HEADER,
};
use smq_dto::{
    req::ProcessMediaRequest,
    res::{FileData, ImageData, MediaData, ProcessedImage},
    MediaMetadata, MediaType,
};
use uuid::Uuid;

use crate::media;

mod broadcast;
mod pool;

const EXIFTOOL_EXE: &str = "exiftool";

#[derive(Debug, Clone)]
pub enum QueueEvent {
    Queued,
    Started,
    Done,
    Err(AppError),
}

impl broadcast::BroadcastEvent for QueueEvent {
    fn init_event() -> Self {
        Self::Queued
    }
}

impl QueueEvent {
    pub fn event(self) -> sse::Event {
        match self {
            QueueEvent::Queued => sse::Event::default().event("queued"),
            QueueEvent::Started => sse::Event::default().event("started"),
            QueueEvent::Done => sse::Event::default().event("done"),
            QueueEvent::Err(err) => sse::Event::default().event("error").data(err.err_message()),
        }
    }
}

pub struct MediaQueue {
    pool: Arc<pool::ThreadPool<AppResult<(MediaMetadata, i64, ProcessedImage)>>>,
    broadcaster: Arc<tokio::sync::Mutex<broadcast::Broadcaster<QueueEvent>>>,
    s3: Arc<S3Storage>,
    interconnect: Arc<ServiceInterconnect>,
    backend_client: Arc<reqwest::Client>,
}

impl Clone for MediaQueue {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            broadcaster: self.broadcaster.clone(),
            s3: self.s3.clone(),
            interconnect: self.interconnect.clone(),
            backend_client: self.backend_client.clone(),
        }
    }
}

impl MediaQueue {
    pub fn new() -> Self {
        let client = reqwest::ClientBuilder::new().build().expect("Failed to create backend client");
        Self {
            pool: Arc::new(pool::ThreadPool::new(8)),
            broadcaster: Arc::new(tokio::sync::Mutex::new(broadcast::Broadcaster::new())),
            s3: Arc::new(S3Storage::new()),
            interconnect: Arc::new(ServiceInterconnect::new()),
            backend_client: Arc::new(client),
        }
    }

    pub fn interconnect(&self) -> &ServiceInterconnect {
        &self.interconnect
    }

    pub async fn queue_job(
        &self,
        ProcessMediaRequest {
            file_id,
            updated_date,
            space_id,
            folder_id,
            s3_file_path,
        }: ProcessMediaRequest,
    ) -> AppResult<()> {
        let s3_file_path = Arc::<str>::from(s3_file_path);
        let s3_file_path_buf = PathBuf::from(s3_file_path.as_ref());

        let ext = s3_file_path_buf
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.msg("Invalid file path without extenstion"))?;
        let file_stem = s3_file_path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .ok_or(ErrType::FsError.msg("Invalid file stem"))?;
        let file_name = s3_file_path_buf
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .ok_or(ErrType::FsError.msg("Invalid file name"))?;

        let media_ty = get_media_type(ext)?;

        {
            let mut b = self.broadcaster.lock().await;
            b.add_client(&file_id).await;
        };

        // spawn job
        let broadcaster = self.broadcaster.clone();
        let s3 = self.s3.clone();
        let _file_name = file_name.clone();
        let mut recv = self.pool.execute(move || {
            // send started event
            tokio::runtime::Handle::current().block_on(async move {
                {
                    let b = broadcaster.lock().await;
                    b.broadcast(&file_id, QueueEvent::Started).await;
                }
            });

            // extract metadata
            let _s3 = s3.clone();
            let _s3_file_path = s3_file_path.clone();
            let metadata_result = tokio::runtime::Handle::current().block_on(async move {
                let file_size = _s3
                    .head_object(&_s3_file_path)
                    .await
                    .and_then(|head| head.content_length.ok_or(ErrType::S3Error.msg("Failed to get size of file")));

                let size_and_url = match file_size {
                    Ok(file_size) => {
                        let url = _s3.generate_stream_signed_url(&_s3_file_path).await;
                        url.map(|u| (file_size, u))
                    }
                    Err(err) => Err(err),
                };

                match size_and_url {
                    Ok((file_size, url)) => extract_metadata(&url, &_file_name).await.map(|m| (m, file_size, url)),
                    Err(err) => Err(err),
                }
            });

            // process thumbnail and preview
            let _s3 = s3.clone();
            let _s3_file_path = s3_file_path.clone();
            let result = match metadata_result {
                Ok((metadata, file_size, url)) => {
                    let rotation = metadata.rotation.as_ref().map(|v| match v {
                        smq_dto::EitherValue::Either(e) => e.get_value(),
                        smq_dto::EitherValue::Or(v) => smq_dto::MediaOrientation::from_rotation(*v).get_value(),
                    });

                    let bytes = match media_ty {
                        MediaType::Image => tokio::runtime::Handle::current()
                            .block_on(async move {
                                let bs = _s3.download_media(&_s3_file_path).await;
                                match bs {
                                    Ok(bs) => bs
                                        .collect()
                                        .map_err(|err| ErrType::S3Error.err(err, "Failed to read download bte stream"))
                                        .await
                                        .map(|b| b.to_vec()),
                                    Err(err) => Err(err),
                                }
                            })
                            .and_then(|bytes| media::handle_image(bytes, rotation)),
                        MediaType::Video => media::handle_video(url, rotation),
                    };

                    bytes.map(|b| (metadata, file_size, b))
                }
                Err(err) => Err(err),
            };

            // upload processed images
            match result {
                Ok((
                    metadata,
                    file_size,
                    media::ProcessedBytes {
                        thumbnail,
                        preview,
                    },
                )) => {
                    let mut thumbnail_path = PathBuf::from(s3_file_path.as_ref());
                    let thumbnail_file_name = format!("thumbnail_{file_stem}.jpeg");
                    thumbnail_path.set_file_name(&thumbnail_file_name);
                    let thumbnail_path = thumbnail_path.to_str().map(|s| s.to_owned()).unwrap_or_default();
                    let thumbnail_data = ImageData {
                        width: thumbnail.width as i32,
                        height: thumbnail.height as i32,
                        file_name: thumbnail_file_name,
                    };

                    let mut preview_path = PathBuf::from(s3_file_path.as_ref());
                    let preview_file_name = format!("preview_{file_stem}.jpeg");
                    preview_path.set_file_name(&preview_file_name);
                    let preview_path = preview_path.to_str().map(|s| s.to_owned()).unwrap_or_default();
                    let preview_data = ImageData {
                        width: preview.width as i32,
                        height: preview.height as i32,
                        file_name: preview_file_name,
                    };

                    tokio::runtime::Handle::current()
                        .block_on(async move {
                            let th = s3.upload_photo(thumbnail_path.as_str(), thumbnail.buf).await;
                            let pr = s3.upload_photo(preview_path.as_str(), preview.buf).await;
                            th.and_then(|_| pr)
                        })
                        .map(|_| {
                            (
                                metadata,
                                file_size,
                                ProcessedImage {
                                    thumbnail: thumbnail_data,
                                    preview: preview_data,
                                    file_name,
                                },
                            )
                        })
                }
                Err(err) => Err(err),
            }
        });

        // process job result
        let broadcaster = self.broadcaster.clone();
        let client = self.backend_client.clone();
        let payload_token = self.interconnect.get_sending_token()?;
        let media_endpoint = self.interconnect.backend_uri("/v1/media/queue/complete");
        tokio::runtime::Handle::current().spawn(async move {
            let result = recv.recv().await;

            let (metadata, file_size, image_data) = match result {
                Some(Ok(data)) => data,
                Some(Err(err)) => {
                    let mut b = broadcaster.lock().await;
                    b.broadcast(&file_id, QueueEvent::Err(err)).await;
                    b.drop_sub(&file_id).await;
                    return;
                }
                None => {
                    let mut b = broadcaster.lock().await;
                    b.broadcast(&file_id, QueueEvent::Done).await;
                    b.drop_sub(&file_id).await;
                    return;
                }
            };

            // call backend to update data
            let response = client
                .post(media_endpoint)
                .bearer_auth(payload_token)
                .header(X_SPACE_HEADER, space_id.to_string())
                .json(&MediaData {
                    file_id,
                    folder_id,
                    updated_date,
                    file_data: FileData {
                        file_name: image_data.file_name,
                        metadata,
                        thumbnail: image_data.thumbnail,
                        preview: image_data.preview,
                        size: file_size,
                        media_type: media_ty,
                    },
                })
                .send()
                .await;

            // validate response
            let response = match response {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        Ok(())
                    } else {
                        Err(ErrType::ServerError
                            .msg(format!("Failed to update the processed images: {:?}", status.canonical_reason())))
                    }
                }
                Err(err) => Err(ErrType::ServerError.err(err, "Failed to call backend for media updation")),
            };

            // emit event
            {
                let mut b = broadcaster.lock().await;
                match response {
                    Ok(_) => b.broadcast(&file_id, QueueEvent::Done).await,
                    Err(err) => b.broadcast(&file_id, QueueEvent::Err(err)).await,
                };
                b.drop_sub(&file_id).await;
            }
        });

        Ok(())
    }

    pub async fn subscribe_job(&self, file_id: &Uuid) -> Option<tokio::sync::broadcast::Receiver<QueueEvent>> {
        let b = self.broadcaster.lock().await;
        b.subscribe(file_id).await
    }
}

/// Extract metadata from image path
pub async fn extract_metadata(media_url: &str, file_name: &str) -> AppResult<MediaMetadata> {
    let output = {
        let cmd = format!("curl -s '{}' | {} -j -", media_url, EXIFTOOL_EXE);
        tokio::process::Command::new("sh")
            .args(["-c", cmd.as_str()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|err| ErrType::MediaError.err(err, "Failed to get exif data"))?
    };

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

    let mut metadata: MediaMetadata = serde_json::from_value(data)
        .map_err(|err| ErrType::MediaError.err(err, format!("Failed to deserialize media data: {}", file_name)))?;

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

/// Get media type [`infer::MatcherType::Image`] or [`infer::MatcherType::Video`]
/// based on `ext` extension
pub fn get_media_type(ext: &str) -> AppResult<MediaType> {
    match ext {
        // images
        "jpg" | "jpeg" | "JPG" | "JPEG" => Ok(MediaType::Image),
        "png" | "PNG" => Ok(MediaType::Image),
        "gif" | "GIF" => Ok(MediaType::Image),
        "bmp" | "BMP" => Ok(MediaType::Image),
        "heif" | "HEIF" => Ok(MediaType::Image),
        "heic" | "HEIC" => Ok(MediaType::Image),
        "avif" | "AVIF" => Ok(MediaType::Image),

        // videos
        "mp4" | "MP4" => Ok(MediaType::Video),
        "m4v" | "M4V" => Ok(MediaType::Video),
        "mkv" | "MKV" => Ok(MediaType::Video),
        "mov" | "MOV" => Ok(MediaType::Video),
        "avi" | "AVI" => Ok(MediaType::Video),
        "hevc" | "HEVC" => Ok(MediaType::Video),
        "mpg" | "MPG" | "mpeg" | "MPEG" => Ok(MediaType::Video),

        // unknown
        ext => Err(ErrType::MediaError.msg(format!("Invalid media format: {ext}"))),
    }
}
