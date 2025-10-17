pub async fn migrate_schema(url: &str) {
    let db = sqlx::postgres::PgPoolOptions::new().max_connections(4).connect(url).await.expect("Failed init PgPool");

    sqlx::migrate!("./migrations").run(&db).await.expect("Failed to run migrations");
}
