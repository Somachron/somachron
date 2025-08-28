use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum ErrType {
    FsError,
    MediaError,
}
impl ErrType {
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
                ErrType::FsError => "FileSystemError",
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

    pub fn exit(self) {
        eprintln!("{} // [{}] - {}", self.message, self.at, self.err_msg);

        std::process::exit(1);
    }
}

pub type AppResult<T> = Result<T, AppError>;
