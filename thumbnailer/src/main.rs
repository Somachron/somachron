use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

mod err;
mod media;

#[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
#[clap(rename_all = "kebab_case")]
enum MediaType {
    Image {
        path: PathBuf,
    },
    Video {
        url: String,
        tmp_path: PathBuf,
    },
}

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[clap(subcommand)]
    media: MediaType,

    #[arg(short, long)]
    rotation: Option<u64>,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.media {
        MediaType::Image {
            path,
        } => {
            if !path.exists() {
                eprintln!("Provided path doesn't exist: {:?}", path);
                std::process::exit(1);
            }
            media::handle_image(path, cli.rotation)
        }
        MediaType::Video {
            url,
            tmp_path,
        } => media::handle_video(url, tmp_path, cli.rotation),
    };

    match result {
        Ok(data) => {
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        Err(err) => err.exit(),
    };
}
