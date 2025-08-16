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
    pub thumbnail_path: String,
    pub r2_path: String,
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
            .query("DEFINE FIELD path ON TABLE file TYPE string")
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to migrate schema"))?;
        Ok(())
    }

    pub async fn get_all_files(&self) -> AppResult<Vec<MigrationFileData>> {
        self.db.select(File::table_name()).await.map_err(|err| ErrType::DbError.err(err, "Failed to get all files"))
    }

    pub async fn set_file_path(&self, file_id: RecordId, path: String) -> AppResult<()> {
        self.db
            .query("UPDATE $f SET path = $p;")
            .bind(("f", file_id.clone()))
            .bind(("p", path))
            .await
            .map_err(|err| ErrType::DbError.err(err, format!("Failed to set path for file: {}", file_id)))?;

        Ok(())
    }

    // ---------------------- MIGRATION

    pub async fn upsert_file(&self, user_id: RecordId, space_id: RecordId, file_data: FileData) -> AppResult<File> {
        let mut res = self
            .db
            .query("SELECT * FROM file WHERE space = $s AND folder_hash = $h AND file_name = $n")
            .bind(("s", space_id.clone()))
            .bind(("h", file_data.folder_hash.get_ref().to_owned()))
            .bind(("n", file_data.file_name.clone()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query for file"))?;

        let files: Vec<File> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize file"))?;

        let file = match files.into_iter().nth(0) {
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
            r2_path,
            thumbnail_path,
            metadata,
            size: file_size,
            media_type,
            file_hash,
            folder_hash,
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("UPDATE $id SET folder_hash = $f, file_hash = $fh, file_name = $n, file_size = $s, media_type = $t, thumbnail_path = $th, r2_path = $r, metadata = $mt")
            .bind(("id", file_id))
            .bind(("f", folder_hash.get()))
            .bind(("fh", file_hash.get()))
            .bind(("n", file_name))
            .bind(("s", file_size))
            .bind(("t", media_type))
            .bind(("th", thumbnail_path))
            .bind(("r", r2_path))
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
            r2_path,
            thumbnail_path,
            metadata,
            size: file_size,
            media_type,
            file_hash,
            folder_hash,
        }: FileData,
    ) -> AppResult<File> {
        let metadata = Metadata::from(metadata);

        let mut res = self
            .db
            .query("CREATE file SET folder_hash = $f, file_hash = $fh, file_name = $n, file_size = $s, media_type = $t, thumbnail_path = $th, r2_path = $r, user = $u, space = $sp, metadata = $mt")
            .bind(("f", folder_hash.get()))
            .bind(("fh", file_hash.get()))
            .bind(("n", file_name))
            .bind(("s", file_size))
            .bind(("t", media_type))
            .bind(("th", thumbnail_path))
            .bind(("r", r2_path))
            .bind(("u", user_id))
            .bind(("sp", space_id))
            .bind(("mt", metadata))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query create file"))?;

        let files: Vec<File> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize created file"))?;

        files.into_iter().nth(0).ok_or(ErrType::DbError.new("Failed to get created file"))
    }

    pub async fn get_files(&self, space_id: RecordId, folder_hash: Hash) -> AppResult<Vec<FileMeta>> {
        let mut res = self
            .db
            .query("SELECT id, file_name, media_type, user FROM file WHERE space = $s AND folder_hash = $h")
            .bind(("s", space_id))
            .bind(("h", folder_hash.get()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query list of files"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize files"))
    }

    pub async fn get_file_thumbnail(&self, file_id: &str) -> AppResult<Option<String>> {
        let file_id = File::get_id(file_id);

        let mut res = self
            .db
            .query("SELECT VALUE thumbnail_path FROM $id")
            .bind(("id", file_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query file thumbnail path"))?;

        let paths: Vec<String> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to get file thumbnail path"))?;

        Ok(paths.into_iter().nth(0))
    }

    pub async fn get_file_r2(&self, file_id: &str) -> AppResult<Option<String>> {
        let file_id = File::get_id(file_id);

        let mut res = self
            .db
            .query("SELECT VALUE r2_path FROM $id")
            .bind(("id", file_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query file r2 path"))?;

        let paths: Vec<String> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to get file r2 path"))?;

        Ok(paths.into_iter().nth(0))
    }

    pub async fn delete_folder(&self, space_id: RecordId, folder_hash: Hash) -> AppResult<()> {
        let res = self
            .db
            .query("DELETE file WHERE space = $s AND folder_hash = $h")
            .bind(("s", space_id))
            .bind(("h", folder_hash.get()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query delete files"))?;
        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to delete files for folder"))?;
        Ok(())
    }

    pub async fn delete_file(&self, space_id: RecordId, file_hash: Hash, folder_hash: Hash) -> AppResult<()> {
        let res = self
            .db
            .query("DELETE file WHERE space = $s AND folder_hash = $h AND file_hash = $fh")
            .bind(("s", space_id))
            .bind(("h", folder_hash.get()))
            .bind(("fh", file_hash.get()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query delete file"))?;
        res.check().map_err(|err| ErrType::DbError.err(err, "Failed to delete file"))?;
        Ok(())
    }
}
