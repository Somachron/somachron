use crate::datastore::Datastore;

mod auth;
mod space;
mod user;

pub struct Service {
    ds: Datastore,
}

impl Service {
    pub async fn new() -> Self {
        Self {
            ds: Datastore::connect().await,
        }
    }

    pub fn ds(&self) -> &Datastore {
        &self.ds
    }
}
