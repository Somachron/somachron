use axum::routing::{get, Router};

use crate::app::AppState;

pub fn bind_routes() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status=200, description="Health check API")),
    tag = "Health"
)]
pub async fn health() -> &'static str {
    "Server is up and running 🚀🚀"
}
