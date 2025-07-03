use chrono::{DateTime, Utc};
use lib_core::{AppError, AppResult, ErrType};

use crate::datastore::{create_id, Datastore};

pub struct Space {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub name: String,
    pub description: String,
    pub picture_url: String,
}
impl From<tokio_postgres::Row> for Space {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            name: value.get(3),
            description: value.get(4),
            picture_url: value.get(5),
        }
    }
}

impl Datastore {
    pub async fn get_space_by_id(&self, id: &str) -> AppResult<Option<Space>> {
        let rows = self
            .client
            .query(&self.space_stmts.get_space_by_id, &[&id])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to query space by id"))?;

        Ok(rows.into_iter().nth(0).map(Space::from))
    }

    pub async fn insert_space(&self, name: &str, description: &str) -> AppResult<Space> {
        let id = create_id();
        let row = self
            .client
            .query_one(&self.space_stmts.insert_space, &[&id, &name, &description, &""])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to insert space"))?;

        if row.is_empty() {
            return Err(AppError::new(ErrType::DbError, "Failed to get inserted space"));
        }

        Ok(Space::from(row))
    }

    pub async fn update_space(&self, id: &str, name: &str, description: &str) -> AppResult<Space> {
        let row = self
            .client
            .query_one(&self.space_stmts.update_space, &[&id, &name, &description])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to update space"))?;

        if row.is_empty() {
            return Err(AppError::new(ErrType::DbError, "Failed to get updated space"));
        }

        Ok(Space::from(row))
    }
}
