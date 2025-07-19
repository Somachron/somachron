use lib_core::{extensions::UserId, storage::Storage, AppResult};

use crate::{
    datastore::SpaceRole,
    dto::space::{
        req::SpaceCreateRequest,
        res::{_SpaceResponse, _UserSpaceResponseVec},
    },
};

use super::Service;

impl Service {
    pub async fn create_user_space(
        &self,
        UserId(user_id): UserId,
        storage: &Storage,
        dto: SpaceCreateRequest,
    ) -> AppResult<_SpaceResponse> {
        let space = self.ds.insert_space(&dto.name, &dto.description).await?;

        let _ = self.ds.add_user_to_space(&user_id, &space.id, SpaceRole::Owner).await?;

        storage.create_space_folder(&space.id).await?;

        Ok(_SpaceResponse(space))
    }

    pub async fn get_user_spaces(&self, UserId(user_id): UserId) -> AppResult<_UserSpaceResponseVec> {
        self.ds.get_all_spaces_for_user(&user_id).await.map(_UserSpaceResponseVec)
    }
}
