use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProcessedImage {
    General {
        thumbnail: ImageData,
        preview: ImageData,
    },
    Heif {
        thumbnail: Vec<ImageData>,
        preview: Vec<ImageData>,
        heif_paths: Vec<PathBuf>,
    },
}
