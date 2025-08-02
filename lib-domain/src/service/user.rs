use lib_core::{AppResult, ErrType};

use crate::{
    dto::user::res::{_PlatformUserResponseVec, _UserResponse},
    extension::UserId,
};

use super::Service;

impl Service {
    pub async fn get_user(&self, UserId(id): UserId) -> AppResult<_UserResponse> {
        self.ds.get_user_by_id(id).await?.map(_UserResponse).ok_or(ErrType::NotFound.new("User not found"))
    }

    pub async fn get_platform_users(&self) -> AppResult<_PlatformUserResponseVec> {
        self.ds.get_platform_users().await.map(_PlatformUserResponseVec)
    }
}
