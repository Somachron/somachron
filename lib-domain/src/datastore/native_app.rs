use lib_core::{AppResult, ErrType};
use uuid::Uuid;

use crate::datastore::Datastore;

pub struct NativeApp {
    pub id: Uuid,
}

pub trait NativeAppDs: Send + Sync {
    fn validate_native_app(&self, identifier: String) -> impl Future<Output = AppResult<()>> + Send;
}

impl NativeAppDs for Datastore {
    async fn validate_native_app(&self, identifier: String) -> AppResult<()> {
        let rows = self
            .db
            .query(&self.native_app_stmts.get_app_by_identifier, &[&identifier])
            .await
            .map_err(|err| ErrType::DbError.err(err, "Failed to get native app by identifier"))?;

        if rows.is_empty() {
            return Err(ErrType::Unauthorized.msg("Invalid build"));
        }

        Ok(())
    }
}
