use lib_core::{AppResult, ErrType};
use uuid::Uuid;

use crate::{
    datastore::{
        space::SpaceDs,
        user_space::{SpaceRole, UserSpaceDs},
    },
    dto::space::res::{UserSpacesResopnse, _SpaceResponse, _SpaceUserResponseVec, _UserSpaceResponseVec},
    extension::{SpaceCtx, UserId},
};

use super::ServiceWrapper;

pub trait UserSpaceService: Send + Sync {
    fn get_spaces_for_user(&self, user_id: UserId) -> impl Future<Output = AppResult<UserSpacesResopnse>> + Send;

    fn get_users_for_space(&self, space_ctx: SpaceCtx)
        -> impl Future<Output = AppResult<_SpaceUserResponseVec>> + Send;

    fn add_user_to_space(&self, space_ctx: SpaceCtx, req_user_id: Uuid) -> impl Future<Output = AppResult<()>> + Send;

    fn update_user_space_role(
        &self,
        user_id: UserId,
        space_ctx: SpaceCtx,
        req_user_id: Uuid,
        req_role: SpaceRole,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn remove_user_from_space(
        &self,
        space_ctx: SpaceCtx,
        req_user_id: Uuid,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn leave_space(&self, space_ctx: SpaceCtx) -> impl Future<Output = AppResult<()>> + Send;
}

impl<D: UserSpaceDs + SpaceDs> UserSpaceService for ServiceWrapper<'_, D> {
    async fn get_spaces_for_user(&self, UserId(user_id): UserId) -> AppResult<UserSpacesResopnse> {
        let default_space =
            self.ds.get_default_space(&user_id).await?.ok_or(ErrType::BadRequest.msg("No default space for user"))?;
        let spaces = self.ds.get_all_spaces_for_user(user_id).await?;

        Ok(UserSpacesResopnse {
            default: _SpaceResponse(default_space),
            spaces: _UserSpaceResponseVec(spaces),
        })
    }

    async fn get_users_for_space(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<_SpaceUserResponseVec> {
        self.ds.get_all_users_for_space(&space_id).await.map(_SpaceUserResponseVec)
    }

    async fn add_user_to_space(
        &self,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot add user: Unauthorized read|upload role"))
            }
            _ => (),
        };

        self.ds.add_user_to_space(&req_user_id, &space_id, SpaceRole::Read).await.map(|_| ())
    }

    async fn update_user_space_role(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: Uuid,
        req_role: SpaceRole,
    ) -> AppResult<()> {
        if user_id == req_user_id {
            return Err(ErrType::BadRequest.msg("Cannot self modify role"));
        }

        match role {
            SpaceRole::Owner => (),
            _ => return Err(ErrType::Unauthorized.msg("Cannot modify user role: Unauthorized role")),
        };

        let space_member = self.ds.get_user_space(&req_user_id, &space_id).await?;
        if let Some(member) = space_member {
            self.ds.update_space_user_role(member.id, req_role).await?;
        }

        Ok(())
    }

    async fn remove_user_from_space(
        &self,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        req_user_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Owner => (),
            _ => return Err(ErrType::Unauthorized.msg("Cannot remove user: Unauthorized role")),
        };

        let space_member = self.ds.get_user_space(&req_user_id, &space_id).await?;
        if let Some(member) = space_member {
            self.ds.remove_user_from_space(member.id).await?;
        }

        Ok(())
    }

    async fn leave_space(
        &self,
        SpaceCtx {
            membership_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<()> {
        self.ds.remove_user_from_space(membership_id).await
    }
}
