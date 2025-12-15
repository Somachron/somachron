use lib_core::{storage::Storage, AppResult, ErrorContext};

use crate::{
    datastore::{space::SpaceDs, storage::StorageDs, user_space::UserSpaceDs},
    dto::space::{req::SpaceCreateRequest, res::_SpaceResponse},
    extension::UserId,
};

use super::Service;

impl<D: UserSpaceDs + SpaceDs + StorageDs> Service<D> {
    pub async fn create_user_space(
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

        self.ds.create_root_folder(&space.id).await.context("s:create_user_space")?;
        storage.create_space_folder(&member.space_id.to_string()).await.context("s:create_user_space")?;

        Ok(_SpaceResponse(space))
    }
}
