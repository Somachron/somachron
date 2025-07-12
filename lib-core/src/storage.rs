use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{config, r2::R2Storage, AppError, AppResult, ErrType};

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    r2_path: Option<String>,
}

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";

/// Manage storage operations
pub struct Storage {
    /// /mounted/volume/ROOT_DATA
    root_path: PathBuf,

    /// R2 client
    r2: R2Storage,
}

impl Storage {
    pub async fn new() -> Self {
        let volume_path = Path::new(config::get_volume_path());

        // create necessary volumes
        let root_path = volume_path.join(ROOT_DATA);
        Self::create_dir(root_path.join(SPACES_PATH)).await.unwrap();

        Self {
            root_path,
            r2: R2Storage::new(),
        }
    }

    async fn create_dir(dir: impl AsRef<Path>) -> AppResult<()> {
        tokio::fs::create_dir_all(dir.as_ref())
            .await
            .map_err(|err| AppError::err(ErrType::FsError, err, "Failed to create dir"))
    }

    pub async fn validate_user_drive(&self, user_id: &str) -> AppResult<()> {
        let user_dir = self.root_path.join(user_id);
        Self::create_dir(user_dir).await
    }
}
