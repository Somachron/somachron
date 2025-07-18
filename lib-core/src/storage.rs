use std::path::{Path, PathBuf};

use nanoid::nanoid;
use tokio::io::AsyncWriteExt;

use super::{config, media, r2::R2Storage, AppResult, ErrType};

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";

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

    fn clean_path<'p>(&'p self, path: &'p str) -> &'p str {
        path.trim_start_matches('/')
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

    pub async fn generate_upload_signed_url(&self, user_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path);

        let file_path = self.root_folder.join(user_id).join(file_path);
        let file_path = file_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        self.r2.generate_upload_signed_url(file_path).await
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

        // prepare r2 path
        let r2_path = self.root_folder.join(user_id).join(file_path);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        // prepare path
        let mut thumbnail_path = self.root_path.join(user_id).join(file_path);
        let file_stem = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
        thumbnail_path.set_file_name(format!("{file_stem}_thumbnail.{ext}"));

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
                let mut thumbnail_file = create_file(thumbnail_path).await?;
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
                    thumbnail_path.set_extension("jpeg");
                    let mut thumbnail_file = create_file(thumbnail_path).await?;
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
        let metadata = media::FileMetadata {
            r2_path: Some(r2_path.to_string()),
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
