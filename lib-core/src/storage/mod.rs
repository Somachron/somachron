use std::path::{Path, PathBuf};

use aws_sdk_s3::primitives::ByteStream;
use nanoid::nanoid;
use tokio::io::AsyncWriteExt;

use crate::ErrorContext;

use super::{config, AppResult, ErrType};

pub mod media;
mod s3;

const ROOT_FOLDER: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";

pub struct FileData {
    pub file_name: String,
    pub thumbnail: media::ImageMeta,
    pub preview: media::ImageMeta,
    pub metadata: media::MediaMetadata,
    pub size: i64,
    pub media_type: media::MediaType,
}

/// Manage storage operations
///
/// Mimic the file structure from [`S3Storage`] in attached volume
pub struct Storage {
    /// /mounted/volume/[`ROOT_SPACES`]
    root_path: PathBuf,

    /// Root folder for S3: [`ROOT_SPACES`]/[`SPACES_PATH`],
    spaces_path: PathBuf,

    /// S3 client
    s3: s3::S3Storage,
}

async fn create_dir(dir: &PathBuf) -> AppResult<()> {
    tokio::fs::create_dir_all(&dir).await.map_err(|err| ErrType::FsError.err(err, "Failed to create dir"))
}

async fn create_file(file_path: &PathBuf) -> AppResult<tokio::fs::File> {
    tokio::fs::File::create(&file_path).await.map_err(|err| ErrType::FsError.err(err, "Failed to create/truncate file"))
}

async fn remove_file(file_path: &PathBuf) -> AppResult<()> {
    tokio::fs::remove_file(file_path).await.map_err(|err| ErrType::FsError.err(err, "Failed to remove file"))
}

impl Storage {
    pub async fn new() -> Self {
        let volume_path = config::get_volume_path();
        let volume_path = Path::new(&volume_path);

        // create necessary volumes
        let root_path = volume_path.join(ROOT_FOLDER);
        create_dir(&root_path).await.unwrap();

        Self {
            root_path,
            spaces_path: PathBuf::from(ROOT_FOLDER).join(SPACES_PATH),
            s3: s3::S3Storage::new(),
        }
    }

    async fn new_tmp_file_path(&self, space_id: &str) -> AppResult<PathBuf> {
        let tmp_dir_path = self.root_path.join(space_id).join("tmp");
        create_dir(&tmp_dir_path).await?;

        let id = nanoid!(8);
        Ok(tmp_dir_path.join(format!("tmp_f_{id}")))
    }

    async fn save_tmp_file(&self, space_id: &str, mut bytes_stream: ByteStream) -> AppResult<PathBuf> {
        let tmp_file_path = self.new_tmp_file_path(space_id).await?;
        {
            let tmp_file = create_file(&tmp_file_path).await?;
            let mut buf_writer = tokio::io::BufWriter::new(tmp_file);

            while let Some(chunk) = bytes_stream.next().await {
                let bytes = chunk.map_err(|err| ErrType::S3Error.err(err, "Failed to read next chunk stream"))?;

                buf_writer
                    .write_all(&bytes)
                    .await
                    .map_err(|err| ErrType::FsError.err(err, "Failed to write tmp media file"))?;
            }
            let _ = buf_writer.flush().await;
        }

        Ok(tmp_file_path)
    }

    /// Cleans path for fs operations
    ///
    /// * Remove `/` from start and end
    /// * Replace `..` with empty from start and end
    pub fn clean_path(&self, path: &str) -> AppResult<String> {
        let path = urlencoding::decode(path).map_err(|err| ErrType::FsError.err(err, "Invalid path"))?;
        Ok(path.replace("..", "").trim_matches('/').to_owned())
    }

    /// Creates space folder
    pub async fn create_space_folder(&self, space_id: &str) -> AppResult<()> {
        let remote_path = self.spaces_path.join(space_id);
        let remote_path = remote_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from folder path"))?;
        self.s3.create_folder(remote_path).await
    }

    /// Generate presigned URL for uploading media
    ///
    /// To be used by frontend
    pub async fn generate_upload_signed_url(&self, space_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path)?;

        let file_path = self.spaces_path.join(space_id).join(file_path);
        let file_path = file_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from file path"))?;

        self.s3.generate_upload_signed_url(file_path).await
    }

    /// Generate presigned URL for steaming media
    ///
    /// To be used by frontend
    pub async fn generate_stream_signed_url(&self, space_id: &str, path: &str) -> AppResult<String> {
        let path = self.clean_path(path)?;
        let path = self.spaces_path.join(space_id).join(path);
        self.s3.generate_stream_signed_url(path.to_str().unwrap()).await
    }

    /// Process the uploaded media
    ///
    /// * prepares the directory in mounted volume
    /// * download the media from S3
    /// * create and save thumbnail
    /// * extract metadata
    pub async fn process_upload_completion(
        &self,
        space_id: &str,
        file_path: &str,
        file_size: usize,
    ) -> AppResult<FileData> {
        let file_path = self.clean_path(file_path)?;
        let file_path = file_path.as_str();

        // prepare r2 path
        let remote_path = self.spaces_path.join(space_id).join(file_path);

        let file_name =
            remote_path.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.msg("No file name"))?.to_owned();

        // get file extension
        let ext = remote_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.msg("Invalid file path without extenstion"))?;

        let remote_path =
            remote_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from file path"))?.trim_matches('/');

        // process thumbnail and metadata
        let media_size = self.s3.head_object(remote_path).await?;
        let media_type = media::get_media_type(ext)?;
        let process_type = match media_type {
            media::MediaType::Image => {
                let bytes_stream = self.s3.download_media(remote_path).await?;
                let tmp_path = self.save_tmp_file(space_id, bytes_stream).await?;
                media::MediaProcessType::Image {
                    path: tmp_path,
                    file_size: media_size,
                }
            }
            media::MediaType::Video => {
                let stream_url = self.s3.generate_stream_signed_url(remote_path).await?;
                let tmp_path = self.new_tmp_file_path(space_id).await?;
                media::MediaProcessType::Video {
                    url: stream_url,
                    name: file_name,
                    tmp_path,
                    file_size: media_size,
                }
            }
        };

        // extract media metadata
        let metadata_result = self.process_media(space_id, file_path, &process_type).await;
        if let media::MediaProcessType::Image {
            path,
            ..
        } = process_type
        {
            let _ = remove_file(&path).await;
        }
        let (metadata, processed_meta) = metadata_result?;

        let all_metadata = FileData {
            file_name: processed_meta.file_name,
            metadata,
            size: file_size as i64,
            media_type,
            thumbnail: processed_meta.thumbnail,
            preview: processed_meta.preview,
        };

        Ok(all_metadata)
    }

    async fn process_media(
        &self,
        space_id: &str,
        file_path: &str,
        process_type: &media::MediaProcessType,
    ) -> AppResult<(media::MediaMetadata, media::ProcessedMeta)> {
        let remote_path = self.spaces_path.join(space_id).join(file_path);
        let src_file_stem = remote_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let src_file_name = remote_path.file_name().and_then(|s| s.to_str()).map(|s| s.to_owned()).unwrap();

        let metadata = media::extract_metadata(process_type).await?;

        // create thumbnail
        let thumbnail_output::ProcessedImage {
            thumbnail,
            preview,
        } = media::run_thumbnailer(process_type, &metadata).await?;

        let thumbnail_file_name = format!("thumbnail_{src_file_stem}.jpeg");
        let mut remote_thumbnail = remote_path.clone();
        remote_thumbnail.set_file_name(&thumbnail_file_name);
        self.s3
            .upload_photo(remote_thumbnail.to_str().unwrap(), &thumbnail.path)
            .await
            .context("uploading thumbnail")?;
        remove_file(&thumbnail.path).await?;

        let preview_file_name = format!("preview_{src_file_stem}.jpeg");
        let mut remote_preview = remote_path.clone();
        remote_preview.set_file_name(&preview_file_name);
        self.s3.upload_photo(remote_preview.to_str().unwrap(), &preview.path).await.context("uploading preview")?;
        remove_file(&preview.path).await?;

        let media_data = media::ProcessedMeta {
            thumbnail: media::ImageMeta {
                width: thumbnail.width as i32,
                height: thumbnail.height as i32,
                file_name: thumbnail_file_name,
            },
            preview: media::ImageMeta {
                width: preview.width as i32,
                height: preview.height as i32,
                file_name: preview_file_name,
            },
            file_name: src_file_name,
        };

        Ok((metadata, media_data))
    }

    pub async fn delete_folder(&self, space_id: &str, dir_path: &str) -> AppResult<()> {
        let path = self.clean_path(dir_path)?;

        let mut remote_path = self.spaces_path.join(space_id).join(path);
        if remote_path.extension().is_some() {
            remote_path.set_file_name("");
        }
        let remote_path = remote_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from folder path"))?;

        self.s3.delete_folder(remote_path).await
    }

    pub async fn delete_file(
        &self,
        space_id: &str,
        remote_file: String,
        remote_thumbnail: String,
        remote_preview: Option<String>,
    ) -> AppResult<()> {
        let remote_file = self.clean_path(&remote_file)?;
        let remote_file = self.spaces_path.join(space_id).join(remote_file);
        self.s3.delete_key(remote_file.to_str().unwrap()).await?;

        let remote_thumbnail = self.clean_path(&remote_thumbnail)?;
        let remote_thumbnail = self.spaces_path.join(space_id).join(remote_thumbnail);
        self.s3.delete_key(remote_thumbnail.to_str().unwrap()).await?;

        if let Some(remote_preview) = remote_preview {
            let remote_preview = self.clean_path(&remote_preview)?;
            let remote_preview = self.spaces_path.join(space_id).join(remote_preview);
            self.s3.delete_key(remote_preview.to_str().unwrap()).await?;
        }
        Ok(())
    }
}
