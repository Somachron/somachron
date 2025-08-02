use lib_core::{storage::Storage, AppResult, ErrType};

use crate::{
    datastore::user_space::SpaceRole,
    dto::cloud::{
        req::UploadCompleteRequest,
        res::{FileEntryResponse, SignedUrlResponse, _FileMetaResponse},
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
        storage: &Storage,
        path: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read => return Err(ErrType::Unauthorized.new("Cannot create folder: Unauthorized read role")),
            _ => (),
        };

        let space_id = space_id.id();
        storage.create_folder(&space_id, &path).await
    }

    pub async fn generate_upload_signed_url(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        path: String,
    ) -> AppResult<SignedUrlResponse> {
        match role {
            SpaceRole::Read => return Err(ErrType::Unauthorized.new("Cannot upload: Unauthorized read role")),
            _ => (),
        };

        let space_id = space_id.id();
        let url = storage.generate_upload_signed_url(&space_id, &path).await?;
        Ok(SignedUrlResponse {
            url,
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
            file_path,
            file_size,
        }: UploadCompleteRequest,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read => return Err(ErrType::Unauthorized.new("Cannot complete upload: Unauthorized read role")),
            _ => (),
        };

        let space_id_str = space_id.id();
        let file_data = storage.process_upload_completion(&space_id_str, &file_path, file_size).await?;
        for data in file_data.into_iter() {
            let _ = self.ds.upsert_file(user_id.clone(), space_id.clone(), data).await?;
        }

        Ok(())
    }

    pub async fn list_dir(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        path: String,
    ) -> AppResult<Vec<FileEntryResponse>> {
        let space_id_str = space_id.id();
        let folder_hash = storage.get_folder_hash(&space_id_str, &path)?;
        let mut folders = storage.list_dir(&space_id_str, &path).await?;
        let mut files = self.ds.get_files(space_id, folder_hash).await?;

        let mut response = Vec::with_capacity(folders.len() + files.len());

        folders.sort();
        for folder in folders.into_iter() {
            response.push(FileEntryResponse::dir(folder));
        }

        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        for file in files.into_iter() {
            response.push(FileEntryResponse::file(_FileMetaResponse(file)));
        }

        Ok(response)
    }

    pub async fn get_file(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: String,
    ) -> AppResult<(Vec<u8>, String)> {
        let thumbnail_path = self.ds.get_file_thumbnail(&file_id).await?;
        match thumbnail_path {
            Some(path) => storage.get_file(&space_id.id(), &path).await,
            None => return Err(ErrType::NotFound.new("Requested file not found")),
        }
    }

    pub async fn generate_download_signed_url(&self, storage: &Storage, file_id: String) -> AppResult<String> {
        let r2_path = self.ds.get_file_r2(&file_id).await?;
        match r2_path {
            Some(path) => storage.generate_download_signed_url(&path).await,
            None => return Err(ErrType::NotFound.new("Requested file not found")),
        }
    }

    pub async fn delete_path(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        path: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.new("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        let space_id_str = space_id.id();

        let is_dir = storage.delete_path_type(&space_id_str, &path)?;
        if is_dir {
            let folder_hashes = storage.delete_folder(&space_id_str, &path).await?;
            for hash in folder_hashes.into_iter() {
                self.ds.delete_folder(space_id.clone(), hash).await?;
            }
        } else {
            let (file_hash, folder_hash) = storage.delete_file(&space_id_str, &path).await?;
            self.ds.delete_file(space_id, file_hash, folder_hash).await?;
        }

        Ok(())
    }
}
