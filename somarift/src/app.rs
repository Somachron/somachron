use std::sync::Arc;

use lib_core::{clerk::ClerkAuth, interconnect::ServiceInterconnect, storage::Storage};
use lib_domain::service::AppServices;

pub struct App {
    auth: ClerkAuth,
    storage: Storage,
    services: AppServices,
    interconnect: ServiceInterconnect,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: ClerkAuth::new(),
            storage: Storage::new().await,
            services: AppServices::new().await,
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

    pub fn services(&self) -> &AppServices {
        &self.services
    }

    pub fn interconnect(&self) -> &ServiceInterconnect {
        &self.interconnect
    }
}
