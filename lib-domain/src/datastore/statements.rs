use tokio_postgres::{types::Type, Client, Statement};

pub(super) struct UserStatements {
    /// `SELECT * FROM users WHERE email = $1`
    pub(super) get_user_by_email: Statement,

    /// `INSERT INTO users (id, given_name, email, picture_url) VALUES ($1, $2, $3, $4) RETURNING *`
    pub(super) insert_user: Statement,

    /// `UPDATE users SET given_name = $1, picture_url = $2 WHERE id = $3 RETURNING *`
    pub(super) update_user: Statement,
}

impl UserStatements {
    pub(super) async fn new(client: &Client) -> Self {
        Self {
            get_user_by_email: client
                .prepare_typed("SELECT * FROM users WHERE email = $1", &[Type::TEXT])
                .await
                .unwrap(),
            insert_user: client
                .prepare_typed(
                    "INSERT INTO users (id, given_name, email, picture_url) VALUES ($1, $2, $3, $4) RETURNING *",
                    &[Type::BPCHAR, Type::TEXT, Type::TEXT, Type::TEXT],
                )
                .await
                .unwrap(),
            update_user: client
                .prepare_typed(
                    "UPDATE users SET given_name = $1, picture_url = $2 WHERE id = $3 RETURNING *",
                    &[Type::TEXT, Type::TEXT, Type::BPCHAR],
                )
                .await
                .unwrap(),
        }
    }
}

pub(super) struct SpaceStatements {
    /// `SELECT * FROM spaces WHERE id = $1`
    pub(super) get_space_by_id: Statement,

    /// `INSERT INTO spaces(id, name, description, picture_url) VALUES ($1, $2, $3, $4) RETURNING *`
    pub(super) insert_space: Statement,

    /// `UPDATE spaces SET name = $1, description = $2 WHERE id = $3 RETURNING *`
    pub(super) update_space: Statement,
}

impl SpaceStatements {
    pub(super) async fn new(client: &Client) -> Self {
        Self {
            get_space_by_id: client.prepare_typed("SELECT * FROM spaces WHERE id = $1", &[Type::BPCHAR]).await.unwrap(),
            insert_space: client
                .prepare_typed(
                    "INSERT INTO spaces(id, name, description, picture_url) VALUES ($1, $2, $3, $4) RETURNING *",
                    &[Type::BPCHAR, Type::TEXT, Type::TEXT, Type::TEXT],
                )
                .await
                .unwrap(),
            update_space: client
                .prepare_typed(
                    "UPDATE spaces SET name = $1, description = $2 WHERE id = $3 RETURNING *",
                    &[Type::TEXT, Type::TEXT, Type::BPCHAR],
                )
                .await
                .unwrap(),
        }
    }
}
