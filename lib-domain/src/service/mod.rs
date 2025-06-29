use crate::datastore::Datastore;

mod auth;

pub struct Service {
    ds: Datastore,
}

impl Service {
    pub async fn new() -> Self {
        Self {
            ds: Datastore::connect().await,
        }
    }
}
