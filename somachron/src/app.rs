use std::sync::Arc;

use lib_core::{clerk::ClerkAuth, storage::Storage};
use lib_domain::service::AppService;

pub struct App {
    auth: ClerkAuth,
    storage: Storage,
    service: AppService,
}

pub type AppState = Arc<App>;

impl App {
    pub async fn new() -> AppState {
        let app = App {
            auth: ClerkAuth::new(),
            storage: Storage::new().await,
            service: AppService::new().await,
        };
        Arc::new(app)
    }

    pub async fn bootstrap(&self) {
        if let Err(err) = self.service().update_thumbnail_sizes(&self.storage).await {
            eprint!("Error bootstraping: {err:?}");
        }
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
}
