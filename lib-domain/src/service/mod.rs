use crate::service::{
    auth::AuthService, media::MediaService, space::SpaceService, user::UserService, user_space::UserSpaceService,
};

use super::datastore::Datastore;

pub mod auth;
pub mod media;
pub mod space;
pub mod user;
pub mod user_space;

pub type AppServices = Service<Datastore>;

pub struct ServiceWrapper<'d, D> {
    ds: &'d D,
}

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

    pub fn auth_service(&self) -> impl AuthService + Send + Sync {
        ServiceWrapper {
            ds: &self.ds,
        }
    }

    pub fn media_service(&self) -> impl MediaService + Send + Sync {
        ServiceWrapper {
            ds: &self.ds,
        }
    }

    pub fn user_service(&self) -> impl UserService + Send + Sync {
        ServiceWrapper {
            ds: &self.ds,
        }
    }

    pub fn space_service(&self) -> impl SpaceService + Send + Sync {
        ServiceWrapper {
            ds: &self.ds,
        }
    }

    pub fn user_space_service(&self) -> impl UserSpaceService + Send + Sync {
        ServiceWrapper {
            ds: &self.ds,
        }
    }
}
