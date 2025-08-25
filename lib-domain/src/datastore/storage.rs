use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};
use lib_core::{
    media::MediaMetadata,
    storage::{FileData, Hash, MediaType},
    AppResult, ErrType,
};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

use crate::{datastore::Datastore, extension::IdStr};

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
pub struct Folder {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub name: String,
}
impl DbSchema for Folder {
    fn table_name() -> &'static str {
        "folder"
    }
}

#[derive(Deserialize)]
pub struct FolderTree {
    pub id: RecordId,
    pub name: String,

    pub next: Vec<FolderTree>,
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
    pub folder_hash: String,
    pub thumbnail_path: String,
    pub r2_path: String,
    pub space: RecordId,
}

pub enum FsLink {
    File(RecordId),
    Folder(RecordId),
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

                DEFINE TABLE folder TYPE NORMAL SCHEMAFULL;
                DEFINE FIELD created_at ON TABLE folder TYPE datetime DEFAULT time::now();
                DEFINE FIELD updated_at ON TABLE folder TYPE datetime VALUE time::now();
                DEFINE FIELD name ON TABLE folder TYPE string ASSERT string::len($value) > 0;
                DEFINE FIELD space ON TABLE folder TYPE record<space>;
                DEFINE INDEX unique_folder ON TABLE folder COLUMNS name UNIQUE;

                DEFINE TABLE fs TYPE RELATION IN folder OUT folder|file ENFORCED SCHEMAFULL;
                DEFINE FIELD created_at ON TABLE fs TYPE datetime DEFAULT time::now();
                DEFINE INDEX unique_link ON TABLE fs COLUMNS in, out UNIQUE;

                DEFINE FIELD IF NOT EXISTS folder ON space TYPE record<folder>;
            "#,
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to migrate schema"))?;
        Ok(())
    }

    pub async fn cleanup_schema(&self) -> AppResult<()> {
        self.db
            .query(
                r#"
                REMOVE FIELD IF EXISTS r2_path ON file;
                REMOVE FIELD IF EXISTS thumbnail_path ON file;
                REMOVE FIELD IF EXISTS file_hash ON file;
                REMOVE FIELD IF EXISTS folder_hash ON file;
                UPDATE file SET r2_path = NONE, thumbnail_path = NONE, file_hash = NONE, folder_hash = NONE;
            "#,
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to migrate schema"))?;
        Ok(())
    }

    pub async fn create_space_root_folder(&self, space_id: RecordId) -> AppResult<RecordId> {
        let mut res = self
            .db
            .query("CREATE folder SET name = $n, space = $s")
            .bind(("n", space_id.id()))
            .bind(("s", space_id.clone()))
            .await
            .map_err(|err| ErrType::DbError.err(err, format!("Failed to create space folder: {}", space_id)))?;

        let folders: Vec<Folder> = res
            .take(0)
            .map_err(|err| ErrType::DbError.err(err, format!("Failed to create space root folder: {}", space_id)))?;

        let Some(folder) = folders.into_iter().nth(0) else {
            return Err(ErrType::DbError.new("No created folders found"));
        };

        let _ = self
            .db
            .query("UPDATE $id SET folder = $f")
            .bind(("id", space_id))
            .bind(("f", folder.id.clone()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to set folder for space"))?;

        Ok(folder.id)
    }

    pub async fn get_all_files(&self) -> AppResult<Vec<MigrationFileData>> {
        self.db.select(File::table_name()).await.map_err(|err| ErrType::DbError.err(err, "Failed to get all files"))
    }

    pub async fn migrate_file_data(
        &self,
        folder_id: RecordId,
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

        self.fs_link(folder_id, FsLink::File(file_id)).await
    }

    pub async fn migrate_folder_paths(
        &self,
        space_id: RecordId,
        path_prefix: std::path::PathBuf,
        root_folder_id: RecordId,
        dirs: Vec<std::path::PathBuf>,
    ) -> AppResult<BTreeMap<String, RecordId>> {
        let mut ids = BTreeMap::<String, RecordId>::new();
        let mut hash_ids = BTreeMap::<String, RecordId>::new();
        let trail = path_prefix.to_str().unwrap();
        hash_ids.insert(Hash::new(trail).get(), root_folder_id.clone());

        for path in dirs.into_iter() {
            let mut trail = String::from(trail);
            let mut prev_folder_id = root_folder_id.clone();

            for comp in path.components().into_iter() {
                match comp {
                    std::path::Component::Normal(os_str) => {
                        let path_str = os_str.to_str().unwrap();

                        trail.push('/');
                        trail.push_str(path_str);
                        let hash = Hash::new(&trail);

                        let folder_id = match ids.get(path_str) {
                            Some(id) => id.clone(),
                            None => {
                                let folder = self
                                    .create_orphan_folder(space_id.clone(), path_str.to_owned())
                                    .await?
                                    .expect("Should have folder id");
                                folder.id
                            }
                        };
                        ids.insert(path_str.to_owned(), folder_id.clone());
                        hash_ids.insert(hash.get(), folder_id.clone());

                        self.fs_link(prev_folder_id.clone(), FsLink::Folder(folder_id.clone())).await?;

                        prev_folder_id = folder_id;
                    }
                    _ => (),
                };
            }
        }

        Ok(hash_ids)
    }

    // ---------------------- MIGRATION

    pub async fn upsert_file(
        &self,
        user_id: RecordId,
        space_id: RecordId,
        folder_id: String,
        file_data: FileData,
    ) -> AppResult<File> {
        let file = match self.get_file_from_fields(space_id.clone(), file_data.file_name.clone(), folder_id).await? {
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
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("UPDATE $id SET file_name = $n, file_size = $s, media_type = $t, thumbnail_file_name = $th, path = $r, metadata = $mt")
            .bind(("id", file_id))
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
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("CREATE file SET file_name = $n, file_size = $s, media_type = $t, thumbnail_file_name = $th, path = $r, user = $u, space = $sp, metadata = $mt")
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
        folder_id: String,
    ) -> AppResult<Option<File>> {
        let folder_id = Folder::get_id(&folder_id);

        let mut res = self
            .db
            .query("SELECT * FROM file WHERE <-fs<-folder[WHERE id = $f AND space = $s] AND file_name = $n")
            .bind(("s", space_id.clone()))
            .bind(("f", folder_id))
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

    pub async fn get_files(&self, space_id: RecordId, folder_id: String) -> AppResult<Vec<FileMeta>> {
        let folder_id = Folder::get_id(&folder_id);

        let mut res = self
            .db
            .query(r#"SELECT VALUE out.{id, file_name, media_type, user} FROM $f->fs[WHERE record::tb(out) == "file" AND in.space = $s]"#)
            .bind(("s", space_id))
            .bind(("f", folder_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query list of files"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize files"))
    }

    pub async fn get_file_stream_paths(&self, space_id: RecordId, file_id: &str) -> AppResult<Option<StreamPaths>> {
        let file_id = File::get_id(file_id);

        let mut res = self
            .db
            .query(
                r#"SELECT string::concat(path, "/", thumbnail_file_name) AS thumbnail_path, string::concat(path, "/", file_name) AS original_path FROM $id WHERE space = $s"#,
            )
            .bind(("id", file_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query file path"))?;

        let paths: Vec<StreamPaths> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to get file path"))?;

        Ok(paths.into_iter().nth(0))
    }

    pub async fn create_orphan_folder(&self, space_id: RecordId, folder_name: String) -> AppResult<Option<Folder>> {
        let mut res = self
            .db
            .query("CREATE folder SET name = $n, space = $s")
            .bind(("n", folder_name))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create orphan folder"))?;

        let folders: Vec<_> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to create orphan folder"))?;
        Ok(folders.into_iter().nth(0))
    }

    pub async fn create_folder(&self, space_id: RecordId, folder_id: String, folder_name: String) -> AppResult<()> {
        let folder_id = Folder::get_id(&folder_id);
        let Some(parent_folder) = self.get_folder(space_id.clone(), folder_id).await? else {
            return Err(ErrType::BadRequest.new("Parent folder not found"));
        };

        let Some(new_folder) = self.create_orphan_folder(space_id, folder_name).await? else {
            return Err(ErrType::DbError.new("Failed to get created folder"));
        };

        self.fs_link(parent_folder.id, FsLink::Folder(new_folder.id)).await
    }

    pub async fn fs_link(&self, parent_folder_id: RecordId, fs_id: FsLink) -> AppResult<()> {
        let res = self
            .db
            .query("RELATE $p->fs->$n")
            .bind(("p", parent_folder_id))
            .bind((
                "n",
                match fs_id {
                    FsLink::File(record_id) => record_id,
                    FsLink::Folder(record_id) => record_id,
                },
            ))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to link folders"))?;

        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to link created folder"))?;

        Ok(())
    }

    pub async fn get_folder(&self, space_id: RecordId, folder_id: RecordId) -> AppResult<Option<Folder>> {
        let mut res = self
            .db
            .query("SELECT * FROM $f WHERE space = $s")
            .bind(("f", folder_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to fetch folder by ID"))?;

        let folders: Vec<_> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to get folder by id"))?;
        Ok(folders.into_iter().nth(0))
    }

    pub async fn list_folder(&self, space_id: RecordId, folder_id: String) -> AppResult<Vec<Folder>> {
        let folder_id = Folder::get_id(&folder_id);

        let mut res = self
            .db
            .query(r#"SELECT VALUE out.* FROM $f->fs[WHERE record::tb(out) == "folder" AND in.space = $s]"#)
            .bind(("f", folder_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query list folders"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to list folders"))
    }

    /// Returns [`Option`] of [`FolderTree`] for `folder_id`
    async fn get_inner_folders(&self, space_id: RecordId, folder_id: RecordId) -> AppResult<Option<FolderTree>> {
        let mut res = self
            .db
            .query(r#"SELECT @.{..}.{ id, name, next: ->fs[WHERE record::tb(out) == "folder"].out.@ } FROM $f WHERE space = $s"#)
            .bind(("f", folder_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get child tree for folder"))?;
        let folder_tree: Vec<FolderTree> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize dir tree"))?;
        Ok(folder_tree.into_iter().nth(0))
    }

    /// Trace path to root
    /// to be used as pefix for deletion
    ///
    /// Eg:
    /// ```
    ///     /---b----e
    ///       \  `-c  `-g
    ///        `-d  `-f
    /// ```
    ///
    /// Querying for `f` should return (`b/c`, `f`);
    /// which implies, do delete `f`, we need `b/c` prefix to delete
    /// - `b/c/f`
    /// - `b/c/f/*`
    pub async fn trace_path_root(&self, space_id: RecordId, folder_id: String) -> AppResult<Option<(String, String)>> {
        let folder_id = Folder::get_id(&folder_id);

        let mut res = self
            .db
            .query("SELECT @.{..}.{ id, name, next: <-fs.in.@ } FROM $f WHERE space = $s")
            .bind(("f", folder_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get parent tree for folder"))?;
        let folder_tree: Vec<FolderTree> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize dir tree"))?;

        let Some(folder) = folder_tree.into_iter().nth(0) else {
            return Ok(None);
        };

        let queried_folder_name = folder.name.clone();

        // fact: all the `next` will be of length 1 as per graph
        // root -> children, but child -> single parent
        let mut queue = VecDeque::new();
        queue.push_back((folder.name, folder.next));

        let mut paths = Vec::new();
        while let Some((folder_name, dirs)) = queue.pop_front() {
            paths.push(folder_name);

            for dir in dirs.into_iter() {
                queue.push_back((dir.name, dir.next));
            }
        }
        // remove root folder
        let _ = paths.pop();
        paths.reverse();
        // remove queried folder
        let _ = paths.pop();

        Ok(Some((paths.join("/"), queried_folder_name)))
    }

    pub async fn get_inner_folder_paths(
        &self,
        space_id: RecordId,
        folder_id: String,
    ) -> AppResult<Vec<(String, RecordId)>> {
        let Some((mut parent_path, _)) = self.trace_path_root(space_id.clone(), folder_id.clone()).await? else {
            return Err(ErrType::DbError.new("Failed to trace parent path"));
        };

        let folder_id = Folder::get_id(&folder_id);

        let Some(folder_tree) = self.get_inner_folders(space_id.clone(), folder_id.clone()).await? else {
            return Err(ErrType::BadRequest.new("No folder found"));
        };

        let mut paths = Vec::<(String, RecordId)>::new();
        let mut queue = VecDeque::new();

        parent_path.push('/');
        parent_path.push_str(&folder_tree.name);
        queue.push_back((parent_path, folder_tree.id, folder_tree.next));

        while let Some((folder_path, folder_id, dirs)) = queue.pop_front() {
            paths.push((folder_path.clone(), folder_id));

            for subfolder in dirs.into_iter() {
                let mut folder_path = folder_path.clone();
                folder_path.push('/');
                folder_path.push_str(&subfolder.name);
                queue.push_back((folder_path, subfolder.id, subfolder.next));
            }
        }

        // reverse the paths => inner to outer most order
        paths.reverse();

        Ok(paths)
    }

    pub async fn delete_folder(&self, space_id: RecordId, folder_id: RecordId) -> AppResult<()> {
        let res = self
            .db
            .query(
                r#"
                DELETE $id->fs.out WHERE space = $s;
                DELETE $id WHERE space = $s;
            "#,
            )
            .bind(("id", folder_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query delete folder"))?;
        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to delete files for folder"))?;

        Ok(())
    }

    pub async fn delete_file(&self, file_id: RecordId) -> AppResult<()> {
        let _: Option<File> =
            self.db.delete(file_id).await.map_err(|err| ErrType::DbError.err(err, "Failed to query delete file"))?;
        Ok(())
    }
}
