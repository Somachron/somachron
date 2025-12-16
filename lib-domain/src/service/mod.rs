use futures::StreamExt;
use lib_core::{storage::Storage, AppError, AppResult};

use crate::datastore::storage::StorageDs;

use super::datastore::Datastore;

mod auth;
mod cloud;
mod space;
mod user;
mod user_space;

pub type AppService = Service<Datastore>;

pub struct Service<D> {
    ds: D,
}

impl Service<Datastore> {
    pub async fn new() -> Self {
        Self {
            ds: Datastore::connect().await,
        }
    }

    pub fn ds(&self) -> &Datastore {
        &self.ds
    }

    pub async fn migrate_previews(&self, storage: &Storage) -> AppResult<()> {
        let files = self.ds.get_all_files().await?;

        let total = files.len();

        for (i, file) in files.into_iter().filter(|f| f.metadata.preview_meta.is_none()).enumerate() {
            let file_path = std::path::PathBuf::from(&file.path).join(&file.node_name);
            println!("processing: {file_path:?}");

            let folder = self.ds.get_folder(&file.space_id, &file.parent_node.unwrap()).await?.unwrap();
            let space_id = file.space_id.to_string();

            let file_data = storage
                .process_upload_completion(space_id.as_str(), file_path.to_str().unwrap(), file.node_size as usize)
                .await?;

            for data in file_data.into_iter() {
                let _ =
                    self.ds.upsert_file(&file.user_id.unwrap(), &file.space_id, &folder, file.updated_at, data).await?;
            }

            println!("{i}/{total}");
        }

        Ok(())
    }
}
