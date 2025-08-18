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
        if storage.migration_exists().await {
            dbg!("Migration exists");
            return;
        }
        if let Err(err) = storage.migration_lock().await {
            dbg!(err);
            return;
        }

        // upload all thumbnails
        let files = self.ds.get_all_files().await.unwrap();
        let mut done = 0;
        let total = files.len();
        for files_chunk in files.chunks(16).into_iter() {
            let mut handles = Vec::with_capacity(16);

            for file in files_chunk.into_iter() {
                let file = file.clone();
                let storage = storage.clone();

                handles.push(tokio::spawn(async move {
                    if let Err(err) =
                        storage.upload_thumbnail(&file.space.id(), &file.file_name, &file.thumbnail_path).await
                    {
                        dbg!(err);
                    }
                    file
                }));
            }

            for handle in handles.into_iter() {
                let file = handle.await.unwrap();
                done += 1;
                let progress = format!("{done}/{total}: {}", file.thumbnail_path);
                dbg!(progress);
            }
        }

        // migrate schema
        if let Err(err) = self.ds.migrate_schema().await {
            dbg!(err);
        }

        // migrate folders
        let spaces = self.ds.get_all_spaces().await.expect("No spaces ?");
        for space in spaces.into_iter() {
            match storage.list_dir(&space.id.id()).await {
                Ok((path_prefix, dirs)) => {
                    for dir_path in dirs.into_iter() {
                        if let Err(err) = self
                            .ds
                            .migration_create_folder(
                                space.id.clone(),
                                path_prefix.to_str().unwrap(),
                                dir_path.to_str().unwrap(),
                            )
                            .await
                        {
                            dbg!(err);
                        }
                    }
                }
                Err(err) => {
                    dbg!(err);
                }
            };
        }

        // update folder path
        for file in files.into_iter() {
            let mut path = std::path::PathBuf::from(&file.r2_path);
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap().to_owned();
            if let Some(_) = path.extension() {
                path.set_file_name("");
            }

            let thumbnail_path = std::path::PathBuf::from(&file.thumbnail_path);
            let thumbnail_ext = thumbnail_path.extension().and_then(|s| s.to_str()).unwrap().to_owned();

            if let Err(err) = self
                .ds
                .migrate_file_data(
                    file.id,
                    path.to_str().unwrap().trim_matches('/').to_owned(),
                    format!("thumbnail_{file_stem}.{thumbnail_ext}"),
                )
                .await
            {
                dbg!(err);
            }
        }
    }
}
