use std::time::Duration;

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use super::extensions::ReqId;

const REQ_ID_HEADER: &str = "x-sc-id";

/// Interceptor to log request and response
///
/// This interceptor also inserts request ID for tracing
pub async fn intercept(mut req: Request, next: Next) -> Response {
    let id = ReqId(nanoid::nanoid!(22));
    let uri = req.uri().clone();

    let method = req.method().clone();
    tracing::info!(req_id = &id.0, message = format!("{}: {}", method, uri), method = ?method, uri = ?uri);

    req.extensions_mut().insert(id.clone());

    let start = std::time::Instant::now();
    let mut res = next.run(req).await;
    res.headers_mut().insert(REQ_ID_HEADER, HeaderValue::from_str(&id.0).unwrap());

    let latency = start.elapsed();
    log_response(id, res.status(), latency);
    res
}

fn log_response(req_id: ReqId, status: StatusCode, duration: Duration) {
    let message = format!("Completed with status {}", status);
    let duration = format!("{:?}", duration);
    match status {
        StatusCode::OK => tracing::info!(req_id = &req_id.0, message, duration),
        StatusCode::INTERNAL_SERVER_ERROR => {
            tracing::error!(req_id = &req_id.0, message, duration)
        }
        _ => tracing::warn!(req_id = &req_id.0, message, duration),
    };
}
