use lib_core::config;
use nanoid::nanoid;
use tokio_postgres::{Client, NoTls};

mod statements;
pub mod user;

pub struct Datastore {
    client: Client,
    user_stmts: statements::UserStatements,
}

impl Datastore {
    pub async fn connect() -> Self {
        let url = config::get_db_url();

        Self::migrate(url).await;

        let (client, connection) = tokio_postgres::connect(url, NoTls).await.expect("Failed to connect to postgres");

        // spawn connection
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {e}");
            }
        });

        // prepared statements
        let user_stmts = statements::UserStatements::new(&client).await;

        Self {
            client,
            user_stmts,
        }
    }

    async fn migrate(url: &str) {
        let db =
            sqlx::postgres::PgPoolOptions::new().max_connections(4).connect(url).await.expect("Failed init PgPool");

        sqlx::migrate!("./migrations").run(&db).await.expect("Failed to run migrations");
    }
}

fn create_id() -> String {
    nanoid!(8)
}
