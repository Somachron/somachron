use chrono::{DateTime, Utc};
use lib_core::{AppError, AppResult, ErrType};

use super::{create_id, Datastore};

pub struct User {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub given_name: String,
    pub email: String,
    pub picture_url: String,
}
impl From<tokio_postgres::Row> for User {
    fn from(value: tokio_postgres::Row) -> Self {
        User {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            given_name: value.get(3),
            email: value.get(4),
            picture_url: value.get(5),
        }
    }
}

impl Datastore {
    /// Get user by email
    pub async fn get_user_by_email(&self, email: &str) -> AppResult<Option<User>> {
        let rows = self
            .client
            .query(&self.user_stmts.get_user_by_email, &[&email])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to query user by email"))?;

        Ok(rows.into_iter().nth(0).map(|u| User::from(u)))
    }

    pub async fn insert_user(&self, given_name: &str, email: &str, picture_url: &str) -> AppResult<User> {
        let id = create_id();
        let row = self
            .client
            .query_one(&self.user_stmts.insert_user, &[&id, &given_name, &email, &picture_url])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to insert user"))?;

        if row.is_empty() {
            return Err(AppError::new(ErrType::DbError, "Failed to get inserted user"));
        }

        Ok(User::from(row))
    }

    pub async fn update_user(&self, id: &str, given_name: &str, picture_url: &str) -> AppResult<User> {
        let row = self
            .client
            .query_one(&self.user_stmts.update_user, &[&given_name, &picture_url, &id])
            .await
            .map_err(|err| AppError::err(ErrType::DbError, err, "Failed to update user"))?;

        if row.is_empty() {
            return Err(AppError::new(ErrType::DbError, "Failed to get updated user"));
        }

        Ok(User::from(row))
    }
}
