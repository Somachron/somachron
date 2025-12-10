use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use aws_sdk_s3::primitives::ByteStream;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

use super::{config, media, s3::S3Storage, AppResult, ErrType};

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    Video,
}

pub struct FileData {
    pub file_name: String,
    pub thumbnail_file_name: String,
    pub metadata: media::MediaMetadata,
    pub size: i64,
    pub media_type: MediaType,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
}

/// Manage storage operations
///
/// Mimic the file structure from [`R2Storage`] in attached volume
pub struct Storage {
    /// /mounted/volume/[`ROOT_DATA`]
    root_path: PathBuf,

    /// Root folder for R2: [`ROOT_DATA`]/[`SPACES_PATH`],
    r2_spaces: PathBuf,

    /// R2 client
    r2: S3Storage,
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
        let root_path = volume_path.join(ROOT_DATA);
        create_dir(&root_path).await.unwrap();

        Self {
            root_path,
            r2_spaces: PathBuf::from(ROOT_DATA).join(SPACES_PATH),
            r2: S3Storage::new(),
        }
    }

    async fn save_tmp_file(&self, space_id: &str, mut bytes_stream: ByteStream) -> AppResult<PathBuf> {
        let tmp_dir_path = self.root_path.join(space_id).join("tmp");
        create_dir(&tmp_dir_path).await?;

        let id = nanoid!(8);
        let tmp_file_path = tmp_dir_path.join(format!("tmp_f_{id}"));
        {
            let tmp_file = create_file(&tmp_file_path).await?;
            let mut buf_writer = tokio::io::BufWriter::new(tmp_file);

            while let Some(chunk) = bytes_stream.next().await {
                let bytes = chunk.map_err(|err| ErrType::R2Error.err(err, "Failed to read next chunk stream"))?;

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
    /// * Remove [`FS_TAG`] from start
    /// * Replace `..` with empty from start and end
    pub fn clean_path(&self, path: &str) -> AppResult<String> {
        let path = urlencoding::decode(path).map_err(|err| ErrType::FsError.err(err, "Invalid path"))?;
        Ok(path.replace("..", "").trim_matches('/').to_owned())
    }

    /// Get path prefix
    pub fn get_spaces_path(&self, space_id: &str) -> String {
        let path = self.r2_spaces.join(space_id);
        path.to_str().map(|s| s.trim_matches('/').to_owned()).unwrap_or_default()
    }

    /// Creates space folder
    pub async fn create_space_folder(&self, space_id: &str) -> AppResult<()> {
        let r2_path = self.r2_spaces.join(space_id);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from folder path"))?;
        self.r2.create_folder(r2_path).await
    }

    /// Generate presigned URL for uploading media
    ///
    /// To be used by frontend
    pub async fn generate_upload_signed_url(&self, space_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path)?;

        let file_path = self.r2_spaces.join(space_id).join(file_path);
        let file_path = file_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from file path"))?;

        self.r2.generate_upload_signed_url(file_path).await
    }

    /// Generate presigned URL for steaming media
    ///
    /// To be used by frontend
    pub async fn generate_download_signed_url(&self, space_id: &str, path: &str) -> AppResult<String> {
        let path = self.clean_path(path)?;
        let path = self.r2_spaces.join(space_id).join(path);
        self.r2.generate_download_signed_url(path.to_str().unwrap()).await
    }

    /// Process the uploaded media
    ///
    /// * prepares the directory in mounted volume
    /// * download the media from R2
    /// * create and save thumbnail
    /// * extract metadata
    pub async fn process_upload_completion(
        &self,
        space_id: &str,
        file_path: &str,
        file_size: usize,
    ) -> AppResult<Vec<FileData>> {
        let file_path = self.clean_path(file_path)?;
        let file_path = file_path.as_str();

        // prepare r2 path
        let r2_path = self.r2_spaces.join(space_id).join(file_path);
        let mut r2_thumbnail = r2_path.clone();

        // get file extension
        let ext = r2_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.msg("Invalid file path without extenstion"))?;

        let r2_path =
            r2_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from file path"))?.trim_matches('/');

        // prepare path
        let file_name =
            r2_thumbnail.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.msg("No file name"))?.to_owned();
        r2_thumbnail.set_file_name(format!("thumbnail_{file_name}"));

        // process thumbnail and metadata
        let media_type = media::get_media_type(ext);
        let bytes_stream = self.r2.download_media(r2_path).await?;
        let tmp_path = self.save_tmp_file(space_id, bytes_stream).await?;

        let file_size = if file_size == 0 {
            tmp_path.metadata().map(|m| m.size() as usize).unwrap_or(file_size)
        } else {
            file_size
        };

        if media_type == infer::MatcherType::Video {
            r2_thumbnail.set_extension("jpeg");
        }

        // extract media metadata
        let metadata_result = self.process_media(space_id, file_path, ext, &tmp_path, &r2_thumbnail, media_type).await;
        let _ = remove_file(&tmp_path).await;
        let (metadata, paths) = metadata_result?;

        let thumbnail_file_name = r2_thumbnail.file_name().and_then(|s| s.to_str()).unwrap().to_owned();

        let all_metadata = paths
            .into_iter()
            .map(|(width, height, processed_file_name, processed_thumbnail_file_name)| FileData {
                file_name: processed_file_name.unwrap_or(file_name.to_owned()),
                thumbnail_file_name: processed_thumbnail_file_name.unwrap_or(thumbnail_file_name.clone()),
                metadata: metadata.clone(),
                size: file_size as i64,
                media_type: match media_type {
                    infer::MatcherType::Video => MediaType::Video,
                    _ => MediaType::Image,
                },
                thumbnail_width: width,
                thumbnail_height: height,
            })
            .collect();

        Ok(all_metadata)
    }

    async fn process_media(
        &self,
        space_id: &str,
        file_path: &str,
        ext: &str,
        tmp_path: &PathBuf,
        r2_thumbnail: &Path,
        media_type: infer::MatcherType,
    ) -> AppResult<(media::MediaMetadata, Vec<(u32, u32, Option<String>, Option<String>)>)> {
        let metadata = media::extract_metadata(tmp_path).await?;

        let path = PathBuf::from(file_path);
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap();
        let thumbnail_file_name = r2_thumbnail.file_stem().and_then(|s| s.to_str()).unwrap();
        let r2_path = self.r2_spaces.join(space_id).join(file_path);

        let mut media_data = Vec::new();

        // create thumbnail
        let thumb_op = media::run_thumbnailer(tmp_path, media_type, &metadata).await?;
        match thumb_op.heif_paths {
            Some(paths) => {
                for (i, tmp_path) in paths.into_iter().enumerate() {
                    let tmp_path = PathBuf::from(tmp_path);
                    let mut tmp_thumbnail_path = tmp_path.clone();
                    let tmp_thumbnail_file =
                        tmp_thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap().to_owned();
                    tmp_thumbnail_path.set_file_name(format!("{tmp_thumbnail_file}.jpeg"));

                    let (file_name, thumbnail_file_name) =
                        (format!("{file_name}_{i}.{ext}"), format!("{thumbnail_file_name}_{i}.jpeg"));

                    let mut r2_path = r2_path.clone();
                    r2_path.set_file_name(&file_name);
                    let r2_path = r2_path.to_str().unwrap();

                    let mut r2_thumbnail = r2_thumbnail.to_path_buf();
                    r2_thumbnail.set_file_name(&thumbnail_file_name);
                    let r2_thumbnail = r2_thumbnail.to_str().unwrap();

                    self.r2.upload_photo(r2_path, &tmp_path).await?;
                    self.r2.upload_photo(r2_thumbnail, &tmp_thumbnail_path).await?;
                    let _ = remove_file(&tmp_path).await;
                    let _ = remove_file(&tmp_thumbnail_path).await;

                    media_data.push((thumb_op.width, thumb_op.height, Some(file_name), Some(thumbnail_file_name)));
                }
            }
            None => {
                let r2_thumbnail = r2_thumbnail.to_str().unwrap();
                self.r2.upload_photo(r2_thumbnail, tmp_path).await?;
                media_data.push((thumb_op.width, thumb_op.height, None, None));
            }
        };

        Ok((metadata, media_data))
    }

    /// Matches over spaces path
    ///
    /// Returns `is_dir` and cleaned `path`
    pub fn delete_path_type(&self, space_id: &str, path: &str) -> AppResult<(String, bool)> {
        let path = self.clean_path(path)?;
        let fs_path = self.r2_spaces.join(space_id).join(&path);
        Ok((path, fs_path.extension().is_none()))
    }

    pub async fn delete_folder(&self, space_id: &str, dir_path: &str) -> AppResult<()> {
        let path = self.clean_path(dir_path)?;

        let mut r2_path = self.r2_spaces.join(space_id).join(path);
        if r2_path.extension().is_some() {
            r2_path.set_file_name("");
        }
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.msg("Failed to get str from folder path"))?;

        self.r2.delete_folder(r2_path).await
    }

    pub async fn delete_file(&self, r2_file: String, r2_thumbnail: String) -> AppResult<()> {
        self.r2.delete_key(&r2_file).await?;
        self.r2.delete_key(&r2_thumbnail).await?;
        Ok(())
    }
}
