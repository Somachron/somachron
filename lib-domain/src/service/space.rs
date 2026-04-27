use lib_core::{storage::Storage, AppResult, ErrType, ErrorContext};
use uuid::Uuid;

use crate::{
    datastore::{space::SpaceDs, storage::StorageDs, user::UserDs, user_space::UserSpaceDs},
    dto::space::{req::SpaceCreateRequest, res::_SpaceResponse},
    extension::UserId,
};

use super::ServiceWrapper;

pub trait SpaceService: Send + Sync {
    fn create_user_space(
        &self,
        user_id: UserId,
        storage: &Storage,
        dto: SpaceCreateRequest,
    ) -> impl Future<Output = AppResult<_SpaceResponse>> + Send;

    fn get_or_setup_default_space(
        &self,
        user_id: Uuid,
        storage: &Storage,
    ) -> impl Future<Output = AppResult<_SpaceResponse>> + Send;
}

impl<D: UserDs + UserSpaceDs + SpaceDs + StorageDs> SpaceService for ServiceWrapper<'_, D> {
    async fn create_user_space(
        &self,
        UserId(user_id): UserId,
        storage: &Storage,
        dto: SpaceCreateRequest,
    ) -> AppResult<_SpaceResponse> {
        let space = self.ds.insert_space(&dto.name, &dto.description).await.context("s:create_user_space")?;

        let member = self
            .ds
            .add_user_to_space(&user_id, &space.id, crate::datastore::user_space::SpaceRole::Owner)
            .await
            .context("s:create_user_space")?;

        storage.create_space_folder(&member.space_id.to_string()).await.context("s:create_user_space")?;

        Ok(_SpaceResponse(space))
    }

    async fn get_or_setup_default_space(&self, user_id: Uuid, storage: &Storage) -> AppResult<_SpaceResponse> {
        let user = self.ds.get_user_by_id(user_id).await?.ok_or(ErrType::BadRequest.msg("User not found"))?;
        if !user.allowed {
            return Err(ErrType::Unauthorized.msg("User not allowed"));
        }

        let space = self.ds.get_default_space(&user_id).await?;
        if let Some(space) = space {
            return Ok(_SpaceResponse(space));
        }

        let space = self.ds.set_default_space(&user_id).await?;

        storage.create_space_folder(&space.id.to_string()).await.context("setting up default space")?;

        Ok(_SpaceResponse(space))
    }
}
