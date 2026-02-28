use lib_core::{
    clerk::webhook::{UserUpdate, UserUpdateEvent},
    AppResult, ErrType,
};

use crate::{
    datastore::user::UserDs,
    dto::user::res::{_PlatformUserResponseVec, _UserResponse},
    extension::UserId,
};

use super::ServiceWrapper;

pub trait UserService: Send + Sync {
    fn get_user(&self, id: UserId) -> impl Future<Output = AppResult<_UserResponse>> + Send;
    fn get_platform_users(&self) -> impl Future<Output = AppResult<_PlatformUserResponseVec>> + Send;
    fn webhook_update_user(&self, data: UserUpdateEvent) -> impl Future<Output = AppResult<()>> + Send;
}

impl<D: UserDs> UserService for ServiceWrapper<'_, D> {
    async fn get_user(&self, UserId(id): UserId) -> AppResult<_UserResponse> {
        self.ds.get_user_by_id(id).await?.map(_UserResponse).ok_or(ErrType::NotFound.msg("User not found"))
    }

    async fn get_platform_users(&self) -> AppResult<_PlatformUserResponseVec> {
        self.ds.get_platform_users().await.map(_PlatformUserResponseVec)
    }

    async fn webhook_update_user(&self, data: UserUpdateEvent) -> AppResult<()> {
        let UserUpdate {
            id,
            first_name,
            last_name,
            picture_url,
        } = data.get_data_update();

        let Some(user) = self.ds.get_user_by_clerk_id(&id).await? else {
            return Ok(());
        };

        let _ = self.ds.update_user(user.id, &first_name, &last_name, &picture_url).await?;
        Ok(())
    }
}
