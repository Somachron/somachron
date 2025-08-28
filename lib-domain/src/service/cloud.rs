use lib_core::{storage::Storage, AppResult, ErrType};

use crate::{
    datastore::user_space::SpaceRole,
    dto::cloud::{
        req::UploadCompleteRequest,
        res::{InitiateUploadResponse, StreamedUrlsResponse, _FileMetaResponseVec, _FolderResponseVec},
    },
    extension::{IdStr, SpaceCtx, UserId},
};

use super::Service;

impl Service {
    pub async fn create_folder(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        parent_folder_id: String,
        folder_name: String,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot create folder: Unauthorized read role"));
        }

        self.ds.create_folder(space_id, parent_folder_id, folder_name).await
    }

    pub async fn initiate_upload(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        folder_id: String,
        file_name: String,
    ) -> AppResult<InitiateUploadResponse> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot upload: Unauthorized read role"));
        }

        let Some((folder_path, folder_name)) = self.ds.trace_path_root(space_id.clone(), folder_id).await? else {
            return Err(ErrType::BadRequest.msg("Folder not found"));
        };

        // TODO: what to do when file with name already exists ?
        // let file = self.ds.get_file_from_fields(space_id.clone(), file_name.clone(), folder_hash).await?;
        // let file_name = file.map(|f| format!("copy_{}", f.file_name)).unwrap_or(file_name);
        let file_path = std::path::PathBuf::from(folder_path).join(folder_name).join(file_name.clone());
        dbg!(&file_path);

        let url = storage.generate_upload_signed_url(&space_id.id(), file_path.to_str().unwrap()).await?;
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
        }: UploadCompleteRequest,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot complete upload: Unauthorized read role"));
        }

        // TODO: handle root folder
        let Some((folder_path, folder_name)) = self.ds.trace_path_root(space_id.clone(), folder_id.clone()).await?
        else {
            return Err(ErrType::BadRequest.msg("Folder not found"));
        };

        let file_path = std::path::PathBuf::from(folder_path).join(folder_name).join(file_name);
        dbg!(&file_path);

        let space_id_str = space_id.id();
        let file_data =
            storage.process_upload_completion(&space_id_str, file_path.to_str().unwrap(), file_size).await?;
        for data in file_data.into_iter() {
            let _ = self.ds.upsert_file(user_id.clone(), space_id.clone(), folder_id.clone(), data).await?;
        }

        Ok(())
    }

    pub async fn list_files(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        folder_id: String,
    ) -> AppResult<_FileMetaResponseVec> {
        let mut files = self.ds.get_files(space_id, folder_id).await?;
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(_FileMetaResponseVec(files))
    }

    pub async fn list_folders(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        folder_id: String,
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
        file_id: String,
    ) -> AppResult<StreamedUrlsResponse> {
        let Some(stream_paths) = self.ds.get_file_stream_paths(space_id, &file_id).await? else {
            return Err(ErrType::NotFound.msg("Requested file not found"));
        };

        let original_stream = storage.generate_download_signed_url(&stream_paths.original_path).await?;
        let thumbnail_stream = storage.generate_download_signed_url(&stream_paths.thumbnail_path).await?;

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
        folder_id: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        let space_id_str = space_id.id();

        let folders = self.ds.get_inner_folder_paths(space_id.clone(), folder_id.clone()).await?;
        for (folder_path, folder_id) in folders.into_iter() {
            storage.delete_folder(&space_id_str, &folder_path).await?;
            self.ds.delete_folder(space_id.clone(), folder_id).await?;
        }

        Ok(())
    }

    pub async fn delete_file(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        if let Some(file) = self.ds.get_file(space_id, &file_id).await? {
            storage
                .delete_file(
                    format!("{}/{}", file.path, file.file_name),
                    format!("{}/{}", file.path, file.thumbnail_file_name),
                )
                .await?;
            self.ds.delete_file(file.id).await?;
        }
        Ok(())
    }
}
