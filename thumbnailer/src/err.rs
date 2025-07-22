use std::{error::Error, fmt::Display};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum ErrType {
    FsError,
    MediaError,
}
impl ErrType {
    pub fn new(self, message: impl Into<String>) -> AppError {
        AppError::init(self, None, message)
    }

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

#[derive(Debug, Serialize)]
pub struct AppError {
    _type: ErrType,
    message: String,
    at: String,
    err_msg: String,
}

impl AppError {
    fn init(_type: ErrType, err: Option<Box<dyn Error>>, message: impl Into<String>) -> Self {
        let at = AppError::caller();
        AppError {
            _type,
            message: message.into(),
            at,
            err_msg: err.map(|e| e.to_string()).unwrap_or("".into()),
        }
    }

    fn caller() -> String {
        let mut file_addr = String::from("");

        let bt = backtrace::Backtrace::new_unresolved();
        let frame = match bt.frames().get(7) {
            Some(frame) => frame,
            _ => return "".into(),
        };
        backtrace::resolve(frame.ip(), |symbol| {
            let file_path = match symbol.filename() {
                Some(path) => path,
                _ => return,
            };

            let file_name = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown");
            let lineno = symbol.lineno().unwrap_or(0);
            let colno = symbol.colno().unwrap_or(0);
            file_addr = format!("{}:{}:{}", file_name, lineno, colno);
        });
        file_addr
    }

    pub fn exit(self) {
        eprintln!("{} // [{}] - {}", self.message, self.at, self.err_msg);

        std::process::exit(1);
    }
}

pub type AppResult<T> = Result<T, AppError>;
