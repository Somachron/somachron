use futures::StreamExt;
use lib_core::{storage::Storage, AppResult, ErrType};

use crate::datastore::{space::SpaceDs, storage::StorageDs, Datastore};

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

    pub async fn update_thumbnail_sizes(&self, storage: &Storage) -> AppResult<()> {
        let spaces = self.ds.get_all_spaces().await?;

        let mut file_list = Vec::new();
        for space in spaces.iter() {
            let files = self.ds.list_files_gallery(&space.id).await?;
            file_list.extend(files.into_iter().map(|f| (space.id, f)));
        }

        let total_files = file_list.len();
        dbg!(total_files);
        let jobs = file_list.into_iter().enumerate().map(|(i, (space_id, file))| async move {
            let paths = self.ds.get_file_stream_paths(&space_id, file.0.id).await?.expect("Failed to get stream path");

            let Some(folder) = self.ds.get_folder(&space_id, &file.0.folder).await? else {
                return Err(ErrType::BadRequest.msg("Folder not found"));
            };

            dbg!(&paths.original_path);
            let data = storage.process_upload_completion(&space_id.to_string(), &paths.original_path, 0).await?;
            for data in data.into_iter() {
                let _ = self.ds.upsert_file(&file.0.user.unwrap(), &space_id, &folder, file.0.updated_at, data).await?;
            }
            println!("{}/{}", i, total_files);
            Ok(())
        });

        let res = futures::stream::iter(jobs).buffer_unordered(11).collect::<Vec<_>>().await;
        for r in res {
            r?;
        }

        Ok(())
    }
}
