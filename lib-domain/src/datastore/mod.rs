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
    default_space_stmts: statements::DefaultSpaceStatements,
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
        let default_space_stmts = statements::DefaultSpaceStatements::new(&db).await;
        let user_space_stmts = statements::UsersSpacesStatements::new(&db).await;
        let storage_stmts = statements::StorageStatements::new(&db).await;
        let native_app_stmts = statements::NativeAppStatements::new(&db).await;

        Self {
            db,
            user_stmts,
            space_stmts,
            default_space_stmts,
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

    pub struct DefaultSpaceStatements {
        /// INSERT INTO default_space (space_fk_id, user_fk_id) VALUES ($1, $2) RETURNING *
        pub set_default_space: tokio_postgres::Statement,

        /// SELECT * FROM spaces
        /// WHERE id = (SELECT space_fk_id FROM default_space WHERE user_fk_id = $1)
        pub get_default_space: tokio_postgres::Statement,
    }
    impl DefaultSpaceStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                set_default_space: db
                    .prepare_typed(
                        r#"INSERT INTO default_space (space_fk_id, user_fk_id) VALUES ($1, $2) RETURNING *"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_default_space: db
                    .prepare_typed(
                        r#"SELECT * FROM spaces
                        WHERE id = (SELECT space_fk_id FROM default_space WHERE user_fk_id = $1)"#,
                        &[Type::UUID],
                    )
                    .await
                    .unwrap(),
            }
        }
    }

    pub struct UsersSpacesStatements {
        /// SELECT * FROM users_spaces WHERE user_id = $1 AND space_id = $2
        pub get_user_space: tokio_postgres::Statement,

        /// SELECT us.*, spaces.*
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
                        r#"SELECT us.*, spaces.*
                        FROM spaces
                        INNER JOIN (SELECT * FROM users_spaces WHERE user_id = $1) us
                        ON spaces.id = us.space_id"#,
                        &[Type::UUID],
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
        /// INSERT INTO media_files
        /// (id, updated_at, user_id, space_id, hash, file_name, object_key, node_size, metadata)
        /// VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        /// ON CONFLICT (space_id, hash) DO UPDATE SET updated_at = excluded.updated_at
        /// RETURNING *
        pub upsert_media_file: tokio_postgres::Statement,

        /// SELECT * FROM media_files WHERE id = $1 AND space_id = $2
        pub get_media_file: tokio_postgres::Statement,

        /// SELECT * FROM media_files
        /// INNER JOIN album_media_files ON album_media_files.media_file_id = media_files.id
        /// WHERE album_media_files.album_id = $1 AND media_files.space_id = $2
        pub list_album_media_files: tokio_postgres::Statement,

        /// SELECT id, updated_at, user_id, file_name, metadata->>'media_type' as media_type,
        ///     metadata->'thumbnail_meta'->>'width' as width, metadata->'thumbnail_meta'->>'height' as height
        /// FROM media_files
        /// WHERE space_id = $1
        /// ORDER BY updated_at DESC
        pub list_media_files_gallery: tokio_postgres::Statement,

        /// SELECT thumbnail_key, preview_key FROM media_files
        /// WHERE id = $1 AND space_id = $2
        pub get_media_stream_keys: tokio_postgres::Statement,

        /// SELECT object_key FROM media_files WHERE id = $1 AND space_id = $2
        pub get_media_object_key: tokio_postgres::Statement,

        /// UPDATE media_files
        /// SET file_name = $3, node_size = $4, metadata = $5, updated_at = $6, thumbnail_key = $7, preview_key = $8
        /// WHERE id = $1 AND space_id = $2
        /// RETURNING *
        pub update_media_file: tokio_postgres::Statement,

        /// DELETE FROM media_files WHERE id = $1 AND space_id = $2
        pub delete_media_file: tokio_postgres::Statement,

        /// INSERT INTO albums (id, user_id, space_id, name, legacy_path)
        /// VALUES ($1, $2, $3, $4, $5) RETURNING *
        pub insert_album: tokio_postgres::Statement,

        /// SELECT * FROM albums WHERE id = $1 AND space_id = $2
        pub get_album: tokio_postgres::Statement,

        /// SELECT * FROM albums WHERE space_id = $1 ORDER BY name ASC, created_at ASC
        pub list_albums: tokio_postgres::Statement,

        /// DELETE FROM albums WHERE id = $1 AND space_id = $2
        pub delete_album: tokio_postgres::Statement,

        /// INSERT INTO album_media_files (album_id, media_file_id)
        /// SELECT a.id, m.id FROM albums a
        /// INNER JOIN media_files m ON m.id = $2
        /// WHERE a.id = $1 AND a.space_id = $3 AND m.space_id = $3
        /// ON CONFLICT DO NOTHING
        pub link_album_media_file: tokio_postgres::Statement,

        /// DELETE FROM album_media_files amf USING albums a, media_files m
        /// WHERE amf.album_id = a.id AND amf.media_file_id = m.id
        /// AND a.id = $1 AND m.id = $2 AND a.space_id = $3 AND m.space_id = $3
        pub unlink_album_media_file: tokio_postgres::Statement,
    }
    impl StorageStatements {
        pub async fn new(db: &tokio_postgres::Client) -> Self {
            Self {
                upsert_media_file: db
                    .prepare_typed(
                        r#"INSERT INTO media_files
                        (id, updated_at, user_id, space_id, hash, file_name, object_key, node_size, metadata)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                        ON CONFLICT (space_id, hash)
                        DO UPDATE SET updated_at = EXCLUDED.updated_at
                        RETURNING *"#,
                        &[
                            Type::UUID,
                            Type::TIMESTAMPTZ,
                            Type::UUID,
                            Type::UUID,
                            Type::BPCHAR,
                            Type::VARCHAR,
                            Type::VARCHAR,
                            Type::INT8,
                            Type::JSONB,
                        ],
                    )
                    .await
                    .unwrap(),
                get_media_file: db
                    .prepare_typed(
                        r#"SELECT * FROM media_files WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                list_album_media_files: db
                    .prepare_typed(
                        r#"SELECT media_files.*
                        FROM media_files
                        INNER JOIN album_media_files amf ON amf.media_file_id = media_files.id
                        WHERE amf.album_id = $1 AND media_files.space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                list_media_files_gallery: db
                    .prepare_typed(
                        r#"SELECT id, updated_at, user_id, file_name, metadata->>'media_type' as media_type,
                            (metadata->'thumbnail_meta'->>'width')::int4 as width, (metadata->'thumbnail_meta'->>'height')::int4 as height
                        FROM media_files
                        WHERE space_id = $1
                        ORDER BY updated_at DESC"#,
                        &[Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_media_stream_keys: db
                    .prepare_typed(
                        r#"SELECT thumbnail_key, preview_key
                        FROM media_files WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                get_media_object_key: db
                    .prepare_typed(
                        r#"SELECT object_key
                        FROM media_files WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                update_media_file: db
                    .prepare_typed(
                        r#"UPDATE media_files
                        SET file_name = $3, node_size = $4, metadata = $5, updated_at = $6, thumbnail_key = $7, preview_key = $8
                        WHERE id = $1 AND space_id = $2
                        RETURNING *"#,
                        &[
                            Type::UUID,
                            Type::UUID,
                            Type::VARCHAR,
                            Type::INT8,
                            Type::JSONB,
                            Type::TIMESTAMPTZ,
                            Type::VARCHAR,
                            Type::VARCHAR,
                        ],
                    )
                    .await
                    .unwrap(),
                delete_media_file: db
                    .prepare_typed(
                        r#"DELETE FROM media_files WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                insert_album: db
                    .prepare_typed(
                        r#"INSERT INTO albums (id, user_id, space_id, name, legacy_path)
                        VALUES ($1, $2, $3, $4, $5)
                        RETURNING *"#,
                        &[Type::UUID, Type::UUID, Type::UUID, Type::VARCHAR, Type::VARCHAR],
                    )
                    .await
                    .unwrap(),
                get_album: db
                    .prepare_typed(
                        r#"SELECT * FROM albums WHERE id = $1 AND space_id = $2"#,
                        &[Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                list_albums: db
                    .prepare_typed(
                        r#"SELECT * FROM albums WHERE space_id = $1 ORDER BY name ASC, created_at ASC"#,
                        &[Type::UUID],
                    )
                    .await
                    .unwrap(),
                delete_album: db
                    .prepare_typed(r#"DELETE FROM albums WHERE id = $1 AND space_id = $2"#, &[Type::UUID, Type::UUID])
                    .await
                    .unwrap(),
                link_album_media_file: db
                    .prepare_typed(
                        r#"INSERT INTO album_media_files (album_id, media_file_id)
                        SELECT a.id, m.id
                        FROM albums a
                        INNER JOIN media_files m ON m.id = $2
                        WHERE a.id = $1 AND a.space_id = $3 AND m.space_id = $3
                        ON CONFLICT DO NOTHING"#,
                        &[Type::UUID, Type::UUID, Type::UUID],
                    )
                    .await
                    .unwrap(),
                unlink_album_media_file: db
                    .prepare_typed(
                        r#"DELETE FROM album_media_files amf
                        USING albums a, media_files m
                        WHERE amf.album_id = a.id
                          AND amf.media_file_id = m.id
                          AND a.id = $1
                          AND m.id = $2
                          AND a.space_id = $3
                          AND m.space_id = $3"#,
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
