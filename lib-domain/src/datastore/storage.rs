use chrono::{DateTime, Utc};
use lib_core::{
    media::MediaMetadata,
    storage::{FileData, MediaType},
    AppError, AppResult, ErrType,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::datastore::{statements::StorageStatements, Datastore};

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
            date_time: metadata.date_time.or(metadata.fs_date_time).map(|dt| dt.0),
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

#[derive(Debug)]
pub enum NodeType {
    Folder,
    File,
}
impl NodeType {
    pub fn value(&self) -> i16 {
        match self {
            NodeType::Folder => 0,
            NodeType::File => 1,
        }
    }
}
impl TryFrom<i16> for NodeType {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(NodeType::Folder),
            1 => Ok(NodeType::File),
            x => Err(ErrType::DbError.msg(format!("Invalid node type: {x}"))),
        }
    }
}
impl<'a> tokio_postgres::types::FromSql<'a> for NodeType {
    fn from_sql(
        ty: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let node_type = i16::from_sql(ty, raw)?;
        let node_type = NodeType::try_from(node_type)?;
        Ok(node_type)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        match *ty {
            tokio_postgres::types::Type::INT2 => true,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct NodeMetadata {
    pub thumbnail_file_name: Option<String>,
    pub file_meta: Option<Metadata>,
    pub media_type: Option<MediaType>,
}
impl<'a> tokio_postgres::types::FromSql<'a> for NodeMetadata {
    fn from_sql(
        _ty: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        // this will also handle invalid data type or json
        serde_json::from_slice(&raw[1..]).map_err(Into::into)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        match *ty {
            tokio_postgres::types::Type::JSONB => true,
            _ => false,
        }
    }
}
impl NodeMetadata {
    pub fn jsonb(
        thumbnail_file_name: String,
        file_meta: Metadata,
        media_type: MediaType,
    ) -> AppResult<serde_json::Value> {
        let meta = Self {
            thumbnail_file_name: Some(thumbnail_file_name),
            file_meta: Some(file_meta),
            media_type: Some(media_type),
        };
        serde_json::to_value(&meta).map_err(|err| ErrType::FsError.err(err, "Failed to serialize metadata"))
    }
}

pub struct FsNode {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub user_id: Option<Uuid>,
    pub space_id: Uuid,
    pub node_type: NodeType,
    pub node_size: i64,
    pub parent_node: Option<Uuid>,
    pub node_name: String,
    pub path: String,
    pub metadata: NodeMetadata,
}
impl TryFrom<tokio_postgres::Row> for FsNode {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.try_get(0)?,
            created_at: value.try_get(1)?,
            updated_at: value.try_get(2)?,
            user_id: value.try_get(3)?,
            space_id: value.try_get(4)?,
            node_type: value.try_get(5)?,
            node_size: value.try_get(6)?,
            parent_node: value.try_get(7)?,
            node_name: value.try_get(8)?,
            path: value.try_get(9)?,
            metadata: value.try_get(10)?,
        })
    }
}

pub struct FileMeta {
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub file_name: String,
    pub media_type: MediaType,
    pub user: Option<Uuid>,
}
impl TryFrom<tokio_postgres::Row> for FileMeta {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        let meta: NodeMetadata = value.get(10);
        Ok(Self {
            id: value.try_get(0)?,
            updated_at: value.try_get(2)?,
            file_name: value.try_get(8)?,
            media_type: meta.media_type.unwrap_or(MediaType::Image),
            user: value.try_get(3)?,
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
            user: value.try_get(2)?,
            file_name: value.try_get(3)?,
            media_type: serde_json::from_value(serde_json::Value::String(media_type)).unwrap_or(MediaType::Image),
        }))
    }
}

pub struct StreamPaths {
    pub thumbnail_path: String,
    pub original_path: String,
}
impl TryFrom<tokio_postgres::Row> for StreamPaths {
    type Error = tokio_postgres::error::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        Ok(Self {
            thumbnail_path: value.try_get(1)?,
            original_path: value.try_get(0)?,
        })
    }
}

#[derive(Debug)]
pub struct InnerFolder {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub path: String,
}

pub trait StorageDs {
    fn upsert_file(
        &self,
        user_id: &Uuid,
        space_id: &Uuid,
        folder: &FsNode,
        file_data: FileData,
    ) -> impl Future<Output = AppResult<FsNode>>;

    fn get_file_from_fields(
        &self,
        space_id: &Uuid,
        file_name: &str,
        folder_id: &Uuid,
    ) -> impl Future<Output = AppResult<Option<FsNode>>>;

    fn get_file(&self, space_id: Uuid, file_id: Uuid) -> impl Future<Output = AppResult<Option<FsNode>>>;
    fn list_files(&self, space_id: &Uuid, folder_id: &Uuid) -> impl Future<Output = AppResult<Vec<FileMeta>>>;
    fn list_files_gallery(&self, space_id: &Uuid) -> impl Future<Output = AppResult<Vec<GalleryFileMeta>>>;
    fn get_file_stream_paths(
        &self,
        space_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<Option<StreamPaths>>>;

    fn create_root_folder(&self, space_id: &Uuid) -> impl Future<Output = AppResult<()>>;
    fn create_folder(
        &self,
        space_id: Uuid,
        parent_folder: FsNode,
        folder_name: String,
    ) -> impl Future<Output = AppResult<()>>;
    fn get_folder(&self, space_id: &Uuid, folder_id: &Uuid) -> impl Future<Output = AppResult<Option<FsNode>>>;
    fn list_folder(&self, space_id: Uuid, parent_folder_id: Uuid) -> impl Future<Output = AppResult<Vec<FsNode>>>;
    fn get_inner_folder_paths(
        &self,
        space_id: &Uuid,
        folder_id: &Uuid,
    ) -> impl Future<Output = AppResult<Vec<InnerFolder>>>;

    fn delete_folder(&self, space_id: &Uuid, inner_folders: Vec<InnerFolder>) -> impl Future<Output = AppResult<()>>;
    fn delete_file(&self, file_id: Uuid) -> impl Future<Output = AppResult<()>>;
}

impl StorageDs for Datastore {
    async fn upsert_file(
        &self,
        user_id: &Uuid,
        space_id: &Uuid,
        folder: &FsNode,
        file_data: FileData,
    ) -> AppResult<FsNode> {
        let file = match self.get_file_from_fields(&space_id, &file_data.file_name, &folder.id).await? {
            Some(file) => update_file(&self.db, &self.storage_stmts, file.id, folder, space_id, file_data).await,
            None => create_file(&self.db, &self.storage_stmts, user_id, space_id, folder, file_data).await,
        }?;

        Ok(file)
    }

    async fn get_file_from_fields(
        &self,
        space_id: &Uuid,
        file_name: &str,
        folder_id: &Uuid,
    ) -> AppResult<Option<FsNode>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_node_by_name, &[&space_id, &folder_id, &file_name])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file by name"))?;

        match rows.into_iter().next() {
            Some(row) => FsNode::try_from(row)
                .map(Some)
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse file from name")),
            None => Ok(None),
        }
    }

    async fn get_file(&self, space_id: Uuid, file_id: Uuid) -> AppResult<Option<FsNode>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_fs_node, &[&file_id, &NodeType::File.value(), &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file by id"))?;

        match rows.into_iter().next() {
            Some(row) => {
                FsNode::try_from(row).map(Some).map_err(|err| ErrType::DbError.err(err, "Failed to parse file by id"))
            }
            None => Ok(None),
        }
    }

    async fn list_files(&self, space_id: &Uuid, folder_id: &Uuid) -> AppResult<Vec<FileMeta>> {
        let rows = self
            .db
            .query(&self.storage_stmts.list_nodes, &[&NodeType::File.value(), &space_id, &folder_id])
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
            .query(&self.storage_stmts.list_gallery_nodes, &[&NodeType::File.value(), &space_id])
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

    async fn get_file_stream_paths(&self, space_id: Uuid, file_id: Uuid) -> AppResult<Option<StreamPaths>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_file_stream_paths, &[&file_id, &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get file stream paths"))?;

        match rows.into_iter().next() {
            Some(row) => StreamPaths::try_from(row)
                .map(Some)
                .map_err(|err| ErrType::DbError.err(err, "Failed to parse stream paths for file")),
            None => Ok(None),
        }
    }

    async fn create_root_folder(&self, space_id: &Uuid) -> AppResult<()> {
        let id = Uuid::now_v7();
        let _ = self
            .db
            .query_one(
                &self.storage_stmts.insert_fs_node,
                &[
                    &id,
                    &Option::<Uuid>::None,
                    &space_id,
                    &NodeType::Folder.value(),
                    &0i64,
                    &Option::<Uuid>::None,
                    &format!("root_{id}"),
                    &"/",
                    &serde_json::json!({}),
                ],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to create space root folder"))?;

        Ok(())
    }

    async fn create_folder(&self, space_id: Uuid, parent_folder: FsNode, folder_name: String) -> AppResult<()> {
        let parent_folder_id = parent_folder.id;
        let mut new_path = if parent_folder.parent_node.is_none() {
            // avoid space root folder name
            String::new()
        } else {
            parent_folder.path
        };
        new_path.push('/');
        new_path.push_str(&folder_name);

        let row = self
            .db
            .query_one(
                &self.storage_stmts.insert_fs_node,
                &[
                    &Uuid::now_v7(),
                    &Option::<Uuid>::None,
                    &space_id,
                    &NodeType::Folder.value(),
                    &0i64,
                    &parent_folder_id,
                    &folder_name,
                    &new_path,
                    &serde_json::json!({}),
                ],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to create folder"))?;

        let folder =
            FsNode::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse created folder"))?;

        fs_link(&self.db, &self.storage_stmts, &parent_folder.id, folder.id).await?;

        Ok(())
    }

    async fn get_folder(&self, space_id: &Uuid, folder_id: &Uuid) -> AppResult<Option<FsNode>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_fs_node, &[&folder_id, &NodeType::Folder.value(), &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get folder"))?;

        match rows.into_iter().next() {
            Some(row) => {
                FsNode::try_from(row).map(Some).map_err(|err| ErrType::DbError.err(err, "Failed to parse folder by id"))
            }
            None => Ok(None),
        }
    }

    async fn list_folder(&self, space_id: Uuid, parent_folder_id: Uuid) -> AppResult<Vec<FsNode>> {
        let rows = self
            .db
            .query(&self.storage_stmts.list_nodes, &[&NodeType::Folder.value(), &space_id, &parent_folder_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get folders"))?;

        let size = rows.len();
        let mut folders = rows.into_iter().try_fold(Vec::with_capacity(size), |mut acc, row| {
            let f = FsNode::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse listed files"))?;
            acc.push(f);
            Ok(acc)
        })?;

        folders.sort_by(|a, b| a.node_name.cmp(&b.node_name));

        Ok(folders)
    }

    async fn get_inner_folder_paths(&self, space_id: &Uuid, folder_id: &Uuid) -> AppResult<Vec<InnerFolder>> {
        let rows = self
            .db
            .query(&self.storage_stmts.get_inner_folders, &[&folder_id, &space_id, &NodeType::Folder.value()])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get inner folders"))?;

        let size = rows.len();
        rows.into_iter().try_fold(Vec::with_capacity(size), |mut acc, row| {
            let fs_node =
                FsNode::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse inner folders"))?;
            acc.push(InnerFolder {
                id: fs_node.id,
                parent: fs_node.parent_node,
                path: fs_node.path,
            });
            Ok(acc)
        })
    }

    async fn delete_folder(&self, space_id: &Uuid, inner_folders: Vec<InnerFolder>) -> AppResult<()> {
        // for each inner-most folder
        for inner in inner_folders.iter().rev() {
            // get files
            let files = self.list_files(space_id, &inner.id).await?;

            // drop all links for this folder
            self.db
                .query(&self.storage_stmts.drop_parent_fs_link, &[&inner.id])
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed remove folder links"))?;

            self.db
                .query(&self.storage_stmts.drop_child_fs_link, &[&inner.id])
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed remove folder links"))?;

            // delete files
            for file in files.iter() {
                self.db
                    .query(&self.storage_stmts.delete_node, &[&file.id, &inner.id, &space_id])
                    .await
                    .map_err(|err| ErrType::DbError.err(err, "Failed to delete file node"))?;
            }

            // delete folder
            self.db
                .query(&self.storage_stmts.delete_node, &[&inner.id, &inner.parent, &space_id])
                .await
                .map_err(|err| ErrType::DbError.err(err, "Failed to delete node"))?;
        }

        Ok(())
    }

    async fn delete_file(&self, file_id: Uuid) -> AppResult<()> {
        let _ = self
            .db
            .query(&self.storage_stmts.unlink_fs_node, &[&file_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to unlink file"));

        let _ = self
            .db
            .query(&self.storage_stmts.delete_node, &[&file_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to delete file"));

        Ok(())
    }
}

async fn update_file(
    db: &tokio_postgres::Client,
    storage_stmts: &StorageStatements,
    file_id: Uuid,
    folder: &FsNode,
    space_id: &Uuid,
    FileData {
        file_name,
        thumbnail_file_name,
        metadata,
        size: file_size,
        media_type,
    }: FileData,
) -> AppResult<FsNode> {
    let file_meta = Metadata::from(metadata);
    let metadata = NodeMetadata::jsonb(thumbnail_file_name, file_meta, media_type)?;

    let row = db
        .query_one(
            &storage_stmts.update_node,
            &[&file_id, &folder.id, &space_id, &file_name, &file_size, &NodeType::File.value(), &metadata],
        )
        .await
        .map_err(|err| ErrType::DbError.err(err, "Failed to update file"))?;

    FsNode::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse updated file"))
}

async fn create_file(
    db: &tokio_postgres::Client,
    storage_stmts: &StorageStatements,
    user_id: &Uuid,
    space_id: &Uuid,
    folder: &FsNode,
    FileData {
        file_name,
        thumbnail_file_name,
        metadata,
        size: file_size,
        media_type,
    }: FileData,
) -> AppResult<FsNode> {
    let metadata = Metadata::from(metadata);
    let file_meta = NodeMetadata::jsonb(thumbnail_file_name, metadata, media_type)?;

    let row = db
        .query_one(
            &storage_stmts.insert_fs_node,
            &[
                &Uuid::now_v7(),
                &user_id,
                &space_id,
                &NodeType::File.value(),
                &(file_size as i64),
                &folder.id,
                &file_name,
                &folder.path,
                &file_meta,
            ],
        )
        .await
        .map_err(|err| ErrType::DbError.err(err, "Failed to create file"))?;

    let file = FsNode::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse created file"))?;

    fs_link(db, storage_stmts, &folder.id, file.id).await?;

    Ok(file)
}

async fn fs_link(
    db: &tokio_postgres::Client,
    storage_stmts: &StorageStatements,
    parent_folder_id: &Uuid,
    fs_id: Uuid,
) -> AppResult<()> {
    let _ = db
        .query_one(&storage_stmts.link_fs_node, &[&parent_folder_id, &fs_id])
        .await
        .map_err(|err| ErrType::DbError.err(err, "Failed to link fs node"))?;

    Ok(())
}
