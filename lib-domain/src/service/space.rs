use lib_core::{storage::Storage, AppResult};

use crate::{
    dto::space::{req::SpaceCreateRequest, res::_SpaceResponse},
    extension::UserId,
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
            self.ds.add_user_to_space(&user_id, &space.id, crate::datastore::user_space::SpaceRole::Owner).await?;

        self.ds.create_root_folder(&space.id).await?;
        storage.create_space_folder(&member.space_id.to_string()).await?;

        Ok(_SpaceResponse(space))
    }
}
