use std::sync::Arc;

use lib_core::{clerk::ClerkAuth, interconnect::ServiceInterconnect, storage::Storage};
use lib_domain::service::AppService;

pub struct App {
    auth: ClerkAuth,
    storage: Storage,
    service: AppService,
    interconnect: ServiceInterconnect,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: ClerkAuth::new(),
            storage: Storage::new().await,
            service: AppService::new().await,
            interconnect: ServiceInterconnect::new(),
        };
        Arc::new(app)
    }

    pub fn auth(&self) -> &ClerkAuth {
        &self.auth
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn service(&self) -> &AppService {
        &self.service
    }

    pub fn interconnect(&self) -> &ServiceInterconnect {
        &self.interconnect
    }
}
