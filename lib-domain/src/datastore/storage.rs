use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};
use lib_core::{
    media::MediaMetadata,
    storage::{FileData, Hash, MediaType},
    AppResult, ErrType,
};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

use crate::datastore::Datastore;

use super::DbSchema;

#[derive(Serialize, Deserialize)]
pub struct Metadata {
    pub make: Option<String>,
    pub model: Option<String>,
    pub software: Option<String>,

    pub image_height: u64,
    pub image_width: u64,

    pub duration: Option<String>,
    pub media_duration: Option<String>,
    pub frame_rate: Option<f64>,

    pub date_time: Option<surrealdb::Datetime>,
    pub iso: Option<u64>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<f64>,
    pub f_number: Option<f64>,
    pub exposure_time: Option<String>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
impl From<MediaMetadata> for Metadata {
    fn from(metadata: MediaMetadata) -> Self {
        Metadata {
            make: metadata.make,
            model: metadata.model,
            software: metadata.software.map(|v| match v {
                lib_core::media::EitherValue::Either(s) => s,
                lib_core::media::EitherValue::Or(f) => f.to_string(),
            }),
            image_height: metadata.image_height as u64,
            image_width: metadata.image_width as u64,
            duration: metadata.duration,
            media_duration: metadata.media_duration,
            frame_rate: metadata.frame_rate,
            date_time: metadata.date_time.map(|dt| surrealdb::Datetime::from(dt.0)),
            iso: metadata.iso.map(|u| u as u64),
            shutter_speed: metadata.shutter_speed.map(|v| match v {
                lib_core::media::EitherValue::Either(s) => s,
                lib_core::media::EitherValue::Or(f) => f.to_string(),
            }),
            aperture: metadata.aperture,
            f_number: metadata.f_number,
            exposure_time: metadata.exposure_time.map(|v| match v {
                lib_core::media::EitherValue::Either(s) => s,
                lib_core::media::EitherValue::Or(f) => f.to_string(),
            }),
            latitude: metadata.latitude,
            longitude: metadata.longitude,
        }
    }
}

#[derive(Deserialize)]
pub struct File {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub file_name: String,
    pub file_size: u64,
    pub media_type: MediaType,
    pub thumbnail_file_name: String,
    pub path: String,
    pub user: Option<RecordId>,
    pub space: RecordId,
    pub metadata: Metadata,
}
impl DbSchema for File {
    fn table_name() -> &'static str {
        "file"
    }
}

#[derive(Deserialize)]
pub struct FileMeta {
    pub id: RecordId,
    pub file_name: String,
    pub media_type: MediaType,
    pub user: Option<RecordId>,
}

#[derive(Deserialize)]
pub struct StreamPaths {
    pub thumbnail_path: String,
    pub original_path: String,
}

#[derive(Clone, Deserialize)]
pub struct MigrationFileData {
    pub id: RecordId,
    pub file_name: String,
    pub thumbnail_path: String,
    pub r2_path: String,
    pub space: RecordId,
}

impl Datastore {
    // ---------------------- MIGRATION

    pub async fn migrate_schema(&self) -> AppResult<()> {
        self.db
            .query(
                r#"
                DEFINE FIELD IF NOT EXISTS path ON file TYPE string;
                DEFINE FIELD IF NOT EXISTS thumbnail_file_name ON file TYPE string;
                UPDATE file SET path = '', thumbnail_file_name = '';

                DEFINE FIELD IF NOT EXISTS dir_tree ON space FLEXIBLE TYPE object;
                UPDATE space SET dir_tree = {};
            "#,
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to migrate schema"))?;
        Ok(())
    }

    pub async fn get_all_files(&self) -> AppResult<Vec<MigrationFileData>> {
        self.db.select(File::table_name()).await.map_err(|err| ErrType::DbError.err(err, "Failed to get all files"))
    }

    pub async fn migrate_file_data(
        &self,
        file_id: RecordId,
        path: String,
        thumbnail_file_name: String,
    ) -> AppResult<()> {
        self.db
            .query("UPDATE $f SET path = $p, thumbnail_file_name = $t")
            .bind(("f", file_id.clone()))
            .bind(("p", path))
            .bind(("t", thumbnail_file_name))
            .await
            .map_err(|err| ErrType::DbError.err(err, format!("Failed to set path for file: {}", file_id)))?;

        Ok(())
    }

    pub async fn migration_create_folder(&self, space_id: RecordId, path_prefix: &str, path: &str) -> AppResult<()> {
        let path_prefix = path_prefix.trim_matches('/');
        let mut depth = Vec::new();
        let mut trail = String::from(path_prefix);

        let path = std::path::PathBuf::from(path.trim_matches('/'));
        for comp in path.components().into_iter() {
            match comp {
                std::path::Component::Normal(os_str) => {
                    let path_str = os_str.to_str().unwrap();

                    trail.push('/');
                    trail.push_str(path_str);
                    let hash = Hash::new(&trail);

                    depth.push((
                        path_str.to_owned(),
                        super::space::Folder {
                            hash: hash.get(),
                            dirs: BTreeMap::new(),
                        },
                    ));
                }
                _ => (),
            };
        }

        let tree = super::space::Folder {
            hash: Hash::new(path_prefix).get(),
            dirs: depth.into_iter().rev().fold(BTreeMap::new(), |acc, (path, folder)| {
                BTreeMap::from([(
                    path,
                    super::space::Folder {
                        hash: folder.hash,
                        dirs: acc,
                    },
                )])
            }),
        };

        let res = self
            .db
            .query("UPDATE $id MERGE { dir_tree: $f }")
            .bind(("id", space_id))
            .bind(("f", tree))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create folder"))?;
        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to create folder"))?;

        Ok(())
    }

    // ---------------------- MIGRATION

    pub async fn upsert_file(&self, user_id: RecordId, space_id: RecordId, file_data: FileData) -> AppResult<File> {
        let file = match self
            .get_file_from_fields(
                space_id.clone(),
                file_data.file_name.clone(),
                file_data.folder_hash.get_ref().to_owned(),
            )
            .await?
        {
            Some(file) => self.update_file(file.id, file_data).await,
            None => self.create_file(user_id, space_id, file_data).await,
        }?;

        Ok(file)
    }

    async fn update_file(
        &self,
        file_id: RecordId,
        FileData {
            file_name,
            path,
            thumbnail_file_name,
            metadata,
            size: file_size,
            media_type,
            folder_hash,
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("UPDATE $id SET folder_hash = $f, file_name = $n, file_size = $s, media_type = $t, thumbnail_file_name = $th, path = $r, metadata = $mt")
            .bind(("id", file_id))
            .bind(("f", folder_hash.get()))
            .bind(("n", file_name))
            .bind(("s", file_size))
            .bind(("t", media_type))
            .bind(("th", thumbnail_file_name))
            .bind(("r", path))
            .bind(("mt", metadata))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create file"))?;

        let files: Vec<File> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize created file"))?;

        files.into_iter().nth(0).ok_or(ErrType::DbError.new("Failed to get created file"))
    }

    async fn create_file(
        &self,
        user_id: RecordId,
        space_id: RecordId,
        FileData {
            file_name,
            path,
            thumbnail_file_name,
            metadata,
            size: file_size,
            media_type,
            folder_hash,
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("CREATE file SET folder_hash = $f, file_name = $n, file_size = $s, media_type = $t, thumbnail_file_name = $th, path = $r, user = $u, space = $sp, metadata = $mt")
            .bind(("f", folder_hash.get()))
            .bind(("n", file_name))
            .bind(("s", file_size))
            .bind(("t", media_type))
            .bind(("th", thumbnail_file_name))
            .bind(("r", path))
            .bind(("u", user_id))
            .bind(("sp", space_id))
            .bind(("mt", metadata))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create file"))?;

        let files: Vec<File> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize created file"))?;

        files.into_iter().nth(0).ok_or(ErrType::DbError.new("Failed to get created file"))
    }

    pub async fn get_file_from_fields(
        &self,
        space_id: RecordId,
        file_name: String,
        folder_hash: String,
    ) -> AppResult<Option<File>> {
        let mut res = self
            .db
            .query("SELECT * FROM file WHERE space = $s AND folder_hash = $h AND file_name = $n")
            .bind(("s", space_id.clone()))
            .bind(("h", folder_hash))
            .bind(("n", file_name))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query for file"))?;

        let files: Vec<File> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize file"))?;

        Ok(files.into_iter().nth(0))
    }

    pub async fn get_file(&self, space_id: RecordId, file_id: &str) -> AppResult<Option<File>> {
        let file_id = File::get_id(file_id);
        let mut res = self
            .db
            .query("SELECT * FROM $id WHERE space = $s")
            .bind(("id", file_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file"))?;

        let files: Vec<_> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize file"))?;

        Ok(files.into_iter().nth(0))
    }

    pub async fn get_files(&self, space_id: RecordId, folder_hash: String) -> AppResult<Vec<FileMeta>> {
        let mut res = self
            .db
            .query("SELECT id, file_name, media_type, user FROM file WHERE space = $s AND folder_hash = $h")
            .bind(("s", space_id))
            .bind(("h", folder_hash))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query list of files"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize files"))
    }

    pub async fn get_file_stream_paths(&self, file_id: &str) -> AppResult<Option<StreamPaths>> {
        let file_id = File::get_id(file_id);

        let mut res = self
            .db
            .query(
                r#"SELECT string::concat(path, "/", thumbnail_file_name) AS thumbnail_path, string::concat(path, "/", file_name) AS original_path FROM $id"#,
            )
            .bind(("id", file_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query file path"))?;

        let paths: Vec<StreamPaths> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to get file path"))?;

        Ok(paths.into_iter().nth(0))
    }

    pub async fn create_folder(
        &self,
        space_id: RecordId,
        path_prefix: &str,
        parent_folder_hash: String,
        folder_name: String,
    ) -> AppResult<()> {
        let dir_tree = self.get_dir_tree(space_id.clone()).await?;

        let Some(path) = dir_tree.trace_path_to_parent(&parent_folder_hash) else {
            return Err(ErrType::BadRequest.new("Parent folder not found"));
        };

        let path = std::path::PathBuf::from(path.trim_matches('/'));
        let tree = path.components().into_iter().rev().fold(
            serde_json::json!({
                folder_name.as_str(): super::space::Folder {
                    hash: Hash::new(&format!("{path_prefix}/{folder_name}")).get(),
                    dirs: BTreeMap::new(),
                }
            }),
            |acc, comp| match comp {
                std::path::Component::Normal(os_str) => {
                    let path_str = os_str.to_str().unwrap();
                    serde_json::json!({
                        path_str: {
                            "dirs": acc,
                        }
                    })
                }
                _ => acc,
            },
        );

        let res = self
            .db
            .query("UPDATE $id MERGE { dir_tree: $f }")
            .bind(("id", space_id))
            .bind((
                "f",
                serde_json::json!({
                    "dirs": tree,
                }),
            ))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create folder"))?;

        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to create folder"))?;

        Ok(())
    }

    pub async fn get_dir_tree(&self, space_id: RecordId) -> AppResult<super::space::Folder> {
        let mut res = self
            .db
            .query("SELECT VALUE dir_tree FROM $id")
            .bind(("id", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query dirs"))?;

        let list: Vec<_> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize space dir_tree"))?;
        Ok(list.into_iter().nth(0).unwrap_or_default())
    }

    pub async fn get_inner_dirs(&self, space_id: RecordId, folder_hash: String) -> AppResult<Vec<(String, String)>> {
        let dirs_tree = self.get_dir_tree(space_id.clone()).await?;
        let Some((folder_path, folder_data)) = self.get_folder_from_hash(dirs_tree, folder_hash) else {
            return Err(ErrType::BadRequest.new("No folder found"));
        };

        let mut results = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back((folder_path, folder_data, 0));

        while let Some((folder_path, folder_data, depth)) = queue.pop_front() {
            results.push((folder_path.clone(), folder_data.hash, depth));

            for (subfolder_path, subfolder_data) in folder_data.dirs.into_iter() {
                queue.push_back((format!("{folder_path}/{subfolder_path}"), subfolder_data, depth + 1));
            }
        }

        // sort in inner most as first order
        results.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));

        Ok(results.into_iter().map(|(p, h, _)| (p, h)).collect())
    }

    pub async fn delete_folder(&self, space_id: RecordId, path: &str) -> AppResult<()> {
        if !path.trim().trim_matches('/').is_empty() {
            let mut query = String::from("UPDATE $id SET dir_tree");
            self.get_query_for_path(&mut query, path);
            query.push_str(" = NONE");

            let res = self
                .db
                .query(query)
                .bind(("id", space_id))
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed to query delete folder"))?;
            res.check().map_err(|err| ErrType::DbError.err(err, "Failed to delete files for folder"))?;
        }

        Ok(())
    }

    pub async fn delete_files(&self, space_id: RecordId, folder_hash: String) -> AppResult<()> {
        let res = self
            .db
            .query("DELETE file WHERE space = $s AND folder_hash = $h")
            .bind(("s", space_id.clone()))
            .bind(("h", folder_hash))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query delete files"))?;
        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to delete files for folder"))?;

        Ok(())
    }

    pub async fn delete_file(&self, folder_id: RecordId) -> AppResult<()> {
        let _: Option<File> =
            self.db.delete(folder_id).await.map_err(|err| ErrType::DbError.err(err, "Failed to query delete file"))?;
        Ok(())
    }

    /// Function that creates nested access for folder in query
    ///
    /// example:
    ///     `a/b/c` => query+`.dirs["a"].dirs["b"].dirs["c"]`
    fn get_query_for_path(&self, query: &mut String, path: &str) {
        let path = std::path::PathBuf::from(&path);
        for comp in path.components().into_iter() {
            match comp {
                std::path::Component::Normal(os_str) => {
                    let path_str = os_str.to_str().unwrap();

                    query.push_str(".dirs[\"");
                    query.push_str(path_str);
                    query.push_str("\"]");
                }
                _ => (),
            };
        }
    }

    /// Get path and folder data from dir_tree for folder_hash
    fn get_folder_from_hash(&self, tree: super::space::Folder, hash: String) -> Option<(String, super::space::Folder)> {
        let mut queue = VecDeque::new();
        queue.push_back(("".to_owned(), tree));

        while let Some((path, folder)) = queue.pop_front() {
            if folder.hash == hash {
                return Some((path, folder));
            }

            for (p, f) in folder.dirs.into_iter() {
                if f.hash == hash {
                    return Some((p, f));
                }
                queue.push_back((p, f));
            }
        }

        None
    }
}
