use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessedImage {
    pub thumbnail: ImageData,
    pub preview: ImageData,
}
