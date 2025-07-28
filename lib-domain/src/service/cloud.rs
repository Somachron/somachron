use lib_core::{storage::Storage, AppResult, ErrType};

use crate::{
    datastore::user_space::UserRole,
    dto::cloud::{req::UploadCompleteRequest, res::SignedUrlResponse},
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
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot create folder: Unauthorized read role")),
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
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot upload: Unauthorized read role")),
            _ => (),
        };

        let space_id = space_id.id();
        let url = storage.generate_upload_signed_url(&space_id, &path).await?;
        Ok(SignedUrlResponse {
            url,
        })
    }

    pub async fn process_upload_skeleton_thumbnail(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        body: UploadCompleteRequest,
    ) -> AppResult<()> {
        match role {
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot complete upload: Unauthorized read role")),
            _ => (),
        };

        let space_id = space_id.id();
        let user_id = user_id.id();
        storage.process_upload_skeleton_thumbnail(&user_id, &space_id, &body.file_path, body.file_size).await
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
            UserRole::Read | UserRole::Upload => {
                return Err(ErrType::Unauthorized.new("Cannot delete: Unauthorized read role"))
            }
            _ => (),
        };

        let space_id = space_id.id();
        storage.delete_path(&space_id, &path).await
    }
}
