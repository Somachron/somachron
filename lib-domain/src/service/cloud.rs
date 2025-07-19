use lib_core::{
    extensions::{SpaceCtx, UserId, UserRole},
    storage::Storage,
    AppResult, ErrType,
};

use crate::dto::cloud::{req::UploadCompleteRequest, res::UploadSignedUrlResponse};

use super::Service;

impl Service {
    pub async fn create_folder(
        &self,
        SpaceCtx {
            id: space_id,
            role,
        }: SpaceCtx,
        storage: &Storage,
        path: String,
    ) -> AppResult<()> {
        match role {
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot create folder: Unauthorized read role")),
            _ => (),
        };

        storage.create_folder(&space_id, &path).await
    }

    pub async fn generate_upload_signed_url(
        &self,
        SpaceCtx {
            id: space_id,
            role,
        }: SpaceCtx,
        storage: &Storage,
        path: String,
    ) -> AppResult<UploadSignedUrlResponse> {
        match role {
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot upload: Unauthorized read role")),
            _ => (),
        };

        let url = storage.generate_upload_signed_url(&space_id, &path).await?;
        Ok(UploadSignedUrlResponse {
            url,
        })
    }

    pub async fn process_upload_skeleton_thumbnail(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            id: space_id,
            role,
        }: SpaceCtx,
        storage: &Storage,
        body: UploadCompleteRequest,
    ) -> AppResult<()> {
        match role {
            UserRole::Read => return Err(ErrType::Unauthorized.new("Cannot complete upload: Unauthorized read role")),
            _ => (),
        };

        storage.process_upload_skeleton_thumbnail(&user_id, &space_id, &body.file_path, body.file_size).await
    }
}
