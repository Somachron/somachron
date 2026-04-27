use chrono::{DateTime, Utc};
use lib_core::{
    smq_dto::{
        res::{FileData, ImageData},
        MediaMetadata, MediaType,
    },
    AppResult, ErrType,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

    pub date_time: Option<DateTime<Utc>>,
    pub iso: Option<u64>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<f64>,
    pub f_number: Option<f64>,
    pub exposure_time: Option<String>,

    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
impl Metadata {
    fn from(metadata: MediaMetadata, updated_date: DateTime<Utc>) -> Self {
        Metadata {
            make: metadata.make,
            model: metadata.model,
            software: metadata.software.map(|v| match v {
                lib_core::smq_dto::EitherValue::Either(s) => s,
                lib_core::smq_dto::EitherValue::Or(f) => f.to_string(),
            }),
            image_height: metadata.image_height as u64,
            image_width: metadata.image_width as u64,
            duration: metadata.duration,
            media_duration: metadata.media_duration,
            frame_rate: metadata.frame_rate,
            date_time: metadata.date_time.map(|dt| dt.0).or(Some(updated_date)),
            iso: metadata.iso.map(|u| u as u64),
            shutter_speed: metadata.shutter_speed.map(|v| match v {
                lib_core::smq_dto::EitherValue::Either(s) => s,
                lib_core::smq_dto::EitherValue::Or(f) => f.to_string(),
            }),
            aperture: metadata.aperture,
            f_number: metadata.f_number,
            exposure_time: metadata.exposure_time.map(|v| match v {
                lib_core::smq_dto::EitherValue::Either(s) => s,
                lib_core::smq_dto::EitherValue::Or(f) => f.to_string(),
            }),
            latitude: metadata.latitude,
            longitude: metadata.longitude,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct NodeMetadata {
    pub thumbnail_meta: Option<ImageData>,
    pub preview_meta: Option<ImageData>,
    pub file_meta: Option<Metadata>,
    pub media_type: Option<MediaType>,
}
impl<'a> tokio_postgres::types::FromSql<'a> for NodeMetadata {
    fn from_sql(
        _ty: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        serde_json::from_slice(&raw[1..]).map_err(Into::into)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        matches!(*ty, tokio_postgres::types::Type::JSONB)
    }
}
impl NodeMetadata {
    pub fn jsonb(
        thumbnail_meta: ImageData,
        preivew_meta: ImageData,
        file_meta: Metadata,
        media_type: MediaType,
    ) -> AppResult<serde_json::Value> {
        let meta = Self {
            thumbnail_meta: Some(thumbnail_meta),
            preview_meta: Some(preivew_meta),
            file_meta: Some(file_meta),
            media_type: Some(media_type),
        };
        serde_json::to_value(&meta).map_err(|err| ErrType::FsError.err(err, "Failed to serialize metadata"))
    }
}

pub struct MediaFile {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub user_id: Uuid,
    pub space_id: Uuid,
    pub hash: String,
    pub file_name: String,
    pub object_key: String,
    pub thumbnail_key: Option<String>,
    pub preview_key: Option<String>,
    pub node_size: i64,
    pub metadata: NodeMetadata,
}
impl TryFrom<tokio_postgres::Row> for MediaFile {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.try_get(0)?,
            created_at: value.try_get(1)?,
            updated_at: value.try_get(2)?,
            user_id: value.try_get(3)?,
            space_id: value.try_get(4)?,
            hash: value.try_get(5)?,
            file_name: value.try_get(6)?,
            object_key: value.try_get(7)?,
            thumbnail_key: value.try_get(8)?,
            preview_key: value.try_get(9)?,
            node_size: value.try_get(10)?,
            metadata: value.try_get(11)?,
        })
    }
}

pub struct Album {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub user_id: Uuid,
    pub space_id: Uuid,
    pub name: String,
    pub legacy_path: String,
}
impl TryFrom<tokio_postgres::Row> for Album {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.try_get(0)?,
            created_at: value.try_get(1)?,
            updated_at: value.try_get(2)?,
            user_id: value.try_get(3)?,
            space_id: value.try_get(4)?,
            name: value.try_get(5)?,
            legacy_path: value.try_get(6)?,
        })
    }
}

pub struct FileMeta {
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub file_name: String,
    pub media_type: MediaType,
    pub user: Option<Uuid>,
    pub width: i32,
    pub height: i32,
}
impl TryFrom<tokio_postgres::Row> for FileMeta {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        let meta: NodeMetadata = value.get(11);
        let (width, height) = meta.thumbnail_meta.map(|m| (m.width, m.height)).unwrap_or((0, 0));
        Ok(Self {
            id: value.try_get(0)?,
            updated_at: value.try_get(2)?,
            file_name: value.try_get(6)?,
            media_type: meta.media_type.unwrap_or(MediaType::Image),
            user: value.try_get(3).ok(),
            width,
            height,
        })
    }
}

pub struct GalleryFileMeta(pub FileMeta);
impl TryFrom<tokio_postgres::Row> for GalleryFileMeta {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        let media_type: String = value.try_get(4)?;
        Ok(Self(FileMeta {
            id: value.try_get(0)?,
            updated_at: value.try_get(1)?,
            user: value.try_get(2).ok(),
            file_name: value.try_get(3)?,
            media_type: serde_json::from_value(serde_json::Value::String(media_type)).unwrap_or(MediaType::Image),
            width: value.try_get(5)?,
            height: value.try_get(6)?,
        }))
    }
}

pub struct StreamKeys {
    pub thumbnail_key: Option<String>,
    pub preview_key: Option<String>,
}
impl TryFrom<tokio_postgres::Row> for StreamKeys {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            thumbnail_key: value.try_get(0)?,
            preview_key: value.try_get(1)?,
        })
    }
}

pub struct StreamKey {
    pub key: String,
}
impl TryFrom<tokio_postgres::Row> for StreamKey {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            key: value.try_get(0)?,
        })
    }
}

pub trait StorageDs: Send + Sync {
    fn get_or_create_file(
        &self,
        user_id: &Uuid,
        space_id: &Uuid,
        file_hash: &str,
        file_name: String,
        object_key: String,
        updated_date: DateTime<Utc>,
        file_data: FileData,
    ) -> impl Future<Output = AppResult<MediaFile>> + Send;

    fn update_file(
        &self,
        file_id: Uuid,
        space_id: &Uuid,
        updated_date: DateTime<Utc>,
        file_data: FileData,
        thumbnail_key: Option<String>,
        preview_key: Option<String>,
    ) -> impl Future<Output = AppResult<MediaFile>> + Send;

    fn get_file(&self, space_id: Uuid, file_id: Uuid) -> impl Future<Output = AppResult<Option<MediaFile>>> + Send;
    fn list_files(&self, space_id: &Uuid, album_id: &Uuid) -> impl Future<Output = AppResult<Vec<FileMeta>>> + Send;
    fn list_files_gallery(&self, space_id: &Uuid) -> impl Future<Output = AppResult<Vec<GalleryFileMeta>>> + Send;
    fn get_thumbnail_preview_stream_keys(
        &self,
        space_id: &Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<Option<StreamKeys>>> + Send;
    fn get_download_stream_key(
        &self,
        space_id: &Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<Option<String>>> + Send;

    fn create_album(
        &self,
        user_id: &Uuid,
        space_id: Uuid,
        album_name: String,
    ) -> impl Future<Output = AppResult<Album>> + Send;
    fn get_album(&self, space_id: &Uuid, album_id: &Uuid) -> impl Future<Output = AppResult<Option<Album>>> + Send;
    fn list_albums(&self, space_id: Uuid) -> impl Future<Output = AppResult<Vec<Album>>> + Send;

    fn link_album_files(
        &self,
        space_id: &Uuid,
        album_id: &Uuid,
        file_ids: &[Uuid],
    ) -> impl Future<Output = AppResult<()>> + Send;
    fn unlink_album_files(
        &self,
        space_id: &Uuid,
        album_id: &Uuid,
        file_ids: &[Uuid],
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn delete_album(&self, space_id: &Uuid, album_id: &Uuid) -> impl Future<Output = AppResult<()>> + Send;
    fn delete_file(&self, file_id: &Uuid, space_id: &Uuid) -> impl Future<Output = AppResult<()>> + Send;
}

impl StorageDs for Datastore {
    async fn get_or_create_file(
        &self,
        user_id: &Uuid,
        space_id: &Uuid,
        file_hash: &str,
        file_name: String,
        object_key: String,
        updated_date: DateTime<Utc>,
        file_data: FileData,
    ) -> AppResult<MediaFile> {
        let metadata = NodeMetadata::jsonb(
            file_data.thumbnail,
            file_data.preview,
            Metadata::from(file_data.metadata, updated_date),
            file_data.media_type,
        )?;

        let row = self
            .db
            .query_one(
                &self.storage_stmts.upsert_media_file,
                &[
                    &Uuid::now_v7(),
                    &updated_date,
                    user_id,
                    space_id,
                    &file_hash,
                    &file_name,
                    &object_key,
                    &file_data.size,
                    &metadata,
                ],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get or create file by hash"))?;

        MediaFile::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse file by hash"))
    }

    async fn update_file(
        &self,
        file_id: Uuid,
        space_id: &Uuid,
        updated_date: DateTime<Utc>,
        FileData {
            file_name,
            metadata,
            size: file_size,
            media_type,
            thumbnail,
            preview,
        }: FileData,
        thumbnail_key: Option<String>,
        preview_key: Option<String>,
    ) -> AppResult<MediaFile> {
        let file_meta = Metadata::from(metadata, updated_date);
        let metadata = NodeMetadata::jsonb(thumbnail, preview, file_meta, media_type)?;

        let row = self
            .db
            .query_one(
                &self.storage_stmts.update_media_file,
                &[&file_id, &space_id, &file_name, &file_size, &metadata, &updated_date, &thumbnail_key, &preview_key],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update file"))?;

        MediaFile::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse updated file"))
    }

    async fn get_file(&self, space_id: Uuid, file_id: Uuid) -> AppResult<Option<MediaFile>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_media_file, &[&file_id, &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file by id"))?;

        match rows.into_iter().next() {
            Some(row) => MediaFile::try_from(row)
                .map(Some)
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse file by id")),
            None => Ok(None),
        }
    }

    async fn list_files(&self, space_id: &Uuid, album_id: &Uuid) -> AppResult<Vec<FileMeta>> {
        let rows = self
            .db
            .query(&self.storage_stmts.list_album_media_files, &[album_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get files"))?;

        let size = rows.len();
        rows.into_iter().try_fold(Vec::with_capacity(size), |mut acc, row| {
            let f = FileMeta::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse listed files"))?;
            acc.push(f);
            Ok(acc)
        })
    }

    async fn list_files_gallery(&self, space_id: &Uuid) -> AppResult<Vec<GalleryFileMeta>> {
        let rows = self
            .db
            .query(&self.storage_stmts.list_media_files_gallery, &[space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get files"))?;

        let size = rows.len();
        rows.into_iter().try_fold(Vec::with_capacity(size), |mut acc, row| {
            let f = GalleryFileMeta::try_from(row)
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse listed files"))?;
            acc.push(f);
            Ok(acc)
        })
    }

    async fn get_thumbnail_preview_stream_keys(&self, space_id: &Uuid, file_id: Uuid) -> AppResult<Option<StreamKeys>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_media_stream_keys, &[&file_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file thumbnail path"))?;

        match rows.into_iter().next() {
            Some(row) => StreamKeys::try_from(row)
                .map(Some)
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse thumbnail path for file")),
            None => Ok(None),
        }
    }

    async fn get_download_stream_key(&self, space_id: &Uuid, file_id: Uuid) -> AppResult<Option<String>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_media_object_key, &[&file_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file preview path"))?;

        match rows.into_iter().next() {
            Some(row) => StreamKey::try_from(row)
                .map(|p| Some(p.key))
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse preview path for file")),
            None => Ok(None),
        }
    }

    async fn create_album(&self, user_id: &Uuid, space_id: Uuid, album_name: String) -> AppResult<Album> {
        let row = self
            .db
            .query_one(
                &self.storage_stmts.insert_album,
                &[&Uuid::now_v7(), user_id, &space_id, &album_name, &String::new()],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to create album"))?;

        Album::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse created album"))
    }

    async fn get_album(&self, space_id: &Uuid, album_id: &Uuid) -> AppResult<Option<Album>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_album, &[album_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get album"))?;

        match rows.into_iter().next() {
            Some(row) => {
                Album::try_from(row).map(Some).map_err(|err| ErrType::DbError.err(err, "Failed to parse album by id"))
            }
            None => Ok(None),
        }
    }

    async fn list_albums(&self, space_id: Uuid) -> AppResult<Vec<Album>> {
        let rows = self
            .db
            .query(&self.storage_stmts.list_albums, &[&space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get albums"))?;

        let size = rows.len();
        rows.into_iter().try_fold(Vec::with_capacity(size), |mut acc, row| {
            let a = Album::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse listed albums"))?;
            acc.push(a);
            Ok(acc)
        })
    }

    async fn link_album_files(&self, space_id: &Uuid, album_id: &Uuid, file_ids: &[Uuid]) -> AppResult<()> {
        for file_id in file_ids {
            let _ = self
                .db
                .query(&self.storage_stmts.link_album_media_file, &[album_id, file_id, space_id])
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed to link file to album"))?;
        }

        Ok(())
    }

    async fn unlink_album_files(&self, space_id: &Uuid, album_id: &Uuid, file_ids: &[Uuid]) -> AppResult<()> {
        for file_id in file_ids {
            let _ = self
                .db
                .query(&self.storage_stmts.unlink_album_media_file, &[album_id, file_id, space_id])
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed to unlink file from album"))?;
        }

        Ok(())
    }

    async fn delete_album(&self, space_id: &Uuid, album_id: &Uuid) -> AppResult<()> {
        let _ = self
            .db
            .query(&self.storage_stmts.delete_album, &[album_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to delete album"))?;

        Ok(())
    }

    async fn delete_file(&self, file_id: &Uuid, space_id: &Uuid) -> AppResult<()> {
        let _ = self
            .db
            .query(&self.storage_stmts.delete_media_file, &[file_id, space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to delete file"))?;

        Ok(())
    }
}
