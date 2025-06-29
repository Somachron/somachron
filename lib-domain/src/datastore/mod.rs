use lib_core::config;
use tokio_postgres::{Client, NoTls};

pub struct Datastore {
    client: Client,
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

        Self {
            client,
        }
    }

    async fn migrate(url: &str) {
        let db =
            sqlx::postgres::PgPoolOptions::new().max_connections(4).connect(url).await.expect("Failed init PgPool");

        sqlx::migrate!("./migrations").run(&db).await.expect("Failed to run migrations");
    }
}
