use chrono::{DateTime, Utc};
use lib_core::{AppResult, ErrType};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;
use utoipa::ToSchema;

use crate::datastore::DbSchema;

use super::Datastore;

#[derive(Debug, ToSchema, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SpaceRole {
    Owner,
    Read,
    Upload,
    Modify,
}

/// [`super::space::Space`] info for [`super::user::User`]
#[derive(Deserialize)]
pub struct UserSpace {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub role: SpaceRole,
    pub space: super::space::Space,
}

/// Member of [`super::space::Space`] with [`super::user::User`] info
#[derive(Deserialize)]
pub struct SpaceUser {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub role: SpaceRole,
    pub user: super::user::User,
}

#[derive(Deserialize)]
pub struct SpaceMember {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// [`super::user::User::id`]
    pub r#in: RecordId,
    /// [`super::space::Space::id`]
    pub out: RecordId,
    pub role: SpaceRole,
}
impl DbSchema for SpaceMember {
    fn table_name() -> &'static str {
        "space_member"
    }
}

impl Datastore {
    pub async fn add_user_to_space(
        &self,
        user_id: &str,
        space_id: RecordId,
        role: SpaceRole,
    ) -> AppResult<SpaceMember> {
        let user_id = super::user::User::get_id(user_id);

        let mut res = self
            .db
            .query("RELATE $u->space_member->$s SET role = $r")
            .bind(("u", user_id))
            .bind(("s", space_id))
            .bind(("r", role))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query add user to space"))?;

        let space_members: Vec<SpaceMember> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize space member"))?;

        space_members.into_iter().nth(0).ok_or(ErrType::DbError.new("Failed to add user to space"))
    }

    pub async fn get_user_space(&self, user_id: &str, space_id: &str) -> AppResult<Option<SpaceMember>> {
        let user_id = super::user::User::get_id(user_id);
        let space_id = super::space::Space::get_id(&space_id);

        let mut res = self
            .db
            .query("SELECT * FROM space_member WHERE in = $u AND out = $s")
            .bind(("u", user_id))
            .bind(("s", space_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query get spaces for users"))?;

        let space_members: Vec<SpaceMember> =
            res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize space member"))?;

        Ok(space_members.into_iter().nth(0))
    }

    pub async fn get_all_spaces_for_user(&self, user_id: RecordId) -> AppResult<Vec<UserSpace>> {
        let mut res = self
            .db
            .query("SELECT id, created_at, updated_at, role, out.* AS space FROM space_member WHERE in = $u")
            .bind(("u", user_id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query get spaces for user"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize spaces for user"))
    }

    pub async fn get_all_users_for_space(&self, space_id: &str) -> AppResult<Vec<SpaceUser>> {
        let id = super::space::Space::get_id(space_id);
        let mut res = self
            .db
            .query("SELECT id, created_at, updated_at, role, in.* AS user FROM space_member WHERE out = $s")
            .bind(("s", id))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query get users for space"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize users for space"))
    }

    pub async fn update_space_user_role(&self, space_member_id: RecordId, role: SpaceRole) -> AppResult<()> {
        let mut res = self
            .db
            .query("UPDATE $id SET role = $r")
            .bind(("id", space_member_id))
            .bind(("r", role))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query update user space role"))?;

        res.take::<Vec<SpaceMember>>(0)
            .map(|_| ())
            .map_err(|err| ErrType::DbError.err(err, "Failed to deseriliaze update user space role"))
    }

    pub async fn remove_user_from_space(&self, space_member_id: RecordId) -> AppResult<()> {
        self.db
            .delete::<Option<SpaceMember>>(space_member_id)
            .await
            .map(|_| ())
            .map_err(|err| ErrType::DbError.err(err, "Failed to remove user from space"))
    }
}
