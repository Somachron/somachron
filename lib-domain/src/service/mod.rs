use std::sync::Arc;

use lib_core::storage::Storage;

use crate::{datastore::Datastore, extension::IdStr};

mod auth;
mod cloud;
mod space;
mod user;
mod user_space;

pub struct Service {
    ds: Datastore,
}

impl Service {
    pub async fn new() -> Self {
        Self {
            ds: Datastore::connect().await,
        }
    }

    pub fn ds(&self) -> &Datastore {
        &self.ds
    }

    pub async fn migrate_thumbnails(&self, storage: Arc<Storage>) {
        // upload all thumbnails
        let files = self.ds.get_all_files().await.unwrap();
        let mut done = 0;
        let total = files.len();
        for files_chunk in files.chunks(8).into_iter() {
            let mut handles = Vec::new();

            for file in files_chunk.into_iter() {
                let file = file.clone();
                let storage = storage.clone();

                handles.push(tokio::spawn(async move {
                    if let Err(err) =
                        storage.upload_thumbnail(&file.space.id(), &file.file_name, &file.thumbnail_path).await
                    {
                        dbg!(err);
                    }
                }));
            }

            for handle in handles.into_iter() {
                handle.await.unwrap();
                done += 1;
                dbg!(done, total);
            }
        }

        // migrate schema
        if let Err(err) = self.ds.migrate_schema().await {
            dbg!(err);
        }

        // recalculate and update folder hashes
        for file in files.into_iter() {
            let mut path = std::path::PathBuf::from(&file.r2_path);
            path.set_file_name("");

            if let Err(err) = self.ds.set_file_path(file.id, path.to_str().unwrap().trim_matches('/').to_owned()).await
            {
                dbg!(err);
            }
        }
    }
}
