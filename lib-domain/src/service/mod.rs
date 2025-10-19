use crate::datastore::Datastore;

mod auth;
mod cloud;
mod space;
mod user;
mod user_space;

pub type AppService = Service<Datastore>;

pub struct Service<D> {
    ds: D,
}

impl Service<Datastore> {
    pub async fn new() -> Self {
        Self {
            ds: Datastore::connect().await,
        }
    }

    pub fn ds(&self) -> &Datastore {
        &self.ds
    }
}
