use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

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
    orientation: Option<u64>,
    #[arg(short, long)]
    rotation: Option<u64>,

    src: PathBuf,
    dst: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    if !cli.src.exists() {
        eprintln!("Provided path doesn't exist: {:?}", cli.src);
        std::process::exit(1);
    }

    let result = match cli.media {
        MediaType::Image => media::handle_image(cli.src, cli.dst, cli.orientation, cli.rotation),
        MediaType::Video => media::handle_video(cli.src, cli.dst, cli.rotation).map(|_| false),
    };

    match result {
        Ok(has_heic) => println!("{has_heic}"),
        Err(err) => err.exit(),
    }
}
