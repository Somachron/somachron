use tokio_postgres::{types::Type, Client, Statement};

use super::SpaceRole;

pub(super) struct UserStatements {
    /// `SELECT id FROM users WHERE email = $1`
    pub(super) get_user_id: Statement,

    /// `SELECT * FROM users WHERE id = $1`
    pub(super) get_user_by_id: Statement,

    /// `INSERT INTO users (id, given_name, email, picture_url) VALUES ($1, $2, $3, $4) RETURNING *`
    pub(super) insert_user: Statement,

    /// `UPDATE users SET given_name = $1, picture_url = $2 WHERE id = $3 RETURNING *`
    pub(super) update_user: Statement,
}

impl UserStatements {
    pub(super) async fn new(client: &Client) -> Self {
        Self {
            get_user_id: client.prepare_typed("SELECT * FROM users WHERE email = $1", &[Type::TEXT]).await.unwrap(),
            get_user_by_id: client.prepare_typed("SELECT * FROM users WHERE id = $1", &[Type::BPCHAR]).await.unwrap(),
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

pub(super) struct UserSpaceStatements {
    /// `SELECT spaces.*, users_spaces.role FROM spaces INNER JOIN users_spaces ON spaces.id = users_spaces.space_id WHERE users_spaces.user_id = $1`
    pub(super) get_spaces_for_user: Statement,

    /// `SELECT users.*, users_spaces.role FROM users INNER JOIN users_spaces ON users.id = users_spaces.user_id WHERE users_spaces.space_id = $1`
    pub(super) get_users_for_space: Statement,

    /// `SELECT spaces.*, users_spaces.role FROM spaces INNER JOIN users_spaces ON spaces.id = users_spaces.space_id WHERE users_spaces.user_id = $1 AND users_spaces.space_id = $2`
    pub(super) get_user_space: Statement,

    /// `INSERT INTO users_spaces (id, user_id, space_id, role) VALUES ($1, $2, $3, $4) RETURNING space_id`
    pub(super) add_user_to_space: Statement,

    /// `DELETE FROM users_spaces WHERE space_id = $1 AND user_id = $2`
    pub(super) remove_user_from_space: Statement,

    /// `UPDATE users_spaces SET role = $1 where space_id = $2 AND user_id = $3 RETURNING id`
    pub(super) update_user_space_role: Statement,
}

impl UserSpaceStatements {
    pub(super) async fn new(client: &Client) -> Self {
        let enum_type = SpaceRole::get_type(client).await;

        Self {
            get_spaces_for_user: client
                .prepare_typed(
                    "SELECT spaces.*, users_spaces.role FROM spaces
                    INNER JOIN users_spaces ON spaces.id = users_spaces.space_id
                    WHERE users_spaces.user_id = $1",
                    &[Type::BPCHAR],
                )
                .await
                .unwrap(),
            get_users_for_space: client
                .prepare_typed(
                    "SELECT users.*, users_spaces.role FROM users
                    INNER JOIN users_spaces ON users.id = users_spaces.user_id
                    WHERE users_spaces.space_id = $1",
                    &[Type::BPCHAR],
                )
                .await
                .unwrap(),
            get_user_space: client
                .prepare_typed(
                    "SELECT spaces.*, users_spaces.role FROM spaces
                    INNER JOIN users_spaces ON spaces.id = users_spaces.space_id
                    WHERE users_spaces.user_id = $1 AND users_spaces.space_id = $2",
                    &[Type::BPCHAR, Type::BPCHAR],
                )
                .await
                .unwrap(),
            add_user_to_space: client
                .prepare_typed(
                    "INSERT INTO users_spaces (id, user_id, space_id, role) VALUES ($1, $2, $3, $4) RETURNING space_id",
                    &[Type::BPCHAR, Type::BPCHAR, Type::BPCHAR, enum_type.clone()],
                )
                .await
                .unwrap(),
            remove_user_from_space: client
                .prepare_typed(
                    "DELETE FROM users_spaces WHERE space_id = $1 AND user_id = $2",
                    &[Type::BPCHAR, Type::BPCHAR],
                )
                .await
                .unwrap(),
            update_user_space_role: client
                .prepare_typed(
                    "UPDATE users_spaces SET role = $1 where space_id = $2 AND user_id = $3 RETURNING id",
                    &[enum_type, Type::BPCHAR, Type::BPCHAR],
                )
                .await
                .unwrap(),
        }
    }
}
