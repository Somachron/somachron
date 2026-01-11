use chrono::{DateTime, Utc};
use lib_core::{AppError, AppResult, ErrType};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::datastore::storage::NodeType;

use super::Datastore;

#[derive(Debug, ToSchema, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SpaceRole {
    Owner,
    Read,
    Upload,
    Modify,
}
impl SpaceRole {
    pub fn value(&self) -> i16 {
        match self {
            SpaceRole::Read => 0,
            SpaceRole::Owner => 1,
            SpaceRole::Modify => 2,
            SpaceRole::Upload => 3,
        }
    }
}
impl TryFrom<i16> for SpaceRole {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SpaceRole::Read),
            1 => Ok(SpaceRole::Owner),
            2 => Ok(SpaceRole::Modify),
            3 => Ok(SpaceRole::Upload),
            x => Err(ErrType::DbError.msg(format!("Invalid space role literal: {x}"))),
        }
    }
}
impl<'a> tokio_postgres::types::FromSql<'a> for SpaceRole {
    fn from_sql(
        ty: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let role_literal = i16::from_sql(ty, raw)?;
        let role = SpaceRole::try_from(role_literal)?;
        Ok(role)
    }

    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        matches!(*ty, tokio_postgres::types::Type::INT2)
    }
}

/// [`super::space::Space`] info for [`super::user::User`]
pub struct UserSpace {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub role: SpaceRole,
    pub space: super::space::Space,
    pub root_folder: Option<Uuid>,
}
impl From<tokio_postgres::Row> for UserSpace {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            // user_id: 3
            // space_id: 4
            role: value.get(5),
            space: super::space::Space {
                id: value.get(6),
                created_at: value.get(7),
                updated_at: value.get(8),
                name: value.get(9),
                description: value.get(10),
                picture_url: value.get(11),
            },
            root_folder: value.get(12),
        }
    }
}

/// Member of [`super::space::Space`] with [`super::user::User`] info
pub struct SpaceUser {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub role: SpaceRole,
    pub user: super::user::User,
}
impl From<tokio_postgres::Row> for SpaceUser {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            // user_id: 3
            // space_id: 4
            role: value.get(5),
            user: super::user::User {
                id: value.get(6),
                created_at: value.get(7),
                updated_at: value.get(8),
                allowed: value.get(9),
                // clerk_id: 10
                email: value.get(11),
                first_name: value.get(12),
                last_name: value.get(13),
                picture_url: value.get(14),
            },
        }
    }
}

pub struct SpaceMember {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub user_id: Uuid,
    pub space_id: Uuid,
    pub role: SpaceRole,
}
impl From<tokio_postgres::Row> for SpaceMember {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            user_id: value.get(3),
            space_id: value.get(4),
            role: value.get(5),
        }
    }
}

pub trait UserSpaceDs {
    fn add_user_to_space(
        &self,
        user_id: &Uuid,
        space_id: &Uuid,
        role: SpaceRole,
    ) -> impl Future<Output = AppResult<SpaceMember>>;
    fn get_user_space(&self, user_id: &Uuid, space_id: &Uuid) -> impl Future<Output = AppResult<Option<SpaceMember>>>;
    fn get_all_spaces_for_user(&self, user_id: Uuid) -> impl Future<Output = AppResult<Vec<UserSpace>>>;
    fn get_all_users_for_space(&self, space_id: &Uuid) -> impl Future<Output = AppResult<Vec<SpaceUser>>>;
    fn update_space_user_role(&self, space_member_id: Uuid, role: SpaceRole) -> impl Future<Output = AppResult<()>>;
    fn remove_user_from_space(&self, space_member_id: Uuid) -> impl Future<Output = AppResult<()>>;
}

impl UserSpaceDs for Datastore {
    async fn add_user_to_space(&self, user_id: &Uuid, space_id: &Uuid, role: SpaceRole) -> AppResult<SpaceMember> {
        let row = self
            .db
            .query_one(&self.user_space_stmts.insert, &[&Uuid::now_v7(), &user_id, &space_id, &role.value()])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to add user to space"))?;

        Ok(SpaceMember::from(row))
    }

    async fn get_user_space(&self, user_id: &Uuid, space_id: &Uuid) -> AppResult<Option<SpaceMember>> {
        let rows = self
            .db
            .query(&self.user_space_stmts.get_user_space, &[&user_id, &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get user space member"))?;

        Ok(rows.into_iter().next().map(SpaceMember::from))
    }

    async fn get_all_spaces_for_user(&self, user_id: Uuid) -> AppResult<Vec<UserSpace>> {
        let rows = self
            .db
            .query(&self.user_space_stmts.get_all_spaces_for_user, &[&user_id, &NodeType::Folder.value()])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get spaces for user"))?;

        Ok(rows.into_iter().map(UserSpace::from).collect())
    }

    async fn get_all_users_for_space(&self, space_id: &Uuid) -> AppResult<Vec<SpaceUser>> {
        let rows = self
            .db
            .query(&self.user_space_stmts.get_all_users_for_space, &[&space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get users for space"))?;

        Ok(rows.into_iter().map(SpaceUser::from).collect())
    }

    async fn update_space_user_role(&self, space_member_id: Uuid, role: SpaceRole) -> AppResult<()> {
        let _ = self
            .db
            .query_one(&self.user_space_stmts.update, &[&space_member_id, &role.value()])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update user space role"))?;
        Ok(())
    }

    async fn remove_user_from_space(&self, space_member_id: Uuid) -> AppResult<()> {
        let _ = self
            .db
            .query_one(&self.user_space_stmts.delete, &[&space_member_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to delete user from space"))?;
        Ok(())
    }
}
