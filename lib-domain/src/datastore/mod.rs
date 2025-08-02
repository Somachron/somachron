use lib_core::config;
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    RecordId, Surreal,
};

pub mod space;
pub mod storage;
pub mod user;
pub mod user_space;

pub struct Datastore {
    db: Surreal<Client>,
}

impl Datastore {
    pub(crate) async fn connect() -> Self {
        let db_config = config::DbConfig::new();

        let url = db_config.url;
        let db = Surreal::new::<Ws>(&url).await.expect(&format!("Failed to connect to db: {url}"));

        db.signin(Root {
            username: &db_config.username,
            password: &db_config.password,
        })
        .await
        .expect("Failed to sign into db");

        db.use_ns("somachron").use_db("somachron").await.expect("Failed to select ns and db");

        Self {
            db,
        }
    }
}

trait DbSchema {
    fn table_name() -> &'static str;

    fn get_id(key: &str) -> RecordId {
        RecordId::from_table_key(Self::table_name(), key)
    }
}
