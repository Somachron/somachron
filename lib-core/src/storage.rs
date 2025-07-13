use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use super::{config, media, r2::R2Storage, AppError, AppResult, ErrType};

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
    tokio::fs::create_dir_all(dir.as_ref())
        .await
        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to create dir"))
}

async fn create_file(file_path: impl AsRef<Path>) -> AppResult<tokio::fs::File> {
    tokio::fs::File::create(file_path.as_ref())
        .await
        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to create/truncate file"))
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
        let folder_path =
            folder_path.to_str().ok_or(AppError::new(ErrType::FsError, "Failed to get str from folder path"))?;
        self.r2.create_folder(folder_path).await?;

        let folder_path = self.root_path.join(user_id).join(folder_path);
        create_dir(folder_path).await
    }

    pub async fn generate_upload_signed_url(&self, user_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path);

        let file_path = self.root_folder.join(user_id).join(file_path);
        let file_path =
            file_path.to_str().ok_or(AppError::new(ErrType::FsError, "Failed to get str from file path"))?;

        self.r2.generate_upload_signed_url(file_path).await
    }

    /// Process the uploaded media
    ///
    /// * prepares the directory in mounted volume
    /// * download the media from R2
    /// * create and save thumbnail
    /// * extract and save exif data
    pub async fn process_upload_skeleton_thumbnail_media(&self, user_id: &str, file_path: &str) -> AppResult<()> {
        // prepare media directory
        let media_path = self.root_path.join(user_id).join(file_path);
        if let Some(parent) = media_path.parent() {
            create_dir(parent).await?;
        }

        // get file extension
        let ext = media_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(AppError::new(ErrType::FsError, "Invalid file path without extenstion"))?;

        // prepare r2 path
        let r2_path = self.root_folder.join(user_id).join(file_path);
        let r2_path = r2_path.to_str().ok_or(AppError::new(ErrType::FsError, "Failed to get str from file path"))?;

        let media_type = media::get_media_type(ext);
        match media_type {
            infer::MatcherType::Image => {
                // download file from R2
                let bytes = self.r2.download_photo(r2_path).await?;
                let file_size = bytes.len();

                // process image data
                let exif_data = media::extract_exif_data(&bytes).await?;
                let thumbnail_bytes = media::process_image_thumnail(bytes, &exif_data).await?;

                // save thumbnail
                {
                    // prepare path
                    let mut thumbnail_path = self.root_path.join(user_id).join(file_path);
                    let file_stem = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
                    thumbnail_path.set_file_name(format!("{file_stem}_thumbnail.{ext}"));

                    // write to file
                    let mut thumbnail_file = create_file(thumbnail_path).await?;
                    thumbnail_file
                        .write_all(&thumbnail_bytes)
                        .await
                        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to save thumbnail file"))?;
                }

                // save metadata
                {
                    // prepare path
                    let mut metadata_path = self.root_path.join(user_id).join(file_path);
                    metadata_path.set_extension(format!("{ext}.json"));

                    // serialize metadata to vec
                    let metadata = media::FileMetadata {
                        r2_path: Some(r2_path.to_string()),
                        exif: exif_data,
                        size: file_size,
                    };
                    let metadata_bytes = serde_json::to_vec(&metadata)
                        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to serialize metadata"))?;

                    // save metadata
                    let mut metadata_file = create_file(media_path).await?;
                    metadata_file
                        .write_all(&metadata_bytes)
                        .await
                        .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to write metadata bytes"))?
                }
            }
            infer::MatcherType::Video => {
                todo!("video impl");
                // let bytes = self.r2.download_video(r2_path).await?;
            }
            _ => return Err(AppError::new(ErrType::FsError, format!("Invalid media extension: {ext}"))),
        };

        Ok(())
    }
}
