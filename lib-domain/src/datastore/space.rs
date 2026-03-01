use chrono::{DateTime, Utc};
use lib_core::{AppResult, ErrType, ErrorContext};
use uuid::Uuid;

use super::Datastore;

pub struct Space {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub name: String,
    pub description: String,
    pub picture_url: String,
}
impl TryFrom<tokio_postgres::Row> for Space {
    type Error = tokio_postgres::Error;

    fn try_from(value: tokio_postgres::Row) -> Result<Self, Self::Error> {
        let row = Self {
            id: value.try_get(0)?,
            created_at: value.try_get(1)?,
            updated_at: value.try_get(2)?,
            name: value.try_get(3)?,
            description: value.try_get(4)?,
            picture_url: value.try_get(5)?,
        };
        Ok(row)
    }
}

pub trait SpaceDs: Send + Sync {
    fn get_space_by_id(&self, id: &Uuid) -> impl Future<Output = AppResult<Option<Space>>> + Send;
    fn insert_space(&self, name: &str, description: &str) -> impl Future<Output = AppResult<Space>> + Send;
    fn update_space(
        &self,
        id: Uuid,
        name: &'static str,
        description: &'static str,
    ) -> impl Future<Output = AppResult<Space>> + Send;
    fn get_default_space(&self, user_id: &Uuid) -> impl Future<Output = AppResult<Option<Space>>> + Send;
    fn set_default_space(&self, user_id: &Uuid) -> impl Future<Output = AppResult<Space>> + Send;
}

impl SpaceDs for Datastore {
    async fn get_space_by_id(&self, id: &Uuid) -> AppResult<Option<Space>> {
        let rows = self
            .db
            .query(&self.space_stmts.get_by_id, &[&id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get space by id"))?;

        match rows.into_iter().next() {
            Some(row) => {
                Space::try_from(row).map(Some).map_err(|err| ErrType::DbError.err(err, "Failed to parse space row"))
            }
            None => Ok(None),
        }
    }

    async fn insert_space(&self, name: &str, description: &str) -> AppResult<Space> {
        let row = self
            .db
            .query_one(&self.space_stmts.insert, &[&Uuid::now_v7(), &name, &description, &""])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to insert space"))?;

        Space::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse inserted space row"))
    }

    async fn update_space(&self, id: Uuid, name: &'static str, description: &'static str) -> AppResult<Space> {
        let row = self
            .db
            .query_one(&self.space_stmts.update, &[&id, &name, &description])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update space"))?;

        Space::try_from(row).map_err(|err| ErrType::DbError.err(err, "Failed to parse updated space row"))
    }

    async fn get_default_space(&self, user_id: &Uuid) -> AppResult<Option<Space>> {
        let rows = self
            .db
            .query(&self.default_space_stmts.get_default_space, &[&user_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query default space for user"))?;

        match rows.into_iter().next() {
            Some(row) => {
                Space::try_from(row).map(Some).map_err(|err| ErrType::DbError.err(err, "Failed to parse space row"))
            }
            None => Ok(None),
        }
    }

    async fn set_default_space(&self, user_id: &Uuid) -> AppResult<Space> {
        let space = self
            .insert_space(&format!("{}'s space", user_id), &format!("Default space for {user_id}"))
            .await
            .context("Setting default space")?;

        let rows = self
            .db
            .query(&self.default_space_stmts.set_default_space, &[&space.id, &user_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query insert default space"))?;

        if rows.is_empty() {
            return Err(ErrType::DbError.msg("Failed to insert default space, empty rows"));
        }

        Ok(space)
    }
}
