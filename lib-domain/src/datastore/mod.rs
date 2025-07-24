use lib_core::{config, extensions::UserRole};
use nanoid::nanoid;
use postgres_types::{FromSql, Kind, Oid, ToSql, Type};
use sonic_rs::Serialize;
use tokio_postgres::{Client, NoTls};
use utoipa::ToSchema;

pub mod space;
mod statements;
pub mod user;
pub mod user_space;

pub struct Datastore {
    client: Client,
    user_stmts: statements::UserStatements,
    space_stmts: statements::SpaceStatements,
    user_space_stmts: statements::UserSpaceStatements,
}

impl Datastore {
    pub(crate) async fn connect() -> Self {
        let url = config::get_db_url();

        lib_migrations::migrate_schema(&url).await;

        let (client, connection) = tokio_postgres::connect(&url, NoTls).await.expect("Failed to connect to postgres");

        // spawn connection
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {e}");
            }
        });

        // prepared statements
        let user_stmts = statements::UserStatements::new(&client).await;
        let space_stmts = statements::SpaceStatements::new(&client).await;
        let user_space_stmts = statements::UserSpaceStatements::new(&client).await;

        Self {
            client,
            user_stmts,
            space_stmts,
            user_space_stmts,
        }
    }
}

fn create_id() -> String {
    nanoid!(8)
}

#[derive(Debug, ToSql, FromSql, ToSchema, Serialize)]
#[postgres(name = "space_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpaceRole {
    Owner,
    Read,
    Upload,
    Modify,
}

impl From<SpaceRole> for UserRole {
    fn from(value: SpaceRole) -> Self {
        match value {
            SpaceRole::Owner => UserRole::Owner,
            SpaceRole::Read => UserRole::Read,
            SpaceRole::Upload => UserRole::Upload,
            SpaceRole::Modify => UserRole::Modify,
        }
    }
}

impl SpaceRole {
    pub async fn get_type(client: &Client) -> Type {
        let kind = Kind::Enum(vec!["owner".into(), "read".into(), "upload".into(), "modify".into()]);

        let row = client
            .query_one("SELECT oid FROM pg_type WHERE typname = 'space_role'", &[])
            .await
            .expect("Failed to get oid for space_role");
        let oid: Oid = row.get(0);

        Type::new("space_role".into(), oid, kind, "public".into())
    }
}
