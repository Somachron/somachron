use chrono::{DateTime, Utc};
use lib_core::{clerk::TokenClaims, AppResult, ErrType};
use uuid::Uuid;

use super::Datastore;

pub struct User {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub allowed: bool,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub picture_url: String,
}
impl From<tokio_postgres::Row> for User {
    fn from(value: tokio_postgres::Row) -> Self {
        Self {
            id: value.get(0),
            created_at: value.get(1),
            updated_at: value.get(2),
            allowed: value.get(3),
            // clerk_id: 4
            email: value.get(5),
            first_name: value.get(6),
            last_name: value.get(7),
            picture_url: value.get(8),
        }
    }
}

pub trait UserDs {
    fn get_user_by_clerk_id(&self, clerk_id: &str) -> impl Future<Output = AppResult<Option<User>>>;
    fn get_user_by_id(&self, id: Uuid) -> impl Future<Output = AppResult<Option<User>>>;
    fn get_platform_users(&self) -> impl Future<Output = AppResult<Vec<User>>>;
    fn insert_user(&self, claims: TokenClaims) -> impl Future<Output = AppResult<User>>;
    fn update_user(
        &self,
        id: Uuid,
        first_name: &str,
        last_name: &str,
        picture_url: &str,
    ) -> impl Future<Output = AppResult<User>>;
}

impl UserDs for Datastore {
    async fn get_user_by_clerk_id(&self, clerk_id: &str) -> AppResult<Option<User>> {
        let rows = self
            .db
            .query(&self.user_stmts.get_by_clerk_id, &[&clerk_id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to check user by clerk id"))?;

        Ok(rows.into_iter().nth(0).map(User::from))
    }

    async fn get_user_by_id(&self, id: Uuid) -> AppResult<Option<User>> {
        let rows = self
            .db
            .query(&self.user_stmts.get_by_id, &[&id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get user by id"))?;

        Ok(rows.into_iter().nth(0).map(User::from))
    }

    async fn get_platform_users(&self) -> AppResult<Vec<User>> {
        let rows = self
            .db
            .query(&self.user_stmts.get_allowed, &[])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get allowed users"))?;

        Ok(rows.into_iter().map(User::from).collect())
    }

    async fn insert_user(&self, claims: TokenClaims) -> AppResult<User> {
        let row = self
            .db
            .query_one(
                &self.user_stmts.insert,
                &[&Uuid::now_v7(), &claims.sub, &claims.email, &claims.name, &"", &claims.picture],
            )
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to insert user"))?;

        Ok(User::from(row))
    }

    async fn update_user(&self, id: Uuid, first_name: &str, last_name: &str, picture_url: &str) -> AppResult<User> {
        let row = self
            .db
            .query_one(&self.user_stmts.update, &[&id, &first_name, &last_name, &picture_url])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update user"))?;

        Ok(User::from(row))
    }
}
