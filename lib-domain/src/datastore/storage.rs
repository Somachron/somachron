use chrono::{DateTime, Utc};
use lib_core::{
    media::MediaMetadata,
    storage::{FileData, Hash, MediaType},
    AppResult, ErrType,
};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

use crate::datastore::Datastore;

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

impl Datastore {
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

    pub async fn get_files(&self, space_id: RecordId, folder_hash: Hash) -> AppResult<Vec<File>> {
        let mut res = self
            .db
            .query("SELECT * FROM file WHERE space = $s AND folder_hash = $h")
            .bind(("s", space_id))
            .bind(("h", folder_hash.get()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query list of files"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize files"))
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
