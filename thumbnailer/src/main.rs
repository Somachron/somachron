use std::{io::Write, path::PathBuf};

use clap::{Parser, ValueEnum};
use err::{AppResult, ErrType};
use sonic_rs::{Deserialize, JsonValueMutTrait, JsonValueTrait, Serialize};

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

    src: PathBuf,
    dst: PathBuf,
    file_path: PathBuf,
    r2_path: String,
    metadata_path: PathBuf,
    thumbnail_filename: String,
    user_id: String,
}

fn main() {
    let cli = Cli::parse();

    if !cli.src.exists() {
        eprintln!("Provided path doesn't exist: {:?}", cli.src);
        std::process::exit(1);
    }

    let metadata = match extract_metadata(&cli.src) {
        Ok(m) => m,
        Err(err) => {
            err.exit();
            return;
        }
    };

    let orientation = metadata.get("Orientation").and_then(|v| v.as_u64());
    let rotation = metadata.get("Rotation").and_then(|v| v.as_u64()).unwrap_or(0);

    match save_metadata(
        cli.user_id,
        cli.metadata_path,
        cli.file_path,
        cli.r2_path,
        cli.thumbnail_filename,
        cli.media,
        metadata,
    ) {
        Ok(()) => (),
        Err(err) => {
            err.exit();
            return;
        }
    };

    let result = match cli.media {
        MediaType::Image => media::handle_image(cli.src, cli.dst, orientation, Some(rotation)),
        MediaType::Video => media::handle_video(cli.src, cli.dst, Some(rotation)).map(|_| false),
    };

    match result {
        Ok(has_heic) => println!("{has_heic}"),
        Err(err) => err.exit(),
    }
}

fn extract_metadata(src: &PathBuf) -> AppResult<sonic_rs::Value> {
    let output = std::process::Command::new("exiftool")
        .args(&["-j", src.to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| ErrType::MediaError.err(err, "Failed to get exif data"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ErrType::MediaError.new(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data = stdout.into_owned();

    let result: sonic_rs::Value =
        sonic_rs::from_str(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize metadata"))?;

    let mut data = if result.is_array() {
        let arr = result.into_array().unwrap();
        arr.into_iter().nth(0).unwrap_or(sonic_rs::Value::default())
    } else {
        result
    };

    if let Some(value) = data.get_mut("SourceFile") {
        *value = sonic_rs::Value::from_static_str("");
    }
    if let Some(value) = data.get_mut("Directory") {
        *value = sonic_rs::Value::from_static_str("");
    }

    Ok(data)
}

#[derive(Serialize, Deserialize)]
struct FileMetadata {
    pub file_name: String,
    pub r2_path: String,
    pub thumbnail_path: String,
    pub metadata: sonic_rs::Value,
    pub size: usize,
    pub user_id: String,
    pub media_type: MediaType,
}

fn save_metadata(
    user_id: String,
    mut metadata_path: PathBuf,
    file_path: PathBuf,
    r2_path: String,
    thumbnail_filename: String,
    media_type: MediaType,
    metadata: sonic_rs::Value,
) -> AppResult<()> {
    let fs_meta = file_path.metadata().map_err(|err| ErrType::FsError.err(err, "Failed to fs metadata"))?;

    // get file extension
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;
    let file_name =
        file_path.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("Invalid file path without name"))?;

    // prepare path
    metadata_path.set_extension(format!("{ext}.json"));

    // serialize metadata to vec
    let metadata = FileMetadata {
        file_name: file_name.to_owned(),
        r2_path,
        thumbnail_path: {
            let mut path = PathBuf::from(file_path);
            path.set_file_name(thumbnail_filename);
            path.to_str().map(|s| s.to_owned()).unwrap()
        },
        metadata,
        size: fs_meta.len() as usize,
        user_id,
        media_type,
    };
    let metadata_bytes =
        sonic_rs::to_vec(&metadata).map_err(|err| ErrType::FsError.err(err, "Failed to serialize metadata"))?;

    // save metadata
    let mut metadata_file = std::fs::File::create(metadata_path)
        .map_err(|err| ErrType::FsError.err(err, "Failed to create metadata file"))?;
    metadata_file
        .write_all(&metadata_bytes)
        .map_err(|err| ErrType::FsError.err(err, "Failed to write metadata bytes"))?;
    let _ = metadata_file.flush();

    Ok(())
}
