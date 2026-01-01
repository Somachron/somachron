use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

use thumbnail_output::{ImageData, ProcessedImage};

mod err;
mod media;

#[derive(Debug, ValueEnum, Clone, Copy, Serialize, Deserialize)]
#[clap(rename_all = "kebab_case")]
enum MediaType {
    Image,
    Video,
}

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[arg(short, long)]
    media: MediaType,

    #[arg(short, long)]
    rotation: Option<u64>,

    src: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    if !cli.src.exists() {
        eprintln!("Provided path doesn't exist: {:?}", cli.src);
        std::process::exit(1);
    }

    let result = match cli.media {
        MediaType::Image => media::handle_image(cli.src, cli.rotation),
        MediaType::Video => media::handle_video(cli.src.clone(), cli.src, cli.rotation).map(|th| ProcessedImage {
            thumbnail: th,
            preview: ImageData::default(),
        }),
    };

    match result {
        Ok(data) => {
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        Err(err) => err.exit(),
    };
}
