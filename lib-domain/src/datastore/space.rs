use chrono::{DateTime, Utc};
use lib_core::{AppResult, ErrType};
use serde::Deserialize;
use surrealdb::RecordId;

use crate::datastore::DbSchema;

use super::Datastore;

#[derive(Deserialize)]
pub struct Space {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub name: String,
    pub description: String,
    pub picture_url: String,

    pub folder: RecordId,
}
impl DbSchema for Space {
    fn table_name() -> &'static str {
        "space"
    }
}

impl Datastore {
    pub async fn get_space_by_id(&self, id: &str) -> AppResult<Option<Space>> {
        let id = Space::get_id(id);
        self.db.select(id).await.map_err(|err| ErrType::DbError.err(err, "Failed to query space by id"))
    }

    pub async fn insert_space(&self, name: &str, description: &str) -> AppResult<Space> {
        let mut res = self
            .db
            .query("CREATE space SET name = $n, description = $d, picture_url = ''")
            .bind(("n", name.to_owned()))
            .bind(("d", description.to_owned()))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query insert space"))?;

        let spaces: Vec<Space> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize space"))?;

        spaces.into_iter().nth(0).ok_or(ErrType::DbError.msg("Failed to create requested space"))
    }

    pub async fn update_space(
        &self,
        id: &str,
        name: &'static String,
        description: &'static String,
    ) -> AppResult<Space> {
        let id = Space::get_id(id);
        let mut res = self
            .db
            .query("UPDATE $id SET name = $n, description = $d")
            .bind(("id", id))
            .bind(("n", name))
            .bind(("d", description))
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to query update space"))?;

        let spaces: Vec<Space> = res.take(0).map_err(|err| ErrType::DbError.err(err, "Failed to deserialize space"))?;

        spaces.into_iter().nth(0).ok_or(ErrType::DbError.msg("Failed to update requested space"))
    }
}
