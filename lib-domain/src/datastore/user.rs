use chrono::{DateTime, Utc};
use lib_core::{clerk::TokenClaims, AppResult, ErrType};
use serde::Deserialize;
use surrealdb::RecordId;

use crate::datastore::DbSchema;

use super::Datastore;

#[derive(Deserialize)]
pub struct User {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub given_name: String,
    pub email: String,
    pub picture_url: String,
    pub allowed: bool,
}
impl DbSchema for User {
    fn table_name() -> &'static str {
        "user"
    }
}

impl Datastore {
    pub async fn get_user_by_clerk_id(&self, clerk_id: &str) -> AppResult<Option<User>> {
        let mut res = self
            .db
            .query("SELECT * FROM user WHERE clerk_id = $e")
            .bind(("e", clerk_id.to_owned()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query check for user using clerk id"))?;

        let users: Vec<User> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize user"))?;

        Ok(users.into_iter().nth(0))
    }

    pub async fn get_user_by_id(&self, id: RecordId) -> AppResult<Option<User>> {
        self.db.select(id).await.map_err(|err| ErrType::DbError.err(err, "Failed to query user by email"))
    }

    pub async fn get_platform_users(&self) -> AppResult<Vec<User>> {
        let mut res = self
            .db
            .query("SELECT * FROM user WHERE allowed = true")
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get platform users"))?;

        res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize platform users"))
    }

    pub async fn insert_user(&self, claims: TokenClaims) -> AppResult<User> {
        let mut res = self
            .db
            .query("CREATE user SET given_name = $n, clerk_id = $c, email = $e, picture_url = $p, allowed = false")
            .bind(("n", claims.name))
            .bind(("c", claims.sub))
            .bind(("e", claims.email))
            .bind(("p", claims.picture))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query insert user"))?;

        let users: Vec<User> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize user"))?;

        users.into_iter().nth(0).ok_or(ErrType::DbError.msg("Failed to create requested user"))
    }

    pub async fn update_user(&self, id: RecordId, given_name: &str, picture_url: &str) -> AppResult<User> {
        let mut res = self
            .db
            .query("UPDATE $id SET given_name = $n, picture_url = $p")
            .bind(("id", id))
            .bind(("n", given_name.to_owned()))
            .bind(("p", picture_url.to_owned()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query update user"))?;

        let users: Vec<User> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize user"))?;

        users.into_iter().nth(0).ok_or(ErrType::ServerError.msg("Failed to update requested user"))
    }
}
