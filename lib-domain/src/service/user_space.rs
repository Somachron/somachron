use lib_core::{AppResult, ErrType};

use crate::{
    datastore::user_space::SpaceRole,
    dto::space::res::{_SpaceUserResponseVec, _UserSpaceResponseVec},
    extension::{IdStr, SpaceCtx, UserId},
};

use super::Service;

impl Service {
    pub async fn get_spaces_for_user(&self, UserId(user_id): UserId) -> AppResult<_UserSpaceResponseVec> {
        self.ds.get_all_spaces_for_user(user_id).await.map(_UserSpaceResponseVec)
    }

    pub async fn get_users_for_space(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<_SpaceUserResponseVec> {
        self.ds.get_all_users_for_space(&space_id.id()).await.map(_SpaceUserResponseVec)
    }

    pub async fn add_user_to_space(
        &self,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot add user: Unauthorized read|upload role"))
            }
            _ => (),
        };

        self.ds.add_user_to_space(&req_user_id, space_id, SpaceRole::Read).await.map(|_| ())
    }

    pub async fn update_user_space_role(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: String,
        req_role: SpaceRole,
    ) -> AppResult<()> {
        if user_id.id() == req_user_id {
            return Err(ErrType::BadRequest.msg("Cannot self modify role"));
        }

        match role {
            SpaceRole::Owner => (),
            _ => return Err(ErrType::Unauthorized.msg("Cannot modify user role: Unauthorized role")),
        };

        let space_member = self.ds.get_user_space(&req_user_id, &space_id.id()).await?;
        if let Some(member) = space_member {
            self.ds.update_space_user_role(member.id, req_role).await?;
        }

        Ok(())
    }

    pub async fn remove_user_from_space(
        &self,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: String,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Owner => (),
            _ => return Err(ErrType::Unauthorized.msg("Cannot remove user: Unauthorized role")),
        };

        let space_member = self.ds.get_user_space(&req_user_id, &space_id.id()).await?;
        if let Some(member) = space_member {
            self.ds.remove_user_from_space(member.id).await?;
        }

        Ok(())
    }

    pub async fn leave_space(
        &self,
        SpaceCtx {
            membership_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<()> {
        self.ds.remove_user_from_space(membership_id).await
    }
}
