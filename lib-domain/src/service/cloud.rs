use chrono::DateTime;
use lib_core::{storage::Storage, AppResult, ErrType};
use uuid::Uuid;

use crate::{
    datastore::{storage::StorageDs, user_space::SpaceRole},
    dto::cloud::{
        req::UploadCompleteRequest,
        res::{InitiateUploadResponse, StreamedUrlsResponse, _FileMetaResponseVec, _FolderResponseVec},
    },
    extension::{SpaceCtx, UserId},
};

use super::Service;

impl<D: StorageDs> Service<D> {
    pub async fn create_folder(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        parent_folder_id: Uuid,
        folder_name: String,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot create folder: Unauthorized read role"));
        }

        let parent_folder = self
            .ds
            .get_folder(&space_id, &parent_folder_id)
            .await?
            .ok_or(ErrType::NotFound.msg("Parent folder not found for folder creation"))?;

        self.ds.create_folder(space_id, parent_folder, folder_name).await
    }

    pub async fn initiate_upload(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        folder_id: Uuid,
        file_name: String,
    ) -> AppResult<InitiateUploadResponse> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot upload: Unauthorized read role"));
        }

        let Some(folder) = self.ds.get_folder(&space_id, &folder_id).await? else {
            return Err(ErrType::BadRequest.msg("Folder not found"));
        };

        // TODO: what to do when file with name already exists ?
        // let file = self.ds.get_file_from_fields(space_id.clone(), file_name.clone(), folder_hash).await?;
        // let file_name = file.map(|f| format!("copy_{}", f.file_name)).unwrap_or(file_name);
        let file_path = std::path::PathBuf::from(&folder.path).join(file_name.clone());

        let url = storage.generate_upload_signed_url(&space_id.to_string(), file_path.to_str().unwrap()).await?;
        Ok(InitiateUploadResponse {
            url,
            file_name,
        })
    }

    pub async fn process_upload_completion(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        storage: &Storage,
        UploadCompleteRequest {
            folder_id,
            file_name,
            file_size,
            updated_millis,
        }: UploadCompleteRequest,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot complete upload: Unauthorized read role"));
        }

        let Some(folder) = self.ds.get_folder(&space_id, &folder_id.0).await? else {
            return Err(ErrType::BadRequest.msg("Folder not found"));
        };

        let Some(updated_date) = DateTime::from_timestamp_millis(updated_millis) else {
            return Err(ErrType::BadRequest.msg("Invalid timestamp"));
        };

        let file_path = std::path::PathBuf::from(&folder.path).join(file_name);

        let space_id_str = space_id.to_string();
        let file_data =
            storage.process_upload_completion(&space_id_str, file_path.to_str().unwrap(), file_size).await?;
        for data in file_data.into_iter() {
            let _ = self.ds.upsert_file(&user_id, &space_id, &folder, updated_date, data).await?;
        }

        Ok(())
    }

    pub async fn list_files(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        folder_id: Uuid,
    ) -> AppResult<_FileMetaResponseVec> {
        let mut files = self.ds.list_files(&space_id, &folder_id).await?;
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(_FileMetaResponseVec(files))
    }

    pub async fn list_files_gallery(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<_FileMetaResponseVec> {
        let files = self.ds.list_files_gallery(&space_id).await?;
        let files: Vec<_> = files.into_iter().map(|g| g.0).collect();
        Ok(_FileMetaResponseVec(files))
    }

    pub async fn list_folders(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        folder_id: Uuid,
    ) -> AppResult<_FolderResponseVec> {
        self.ds.list_folder(space_id, folder_id).await.map(_FolderResponseVec)
    }

    pub async fn generate_download_signed_url(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> AppResult<StreamedUrlsResponse> {
        let Some(stream_paths) = self.ds.get_file_stream_paths(&space_id, file_id).await? else {
            return Err(ErrType::NotFound.msg("Requested file not found"));
        };

        let space_id_str = space_id.to_string();
        let original_stream = storage.generate_download_signed_url(&space_id_str, &stream_paths.original_path).await?;
        let thumbnail_stream =
            storage.generate_download_signed_url(&space_id_str, &stream_paths.thumbnail_path).await?;

        Ok(StreamedUrlsResponse {
            original_stream,
            thumbnail_stream,
        })
    }

    pub async fn delete_folder(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        folder_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        let space_id_str = space_id.to_string();
        let folders = self.ds.get_inner_folder_paths(&space_id, &folder_id).await?;
        for inner in folders.iter().rev() {
            storage.delete_folder(&space_id_str, &inner.path).await?;
        }
        self.ds.delete_folder(&space_id, folders).await
    }

    pub async fn delete_file(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        if let Some(file) = self.ds.get_file(space_id, file_id).await? {
            storage
                .delete_file(
                    format!("{}/{}", file.path, file.node_name),
                    format!("{}/{}", file.path, file.metadata.thumbnail_meta.unwrap_or_default().file_name),
                )
                .await?;
            self.ds.delete_file(file.id).await?;
        }
        Ok(())
    }
}
