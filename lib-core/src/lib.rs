use std::{error::Error, fmt::Display};

use aws_sdk_s3::{
    config::http::HttpResponse,
    error::SdkError,
    operation::{
        delete_object::DeleteObjectError, get_object::GetObjectError, list_objects_v2::ListObjectsV2Error,
        put_object::PutObjectError,
    },
};
use axum::{
    extract::{rejection::JsonRejection, FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;
use validator::Validate;

pub mod clerk;
pub mod config;
pub mod interceptor;
pub mod media;
mod s3;
pub mod storage;

#[repr(transparent)]
pub struct ReqId(pub String);
impl Clone for ReqId {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Serialize, ToSchema)]
pub struct EmptyResponse {
    status: u16,
    message: String,
}
impl EmptyResponse {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        EmptyResponse {
            status: status.as_u16(),
            message: message.into(),
        }
    }
}

/// Custom Json wrapper handling json payload
/// parsing errors.
///
/// See more: [`axum::Json`] [`validator`]
pub struct Json<T>(pub T);

impl<T> IntoResponse for Json<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        (StatusCode::OK, axum::Json(self.0)).into_response()
    }
}

/// Custom Json wrapper handling json payload
///
/// Struct being extract must have [`serde::Deserialize`] and [`validator::Validate`] to validate the payload
impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    T: Validate,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        let req_id: ReqId = {
            let req = &req;
            let id: &ReqId = req.extensions().get().unwrap();
            id.clone()
        };

        let axum::Json(payload) = axum::Json::<T>::from_request(req, state).await.map_err(|e| {
            let err_msg = e.body_text();
            ApiError(ErrType::InvalidBody.err(e, err_msg), req_id.clone())
        })?;

        payload.validate().map_err(|e| {
            let err_msg = format!("Bad Payload: {e}");
            ApiError(ErrType::BadRequest.err(e, err_msg), req_id.clone())
        })?;

        Ok(Json(payload))
    }
}

#[derive(Debug)]
pub enum ErrType {
    Unauthorized,
    BadRequest,
    NotFound,
    ServerError,
    InvalidBody,
    TooManyRequests,

    DbError,
    FsError,
    R2Error,
    MediaError,
}
impl ErrType {
    #[track_caller]
    pub fn r2_put(err: SdkError<PutObjectError, HttpResponse>, message: impl Into<String>) -> AppError {
        AppError::init(
            ErrType::R2Error,
            match err.into_service_error() {
                PutObjectError::EncryptionTypeMismatch(encryption_type_mismatch) => {
                    Some(encryption_type_mismatch.into())
                }
                PutObjectError::InvalidRequest(invalid_request) => Some(invalid_request.into()),
                PutObjectError::InvalidWriteOffset(invalid_write_offset) => Some(invalid_write_offset.into()),
                PutObjectError::TooManyParts(too_many_parts) => Some(too_many_parts.into()),
                err => Some(err.into()),
            },
            message,
        )
    }

    #[track_caller]
    pub fn r2_get(err: SdkError<GetObjectError, HttpResponse>, message: impl Into<String>) -> AppError {
        AppError::init(
            ErrType::R2Error,
            match err.into_service_error() {
                GetObjectError::InvalidObjectState(invalid_object_state) => Some(invalid_object_state.into()),
                GetObjectError::NoSuchKey(no_such_key) => Some(no_such_key.into()),
                err => Some(err.into()),
            },
            message,
        )
    }

    #[track_caller]
    pub fn r2_delete(err: SdkError<DeleteObjectError, HttpResponse>, message: impl Into<String>) -> AppError {
        let err = err.into_service_error();
        AppError::init(ErrType::R2Error, Some(err.into()), message)
    }

    #[track_caller]
    pub fn r2_list_err(err: SdkError<ListObjectsV2Error, HttpResponse>, message: impl Into<String>) -> AppError {
        AppError::init(
            ErrType::R2Error,
            match err.into_service_error() {
                ListObjectsV2Error::NoSuchBucket(no_such_bucket) => Some(no_such_bucket.into()),
                err => Some(err.into()),
            },
            message,
        )
    }

    #[track_caller]
    pub fn msg(self, message: impl Into<String>) -> AppError {
        AppError::init(self, None, message)
    }

    #[track_caller]
    pub fn err(self, err: impl Into<Box<dyn Error>>, message: impl Into<String>) -> AppError {
        AppError::init(self, Some(err.into()), message)
    }
}
impl Display for ErrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrType::Unauthorized => "Unauthorized",
                ErrType::BadRequest => "BadRequest",
                ErrType::NotFound => "NotFound",
                ErrType::ServerError => "ServerError",
                ErrType::InvalidBody => "InvalidBody",
                ErrType::TooManyRequests => "TooManyRequests",

                ErrType::DbError => "DbError",
                ErrType::FsError => "FileSystemError",
                ErrType::R2Error => "R2Error",
                ErrType::MediaError => "MediaError",
            }
        )
    }
}

#[derive(Debug)]
pub struct AppError {
    _type: ErrType,
    message: String,
    at: String,
    err_msg: String,
}

impl AppError {
    #[track_caller]
    fn init(_type: ErrType, err: Option<Box<dyn Error>>, message: impl Into<String>) -> Self {
        let location = std::panic::Location::caller();
        let at = format!("{}:{}:{}", location.file(), location.line(), location.column());
        AppError {
            _type,
            message: message.into(),
            at,
            err_msg: err.map(|e| e.to_string()).unwrap_or("".into()),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;
pub struct ApiError(pub AppError, pub ReqId);
pub type ApiResult<T> = axum::response::Result<Json<T>, ApiError>;

impl IntoResponse for ApiError {
    /// Function to map errors into appropriate responses
    fn into_response(self) -> Response {
        let err = self.0;
        let req_id = self.1;

        let id: &str = &req_id.0;
        let _type = err._type;
        let err_msg = err.err_msg;
        let message = format!("[{}]: {}", _type, err.message);
        let at = err.at;

        let status = match _type {
            ErrType::InvalidBody => StatusCode::BAD_REQUEST,
            ErrType::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrType::BadRequest => StatusCode::BAD_REQUEST,
            ErrType::NotFound => StatusCode::NOT_FOUND,
            ErrType::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrType::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,

            ErrType::DbError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrType::FsError => StatusCode::FAILED_DEPENDENCY,
            ErrType::R2Error => StatusCode::FAILED_DEPENDENCY,
            ErrType::MediaError => StatusCode::UNPROCESSABLE_ENTITY,
        };

        match status {
            StatusCode::INTERNAL_SERVER_ERROR | StatusCode::FAILED_DEPENDENCY => {
                tracing::error!(req_id = id, message = message, at = at, err = err_msg)
            }
            _ => tracing::warn!(req_id = id, message = message, at = at, err = err_msg),
        };

        (
            status,
            Json(EmptyResponse {
                status: status.as_u16(),
                message,
            }),
        )
            .into_response()
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self(ErrType::InvalidBody.err(rejection, "Invalid payload"), ReqId("".into()))
    }
}
