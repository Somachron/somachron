use lib_core::{storage::Storage, AppResult, ErrType};

use crate::{
    datastore::{space::Folder, user_space::SpaceRole},
    dto::cloud::{
        req::UploadCompleteRequest,
        res::{SignedUrlResponse, _FileMetaResponseVec},
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
        parent_folder_hash: String,
        folder_name: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read => return Err(ErrType::Unauthorized.new("Cannot create folder: Unauthorized read role")),
            _ => (),
        };

        let path_prefix = storage.get_spaces_path(&space_id.id());
        self.ds.create_folder(space_id, &path_prefix, parent_folder_hash, folder_name).await
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

    pub async fn list_files(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        folder_hash: String,
    ) -> AppResult<_FileMetaResponseVec> {
        let mut files = self.ds.get_files(space_id, folder_hash).await?;
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(_FileMetaResponseVec(files))
    }

    pub async fn list_folders(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<Folder> {
        self.ds.get_dir_tree(space_id).await
    }

    pub async fn generate_download_signed_url(
        &self,
        storage: &Storage,
        file_id: String,
        is_thumbnail: bool,
    ) -> AppResult<String> {
        let r2_path = self.ds.get_file_path(&file_id, is_thumbnail).await?;
        match r2_path {
            Some(path) => storage.generate_download_signed_url(&path).await,
            None => return Err(ErrType::NotFound.new("Requested file not found")),
        }
    }

    pub async fn delete_folder(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        folder_hash: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.new("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        let space_id_str = space_id.id();

        let folders = self.ds.get_inner_dirs(space_id.clone(), folder_hash).await?;
        for (folder_path, hash) in folders.iter() {
            storage.delete_folder(&space_id_str, &folder_path).await?;
            self.ds.delete_files(space_id.clone(), hash.clone()).await?;
        }
        let (folder_path, _) = folders.into_iter().last().expect("Whoa.. empty list ?");
        self.ds.delete_folder(space_id.clone(), &folder_path).await?;

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
                return Err(ErrType::Unauthorized.new("Cannot delete: Unauthorized read|upload role"))
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
