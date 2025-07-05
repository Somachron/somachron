use lib_core::{AppError, AppResult, ErrType, UserId};

use crate::dto::user::res::_UserResponse;

use super::Service;

impl Service {
    pub async fn get_user(&self, UserId(id): UserId) -> AppResult<_UserResponse> {
        self.ds.get_user_by_id(&id).await?.map(_UserResponse).ok_or(AppError::new(ErrType::NotFound, "User not found"))
    }
}
