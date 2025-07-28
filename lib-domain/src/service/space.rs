use lib_core::{storage::Storage, AppResult};

use crate::{
    dto::space::{
        req::SpaceCreateRequest,
        res::{_SpaceResponse, _UserSpaceResponseVec},
    },
    extension::{IdStr, UserId},
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

        let member =
            self.ds.add_user_to_space(user_id, space.id.clone(), crate::datastore::user_space::UserRole::Owner).await?;

        let space_id = member.out.id();
        storage.create_space_folder(&space_id).await?;

        Ok(_SpaceResponse(space))
    }

    pub async fn get_user_spaces(&self, UserId(user_id): UserId) -> AppResult<_UserSpaceResponseVec> {
        self.ds.get_all_spaces_for_user(user_id).await.map(_UserSpaceResponseVec)
    }
}
