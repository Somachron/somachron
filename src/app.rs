use std::sync::Arc;

use lib_core::{google::GoogleAuth, storage::Storage};
use lib_domain::service::Service;

pub struct App {
    auth: GoogleAuth,
    storage: Storage,
    service: Service,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: GoogleAuth::new().await,
            storage: Storage::new().await,
            service: Service::new().await,
        };
        Arc::new(app)
    }

    pub fn auth(&self) -> &GoogleAuth {
        &self.auth
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn service(&self) -> &Service {
        &self.service
    }
}
