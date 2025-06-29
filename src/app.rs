use std::sync::Arc;

use lib_core::{google::GoogleAuth, r2::R2Storage};
use lib_domain::service::Service;

pub struct App {
    auth: GoogleAuth,
    r2: R2Storage,
    service: Service,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: GoogleAuth::new().await,
            r2: R2Storage::new(),
            service: Service::new().await,
        };
        Arc::new(app)
    }

    pub fn auth(&self) -> &GoogleAuth {
        &self.auth
    }

    pub fn r2(&self) -> &R2Storage {
        &self.r2
    }

    pub fn service(&self) -> &Service {
        &self.service
    }
}
