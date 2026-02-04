use std::path::PathBuf;

use super::{AppResult, ErrType};

pub mod s3;

const ROOT_FOLDER: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";

/// Manage storage operations
///
/// Mimic the file structure from [`S3Storage`] in attached volume
pub struct Storage {
    /// Root folder for S3: [`ROOT_FOLDER`]/[`SPACES_PATH`],
    spaces_path: PathBuf,

    /// S3 client
    s3: s3::S3Storage,
}

impl Storage {
    pub async fn new() -> Self {
        Self {
            spaces_path: PathBuf::from(ROOT_FOLDER).join(SPACES_PATH),
            s3: s3::S3Storage::new(),
        }
    }

    /// Cleans path for fs operations
    ///
    /// * Remove `/` from start and end
    /// * Replace `..` with empty from start and end
    pub fn clean_path(&self, path: &str) -> AppResult<String> {
        let path = urlencoding::decode(path)
            .map(|c| c.into_owned())
            .map_err(|err| ErrType::FsError.err(err, "Invalid path"))?;
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

    pub fn get_remote_path(&self, space_id: &str, path: &str) -> AppResult<String> {
        let file_path = self.clean_path(path)?;
        self.spaces_path
            .join(space_id)
            .join(file_path)
            .to_str()
            .map(|s| s.to_owned())
            .ok_or(ErrType::FsError.msg("Failed to get remote path"))
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
