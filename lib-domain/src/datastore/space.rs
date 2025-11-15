use chrono::{DateTime, Utc};
use lib_core::{AppResult, ErrType};
use tokio_postgres::types::Type;
use uuid::Uuid;

use crate::datastore::storage::NodeType;

use super::Datastore;

pub struct Space {
    pub id: Uuid,
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

pub trait SpaceDs {
    fn get_space_by_id(&self, id: &Uuid) -> impl Future<Output = AppResult<Option<Space>>>;
    fn insert_space(&self, name: &str, description: &str) -> impl Future<Output = AppResult<Space>>;
    fn update_space(
        &self,
        id: Uuid,
        name: &'static String,
        description: &'static String,
    ) -> impl Future<Output = AppResult<Space>>;

    //--- MIG
    fn get_all_spaces(&self) -> impl Future<Output = AppResult<Vec<Space>>>;
    fn get_space_root(&self, space_id: &Uuid) -> impl Future<Output = AppResult<Uuid>>;
}

impl SpaceDs for Datastore {
    //--- MIG
    async fn get_all_spaces(&self) -> AppResult<Vec<Space>> {
        let st = self.db.prepare(r#"SELECT * FROM spaces"#).await.unwrap();
        let rows =
            self.db.query(&st, &[]).await.map_err(|err| ErrType::DbError.err(err, "Failed to get all spaces"))?;

        Ok(rows.into_iter().map(Space::from).collect())
    }
    async fn get_space_root(&self, space_id: &Uuid) -> AppResult<Uuid> {
        let st = self
            .db
            .prepare_typed(
                r#"SELECT id FROM fs_node WHERE space_id = $1 AND node_type = $2 AND parent_node is null"#,
                &[Type::UUID, Type::INT2],
            )
            .await
            .unwrap();

        let rows = self
            .db
            .query(&st, &[&space_id, &NodeType::Folder.value()])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get space root folder"))?;

        Ok(rows.into_iter().nth(0).map(|r| r.get(0)).unwrap())
    }

    async fn get_space_by_id(&self, id: &Uuid) -> AppResult<Option<Space>> {
        let rows = self
            .db
            .query(&self.space_stmts.get_by_id, &[&id])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get space by id"))?;

        Ok(rows.into_iter().nth(0).map(Space::from))
    }

    async fn insert_space(&self, name: &str, description: &str) -> AppResult<Space> {
        let row = self
            .db
            .query_one(&self.space_stmts.insert, &[&Uuid::now_v7(), &name, &description, &""])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to insert space"))?;

        Ok(Space::from(row))
    }

    async fn update_space(&self, id: Uuid, name: &'static String, description: &'static String) -> AppResult<Space> {
        let row = self
            .db
            .query_one(&self.space_stmts.update, &[&id, &name, &description])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to update space"))?;

        Ok(Space::from(row))
    }
}
