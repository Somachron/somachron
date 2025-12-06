use lib_core::config;

pub mod native_app;
pub mod space;
pub mod storage;
pub mod user;
pub mod user_space;

pub struct Datastore {
    db: tokio_postgres::Client,
    user_stmts: statements::UserStatements,
    space_stmts: statements::SpaceStatements,
    user_space_stmts: statements::UsersSpacesStatements,
    storage_stmts: statements::StorageStatements,
    native_app_stmts: statements::NativeAppStatements,
}

impl Datastore {
    pub(crate) async fn connect() -> Self {
        let db_config = config::DbConfig::new();

        lib_migrations::migrate_schema(&db_config.url).await;

        let (db, connection) = tokio_postgres::connect(&db_config.url, tokio_postgres::NoTls)
            .await
            .expect("Failed to connect to postgres");

        tokio::spawn(async move {
            if let Err(err) = connection.await {
                eprintln!("Pg connection error: {err}");
            }
        });

        let user_stmts = statements::UserStatements::new(&db).await;
        let space_stmts = statements::SpaceStatements::new(&db).await;
        let user_space_stmts = statements::UsersSpacesStatements::new(&db).await;
        let storage_stmts = statements::StorageStatements::new(&db).await;
        let native_app_stmts = statements::NativeAppStatements::new(&db).await;

        Self {
            db,
            user_stmts,
            space_stmts,
            user_space_stmts,
            storage_stmts,
            native_app_stmts,
        }
    }
}

mod statements {
    use tokio_postgres::types::Type;

    pub struct UserStatements {
        /// SELECT * FROM users WHERE clerk_id = $1
        pub get_by_clerk_id: tokio_postgres::Statement,

        /// SELECT * FROM users WHERE id = $1
        pub get_by_id: tokio_postgres::Statement,

        /// SELECT * FROM users WHERE allowed = true
        pub get_allowed: tokio_postgres::Statement,

        /// INSERT INTO users
        /// (id, clerk_id, email, first_name, last_name, picture_url)
        /// VALUES ($1, $2, $3, $4, $5, $6) RETURNING *
        pub insert: tokio_postgres::Statement,

        /// UPDATE users SET first_name = $2, last_name = $3, picture_url = $4
        /// WHERE id = $1 RETURNING *
        pub update: tokio_postgres::Statement,
    }
    impl UserStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                get_by_clerk_id: db
                    .prepare_typed(r#"SELECT * FROM users WHERE clerk_id = $1"#, &[Type::BPCHAR])
                    .await
                    .unwrap(),
                get_by_id: db.prepare_typed(r#"SELECT * FROM users WHERE id = $1"#, &[Type::UUID]).await.unwrap(),
                get_allowed: db.prepare_typed(r#"SELECT * FROM users WHERE allowed = true"#, &[]).await.unwrap(),
                insert: db
                    .prepare_typed(
                        r#"INSERT INTO users
                        (id, clerk_id, email, first_name, last_name, picture_url)
                        VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"#,
                        &[Type::UUID, Type::BPCHAR, Type::VARCHAR, Type::VARCHAR, Type::VARCHAR, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
                update: db
                    .prepare_typed(
                        r#"UPDATE users SET first_name = $2, last_name = $3, picture_url = $4
                        WHERE id = $1 RETURNING *"#,
                        &[Type::UUID, Type::VARCHAR, Type::VARCHAR, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
            }
        }
    }

    pub struct SpaceStatements {
        /// SELECT * FROM spaces WHERE id = $1
        pub get_by_id: tokio_postgres::Statement,

        /// INSERT INTO spaces
        /// (id, name, description, picture_url)
        /// VALUES ($1, $2, $3, $4) RETURNING *
        pub insert: tokio_postgres::Statement,

        /// UPDATE spaces SET name = $2, description = $3
        /// WHERE id = $1 RETURNING *
        pub update: tokio_postgres::Statement,
    }
    impl SpaceStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                get_by_id: db.prepare_typed(r#"SELECT * FROM spaces WHERE id = $1"#, &[Type::UUID]).await.unwrap(),
                insert: db
                    .prepare_typed(
                        r#"INSERT INTO spaces
                        (id, name, description, picture_url)
                        VALUES ($1, $2, $3, $4) RETURNING *"#,
                        &[Type::UUID, Type::VARCHAR, Type::VARCHAR, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
                update: db
                    .prepare_typed(
                        r#"UPDATE spaces SET name = $2, description = $3
                        WHERE id = $1 RETURNING *"#,
                        &[Type::UUID, Type::VARCHAR, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
            }
        }
    }

    pub struct UsersSpacesStatements {
        /// SELECT * FROM users_spaces WHERE user_id = $1 AND space_id = $2
        pub get_user_space: tokio_postgres::Statement,

        /// SELECT us.*, spaces.*,
        /// (SELECT id FROM fs_node fs WHERE fs.space_id = spaces.id AND node_type = $2 AND parent_node IS NULL) AS root_node
        /// FROM spaces
        /// INNER JOIN (SELECT * FROM users_spaces WHERE user_id = $1) us
        /// ON spaces.id = us.space_id
        pub get_all_spaces_for_user: tokio_postgres::Statement,

        /// SELECT us.*, users.*
        /// FROM users
        /// INNER JOIN (SELECT * FROM users_spaces WHERE space_id = $1) us
        /// ON users.id = us.user_id
        pub get_all_users_for_space: tokio_postgres::Statement,

        /// INSERT INTO users_spaces
        /// (id, user_id, space_id, role)
        /// VALUES ($1, $2, $3, $4) RETURNING *
        pub insert: tokio_postgres::Statement,

        /// UPDATE users_spaces SET role = $2 WHERE id = $1 RETURNING *
        pub update: tokio_postgres::Statement,

        /// DELETE FROM users_spaces WHERE id = $1
        pub delete: tokio_postgres::Statement,
    }
    impl UsersSpacesStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                get_user_space: db
                    .prepare_typed(
                        r#"SELECT * FROM users_spaces WHERE user_id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_all_spaces_for_user: db
                    .prepare_typed(
                        r#"SELECT us.*, spaces.*,
                            (SELECT id FROM fs_node fs
                                WHERE fs.space_id = spaces.id AND node_type = $2 AND parent_node IS NULL) AS root_node
                        FROM spaces
                        INNER JOIN (SELECT * FROM users_spaces WHERE user_id = $1) us
                        ON spaces.id = us.space_id"#,
                        &[Type::UUID, Type::INT2],
                    )
                    .await
                    .unwrap(),
                get_all_users_for_space: db
                    .prepare_typed(
                        r#"SELECT us.*, users.*
                        FROM users
                        INNER JOIN (SELECT * FROM users_spaces WHERE space_id = $1) us
                        ON users.id = us.user_id"#,
                        &[Type::UUID],
                    )
                    .await
                    .unwrap(),
                insert: db
                    .prepare_typed(
                        r#"INSERT INTO users_spaces (id, user_id, space_id, role) VALUES ($1, $2, $3, $4) RETURNING *"#,
                        &[Type::UUID, Type::UUID, Type::UUID, Type::INT2],
                    )
                    .await
                    .unwrap(),
                update: db
                    .prepare_typed(
                        r#"UPDATE users_spaces SET role = $2 WHERE id = $1 RETURNING *"#,
                        &[Type::UUID, Type::INT2],
                    )
                    .await
                    .unwrap(),
                delete: db.prepare_typed(r#"DELETE FROM users_spaces WHERE id = $1"#, &[Type::UUID]).await.unwrap(),
            }
        }
    }

    pub struct StorageStatements {
        /// INSERT INTO fs_node
        /// (id, updated_at, user_id, space_id, node_type, node_size, parent_node, node_name, path, metadata)
        /// VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *
        pub insert_fs_node: tokio_postgres::Statement,

        /// INSERT INTO fs_link
        /// (node_id, child_node_id)
        /// VALUES ($1, $2) RETURNING *
        pub link_fs_node: tokio_postgres::Statement,

        /// SELECT * FROM fs_node
        /// WHERE id = $1 AND node_type = $2 AND space_id = $3
        pub get_fs_node: tokio_postgres::Statement,

        /// SELECT * FROM fs_node
        /// WHERE space_id = $1 AND parent_node = $2 AND node_name = $3
        pub get_node_by_name: tokio_postgres::Statement,

        /// SELECT concat(path, '/', node_name) as og_path,
        ///     concat(path, '/', metadata->'thumbnail_meta'->>'file_name') as th_path
        /// FROM fs_node WHERE id = $1 AND space_id = $2
        pub get_file_stream_paths: tokio_postgres::Statement,

        /// WITH RECURSIVE child_folders AS (
        ///     SELECT * FROM fs_node WHERE id = $1 AND space_id = $2
        ///     UNION ALL
        ///
        ///     -- Recursive step: find children via fs_link
        ///     SELECT fn_child.*
        ///     FROM child_folders cf
        ///         JOIN fs_link fl ON fl.node_id = cf.id
        ///         JOIN (SELECT * FROM fs_node WHERE node_type = $3)
        ///         fn_child ON fn_child.id = fl.child_node_id
        /// )
        /// SELECT *
        /// FROM child_folders
        pub get_inner_folders: tokio_postgres::Statement,

        /// SELECT * FROM fs_node WHERE node_type = $1 AND space_id = $2 AND parent_node = $3
        pub list_nodes: tokio_postgres::Statement,

        /// SELECT id, updated_at, user_id, node_name, metadata->>'media_type' as media_type,
        ///     metadata->'thumbnail_meta'->>'width' as width, metadata->'thumbnail_meta'->>'height' as height
        /// FROM fs_node
        /// WHERE node_type = $1 AND space_id = $2
        /// ORDER BY update_at DESC
        pub list_gallery_nodes: tokio_postgres::Statement,

        /// UPDATE fs_node
        /// SET node_name = $4, node_size = $5, node_type = $6, metadata = $7, updated_at = $8
        /// WHERE id = $1 AND parent_node = $2 AND space_id = $3
        /// RETURNING *
        pub update_node: tokio_postgres::Statement,

        /// DELETE FROM fs_link WHERE node_id = $1 AND child_node_id = $2
        pub unlink_fs_node: tokio_postgres::Statement,

        /// DELETE FROM fs_link WHERE node_id = $1
        pub drop_parent_fs_link: tokio_postgres::Statement,

        /// DELETE FROM fs_link WHERE child_node_id = $1
        pub drop_child_fs_link: tokio_postgres::Statement,

        /// DELETE FROM fs_node WHERE id = $1 AND parent_node = $2 AND space_id = $3
        pub delete_node: tokio_postgres::Statement,
    }
    impl StorageStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                insert_fs_node: db
                    .prepare_typed(
                        r#"INSERT INTO fs_node
                        (id, updated_at, user_id, space_id, node_type, node_size, parent_node, node_name, path, metadata)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *"#,
                        &[
                            Type::UUID,
                            Type::TIMESTAMPTZ,
                            Type::UUID,
                            Type::UUID,
                            Type::INT2,
                            Type::INT8,
                            Type::UUID,
                            Type::VARCHAR,
                            Type::VARCHAR,
                            Type::JSONB,
                        ],
                    )
                    .await
                    .unwrap(),
                link_fs_node: db
                    .prepare_typed(
                        r#"INSERT INTO fs_link (node_id, child_node_id) VALUES ($1, $2) RETURNING *"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_fs_node: db
                    .prepare_typed(
                        r#"SELECT * FROM fs_node WHERE id = $1 AND node_type = $2 AND space_id = $3"#,
                        &[Type::UUID, Type::INT2, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_node_by_name: db
                    .prepare_typed(
                        r#"SELECT * FROM fs_node WHERE space_id = $1 AND parent_node = $2 AND node_name = $3"#,
                        &[Type::UUID, Type::UUID, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
                get_file_stream_paths: db
                    .prepare_typed(
                        r#"SELECT concat(path, '/', node_name) as og_path,
                        concat(path, '/', metadata->'thumbnail_meta'->>'file_name') as th_path
                        FROM fs_node WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_inner_folders: db
                    .prepare_typed(
                        r#"WITH RECURSIVE child_folders AS (

                            SELECT * FROM fs_node WHERE id = $1 AND space_id = $2

                            UNION ALL

                            -- Recursive step: find children via fs_link

                            SELECT fn_child.*
                            FROM child_folders cf
                                JOIN fs_link fl ON fl.node_id = cf.id
                                JOIN (SELECT * FROM fs_node WHERE node_type = $3)
                                fn_child ON fn_child.id = fl.child_node_id
                        )
                        SELECT *
                        FROM child_folders"#,
                        &[Type::UUID, Type::UUID, Type::INT2],
                    )
                    .await
                    .unwrap(),
                list_nodes: db
                    .prepare_typed(
                        r#"SELECT * FROM fs_node WHERE node_type = $1 AND space_id = $2 AND parent_node = $3"#,
                        &[Type::INT2, Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                list_gallery_nodes: db
                    .prepare_typed(
                        r#"SELECT id, updated_at, user_id, node_name, metadata->>'media_type' as media_type,
                            (metadata->'thumbnail_meta'->>'width')::int4 as width, (metadata->'thumbnail_meta'->>'height')::int4 as height
                        FROM fs_node
                        WHERE node_type = $1 AND space_id = $2
                        ORDER BY updated_at DESC"#,
                        &[Type::INT2, Type::UUID],
                    )
                    .await
                    .unwrap(),
                update_node: db
                    .prepare_typed(
                        r#"UPDATE fs_node
                        SET node_name = $4, node_size = $5, node_type = $6, metadata = $7, updated_at = $8
                        WHERE id = $1 AND parent_node = $2 AND space_id = $3 RETURNING *"#,
                        &[Type::UUID, Type::UUID, Type::UUID, Type::VARCHAR, Type::INT8, Type::INT2, Type::JSONB, Type::TIMESTAMPTZ],
                    )
                    .await
                    .unwrap(),
                unlink_fs_node: db
                    .prepare_typed(
                        r#"DELETE FROM fs_link WHERE node_id = $1 AND child_node_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                drop_parent_fs_link: db
                    .prepare_typed(r#"DELETE FROM fs_link WHERE node_id = $1"#, &[Type::UUID])
                    .await
                    .unwrap(),
                drop_child_fs_link: db
                    .prepare_typed(r#"DELETE FROM fs_link WHERE child_node_id = $1"#, &[Type::UUID])
                    .await
                    .unwrap(),
                delete_node: db
                    .prepare_typed(
                        r#"DELETE FROM fs_node WHERE id = $1 AND parent_node = $2 AND space_id = $3"#,
                        &[Type::UUID, Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
            }
        }
    }

    pub struct NativeAppStatements {
        /// SELECT * FROM native_app WHERE secure_identifier = $1
        pub get_app_by_identifier: tokio_postgres::Statement,
    }
    impl NativeAppStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                get_app_by_identifier: db
                    .prepare_typed(r#"SELECT * FROM native_app WHERE secure_identifier = $1"#, &[Type::VARCHAR])
                    .await
                    .unwrap(),
            }
        }
    }
}
