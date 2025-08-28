use std::sync::Arc;

use lib_core::{clerk::ClerkAuth, storage::Storage};
use lib_domain::service::Service;

pub struct App {
    auth: ClerkAuth,
    storage: Storage,
    service: Service,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: ClerkAuth::new(),
            storage: Storage::new().await,
            service: Service::new().await,
        };
        Arc::new(app)
    }

    pub fn auth(&self) -> &ClerkAuth {
        &self.auth
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn service(&self) -> &Service {
        &self.service
    }
}
