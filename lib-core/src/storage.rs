use std::path::{Path, PathBuf};

use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use utoipa::ToSchema;

use super::{config, media, r2::R2Storage, AppResult, ErrType};

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";
const FS_TAG: &str = "fs::";

#[derive(Serialize, Deserialize, ToSchema)]
pub struct FileMetadata {
    pub file_name: String,
    pub r2_path: String,
    pub thumbnail_path: String,
    pub metadata: serde_json::Value,
    pub size: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum FileEntry {
    Dir {
        tag: String,
        name: String,
    },
    File {
        tag: String,
        data: FileMetadata,
    },
}

/// Manage storage operations
///
/// Mimic the file structure from [`R2Storage`] in attached volume
pub struct Storage {
    /// /mounted/volume/[`ROOT_DATA`]
    root_path: PathBuf,

    /// Root folder for R2: [`ROOT_DATA`],
    root_folder: PathBuf,

    /// R2 client
    r2: R2Storage,
}

async fn create_dir(dir: impl AsRef<Path>) -> AppResult<()> {
    tokio::fs::create_dir_all(dir.as_ref()).await.map_err(|err| ErrType::FsError.err(err, "Failed to create dir"))
}

async fn create_file(file_path: impl AsRef<Path>) -> AppResult<tokio::fs::File> {
    tokio::fs::File::create(file_path.as_ref())
        .await
        .map_err(|err| ErrType::FsError.err(err, "Failed to create/truncate file"))
}

async fn remove_file(file_path: impl AsRef<Path>) -> AppResult<()> {
    tokio::fs::remove_file(file_path).await.map_err(|err| ErrType::FsError.err(err, "Failed to remove file"))
}

impl Storage {
    pub async fn new() -> Self {
        let volume_path = Path::new(config::get_volume_path());

        // create necessary volumes
        let root_path = volume_path.join(ROOT_DATA);
        create_dir(root_path.join(SPACES_PATH)).await.unwrap();

        Self {
            root_path,
            root_folder: PathBuf::from(ROOT_DATA),
            r2: R2Storage::new(),
        }
    }

    async fn get_tmp_file(&self, user_id: &str) -> AppResult<(tokio::fs::File, PathBuf)> {
        let tmp_dir_path = self.root_path.join(user_id).join("tmp");
        create_dir(&tmp_dir_path).await?;

        let id = nanoid!(8);
        let tmp_file_path = tmp_dir_path.join(format!("tmp_f_{id}"));
        create_file(&tmp_file_path).await.map(|f| (f, tmp_file_path))
    }

    /// Cleans path for fs operations
    ///
    /// * Remove `/` from start and end
    /// * Remove [`FS_TAG`] from start
    /// * Replace `..` with empty from start and end
    fn clean_path(&self, path: &str) -> String {
        path.trim_start_matches(FS_TAG).replace("..", "").trim_matches('/').to_owned()
    }

    pub async fn validate_user_drive(&self, user_id: &str) -> AppResult<()> {
        let user_dir = self.root_path.join(user_id);
        create_dir(user_dir).await
    }

    /// Creates folder for `user_id`
    ///
    /// * `folder_path`: some/existing/path/new_folder
    pub async fn create_folder(&self, user_id: &str, folder_path: &str) -> AppResult<()> {
        let folder_path = self.clean_path(folder_path);

        let folder_path = self.root_folder.join(user_id).join(folder_path);
        let folder_path = folder_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;
        self.r2.create_folder(folder_path).await?;

        let folder_path = self.root_path.join(user_id).join(folder_path);
        create_dir(folder_path).await
    }

    /// Generate presigned URL for uploading image
    ///
    /// To be used by frontend
    pub async fn generate_upload_signed_url(&self, user_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path);

        let file_path = self.root_folder.join(user_id).join(file_path);
        let file_path = file_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        self.r2.generate_upload_signed_url(file_path).await
    }

    /// List items in the `dir` path
    ///
    /// * Skips `tmp`
    /// * Skips `.*` files
    /// * Processes only `*.json` files
    ///
    /// Returns vec [`FileEntry`]
    pub async fn list_dir(&self, user_id: &str, dir: &str) -> AppResult<Vec<FileEntry>> {
        let dir = self.clean_path(dir);
        let dir = dir.trim_start_matches(&['f', 's', ':', ':']);
        let dir_path = self.root_path.join(user_id).join(dir);

        let mut rd = tokio::fs::read_dir(&dir_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to read dir: {:?}", dir_path)))?;

        let mut entries = Vec::<FileEntry>::new();

        while let Some(dir) = rd.next_entry().await.map_err(|err| ErrType::FsError.err(err, "Failed to iter dir"))? {
            let file_name = dir.file_name();
            let file_name =
                file_name.to_str().ok_or(ErrType::FsError.new(format!("Failed to get file_name: {:?}", dir_path)))?;

            match file_name {
                // skip tmp files
                x if x.starts_with("tmp") => continue,

                // skip hidden files
                x if x.starts_with('.') => continue,
                _ => (),
            };

            let ft = dir.file_type().await.map_err(|err| ErrType::FsError.err(err, "Failed to get file type"))?;

            if ft.is_dir() {
                entries.push(FileEntry::Dir {
                    tag: "dir".to_owned(),
                    name: file_name.to_owned(),
                });
                continue;
            }

            if ft.is_file() {
                let path = dir.path();
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

                if ext.ends_with("json") {
                    let mut file = tokio::fs::File::open(&path)
                        .await
                        .map_err(|err| ErrType::FsError.err(err, format!("Failed to open file: {:?}", path)))?;

                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)
                        .await
                        .map_err(|err| ErrType::FsError.err(err, format!("Failed to read file: {:?}", path)))?;

                    let data: FileMetadata = serde_json::from_slice(&buf)
                        .map_err(|err| ErrType::FsError.err(err, "Failed to deserialize metadata"))?;

                    entries.push(FileEntry::File {
                        tag: "file".to_owned(),
                        data,
                    });
                }

                continue;
            }
        }

        Ok(entries)
    }

    /// Get requested file from filesystem
    pub async fn get_file(
        &self,
        user_id: &str,
        file_path: &str,
    ) -> AppResult<(tokio_util::io::ReaderStream<tokio::fs::File>, String)> {
        let file_path = self.clean_path(file_path);
        let fs_path = self.root_path.join(user_id).join(&file_path);
        let ext = fs_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        if !fs_path.exists() {
            return Err(ErrType::NotFound.new(format!("File not found: {file_path}")));
        }
        if ext.ends_with("json") {
            return Err(ErrType::BadRequest.new(format!("Invalid file requested: {file_path}")));
        }

        let file = tokio::fs::File::open(&fs_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to open file: {file_path}")))?;

        let stream = tokio_util::io::ReaderStream::new(file);
        Ok((stream, ext.to_owned()))
    }

    /// Process the uploaded media
    ///
    /// * prepares the directory in mounted volume
    /// * download the media from R2
    /// * create and save thumbnail
    /// * extract and save exif data
    pub async fn process_upload_skeleton_thumbnail_media(
        &self,
        user_id: &str,
        file_path: &str,
        file_size: usize,
    ) -> AppResult<()> {
        let file_path = self.clean_path(&file_path);
        let file_path = file_path.as_str();

        // prepare media directory
        let media_path = self.root_path.join(user_id).join(file_path);
        if let Some(parent) = media_path.parent() {
            create_dir(parent).await?;
        }

        // get file extension
        let ext = media_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;
        let file_name = media_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without name"))?;

        // prepare r2 path
        let r2_path = self.root_folder.join(user_id).join(file_path);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        // prepare path
        let mut thumbnail_path = self.root_path.join(user_id).join(file_path);
        let file_stem = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let mut thumbnail_file_name = PathBuf::from(format!("{file_stem}_thumbnail.{ext}"));
        thumbnail_path.set_file_name(&thumbnail_file_name);

        // process thumbnail and metadata
        let media_type = media::get_media_type(ext);
        let metadata = match media_type {
            infer::MatcherType::Image => {
                // download file from R2
                let image_bytes = self.r2.download_photo(r2_path).await?;

                // prepare tmp file
                let (mut tmp_file, tmp_path) = self.get_tmp_file(user_id).await?;
                tmp_file
                    .write_all(&image_bytes)
                    .await
                    .map_err(|err| ErrType::FsError.err(err, "Failed to write tmp image file"))?;

                // process image data
                let exif_data = media::extract_metadata(&tmp_path)?;
                let thumbnail_bytes = media::create_thumbnail(image_bytes, None, &exif_data)?;

                // save thumbnail
                let mut thumbnail_file = create_file(&thumbnail_path).await?;
                thumbnail_file
                    .write_all(&thumbnail_bytes)
                    .await
                    .map_err(|err| ErrType::FsError.err(err, "Failed to save thumbnail file"))?;

                remove_file(tmp_path).await?;

                // return metadata
                exif_data
            }
            infer::MatcherType::Video => {
                // download initial chunk from R2
                let video_bytes = self.r2.download_video(r2_path).await?;

                // prepare tmp file
                let (mut tmp_file, tmp_path) = self.get_tmp_file(user_id).await?;
                tmp_file
                    .write_all(&video_bytes)
                    .await
                    .map_err(|err| ErrType::FsError.err(err, "Failed to write tmp media file"))?;

                // process thumbnail
                if let Some(thumbnail_bytes) = media::process_video_thumbnail(&tmp_path)? {
                    // set thumbnail extension to jpeg
                    thumbnail_path.set_extension("jpeg");
                    thumbnail_file_name.set_extension("jpeg");

                    let mut thumbnail_file = create_file(&thumbnail_path).await?;
                    thumbnail_file
                        .write_all(&thumbnail_bytes)
                        .await
                        .map_err(|err| ErrType::FsError.err(err, "Failed to write thumbnail file"))?;
                }

                let metadata = media::extract_metadata(&tmp_path)?;

                remove_file(tmp_path).await?;

                // return metadata
                metadata
            }
            _ => return Err(ErrType::FsError.new(format!("Invalid media extension: {ext}"))),
        };

        // prepare path
        let mut metadata_path = self.root_path.join(user_id).join(file_path);
        metadata_path.set_extension(format!("{ext}.json"));

        // serialize metadata to vec
        let metadata = FileMetadata {
            file_name: file_name.to_owned(),
            r2_path: r2_path.to_string(),
            thumbnail_path: {
                let mut path = PathBuf::from(file_path);
                path.set_file_name(thumbnail_file_name);
                path.to_str().map(|s| s.to_owned()).unwrap()
            },
            metadata,
            size: file_size,
        };
        let metadata_bytes =
            serde_json::to_vec(&metadata).map_err(|err| ErrType::FsError.err(err, "Failed to serialize metadata"))?;

        // save metadata
        let mut metadata_file = create_file(metadata_path).await?;
        metadata_file
            .write_all(&metadata_bytes)
            .await
            .map_err(|err| ErrType::FsError.err(err, "Failed to write metadata bytes"))
    }
}
