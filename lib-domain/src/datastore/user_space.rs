use chrono::{DateTime, Utc};
use lib_core::{AppResult, ErrType};

use super::{create_id, Datastore, SpaceRole};

pub struct UserSpace {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub name: String,
    pub description: String,
    pub picture_url: String,
    pub role: SpaceRole,
}
impl From<tokio_postgres::Row> for UserSpace {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            name: value.get(3),
            description: value.get(4),
            picture_url: value.get(5),
            role: value.get(6),
        }
    }
}

pub struct SpaceUser {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub given_name: String,
    pub picture_url: String,
    pub role: SpaceRole,
}
impl From<tokio_postgres::Row> for SpaceUser {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            given_name: value.get(3),
            picture_url: value.get(5),
            role: value.get(6),
        }
    }
}

impl Datastore {
    pub async fn add_user_to_space(&self, user_id: &str, space_id: &str, role: SpaceRole) -> AppResult<String> {
        let id = create_id();
        let row = self
            .client
            .query_one(&self.user_space_stmts.add_user_to_space, &[&id, &user_id, &space_id, &role])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to add user to space"))?;

        Ok(row.get(0))
    }

    pub async fn get_all_spaces_for_user(&self, user_id: &str) -> AppResult<Vec<UserSpace>> {
        let rows = self
            .client
            .query(&self.user_space_stmts.get_spaces_for_user, &[&user_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get spaces for users"))?;

        Ok(rows.into_iter().map(UserSpace::from).collect())
    }

    pub async fn get_user_space(&self, user_id: &str, space_id: &str) -> AppResult<Option<UserSpace>> {
        let rows = self
            .client
            .query(&self.user_space_stmts.get_user_space, &[&user_id, &space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get spaces for users"))?;

        Ok(rows.into_iter().nth(0).map(UserSpace::from))
    }

    pub async fn get_users_from_space(&self, space_id: &str) -> AppResult<Vec<SpaceUser>> {
        let rows = self
            .client
            .query(&self.user_space_stmts.get_users_for_space, &[&space_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get users for space"))?;

        Ok(rows.into_iter().map(SpaceUser::from).collect())
    }

    pub async fn update_space_user_role(&self, user_id: &str, space_id: &str, role: SpaceRole) -> AppResult<String> {
        let row = self
            .client
            .query_one(&self.user_space_stmts.update_user_space_role, &[&role, &space_id, &user_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update user space role"))?;

        Ok(row.get(0))
    }

    pub async fn remove_user_from_space(&self, user_id: &str, space_id: &str) -> AppResult<()> {
        self.client
            .query(&self.user_space_stmts.remove_user_from_space, &[&space_id, &user_id])
            .await
            .map(|_| ())
            .map_err(|err| ErrType::DbError.err(err, "Failed to remove user from space"))
    }
}
