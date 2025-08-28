use axum::Router;
use lib_core::config;
use tokio::signal;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    app::{App, AppState},
    routes,
};

/// Serves axum backend server
pub async fn serve() {
    let app = App::new().await;

    // build our application with a route
    // bind routes
    let router = get_router(app).await;

    let addr = config::get_host_addr();
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Failed to start TCP listener");
    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await.expect("Failed to serve");
}

pub async fn get_router(app: AppState) -> Router {
    // Prepare swagger
    let swagger = SwaggerUi::new("/v1/swagger").url("/v1/api-docs/openapi.json", routes::ApiDoc::openapi());

    routes::bind_routes(app.clone(), Router::<AppState>::new())
        .merge(swagger)
        .layer(axum::middleware::from_fn(lib_core::interceptor::intercept))
        .with_state(app)
}

/// Function that listens to signals and notify waiters
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
