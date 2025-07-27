use std::{io::Write, path::PathBuf};

use chrono::{DateTime, FixedOffset};
use clap::{Parser, ValueEnum};
use err::{AppResult, ErrType};
use sonic_rs::{Deserialize, JsonValueTrait, Serialize};

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

    let orientation = metadata.orientation.as_ref().and_then(|v| Some(v.parse().unwrap_or(0)));
    let rotation = metadata.rotation.unwrap_or(0);

    match save_metadata(
        cli.user_id,
        &cli.src,
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

fn extract_metadata(src: &PathBuf) -> AppResult<MediaMetadata> {
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

    let data = if result.is_array() {
        let arr = result.into_array().unwrap();
        arr.into_iter().nth(0).unwrap_or(sonic_rs::Value::default())
    } else {
        result
    };

    let gps_info = extract_gps_info(&data);

    let mut metadata: MediaMetadata =
        sonic_rs::from_value(&data).map_err(|err| ErrType::MediaError.err(err, "Failed to deserialize media data"))?;

    if let Some((lat, lng)) = gps_info {
        metadata.latitude = Some(lat);
        metadata.longitude = Some(lng);
    }

    Ok(metadata)
}

fn extract_gps_info(data: &sonic_rs::Value) -> Option<(f64, f64)> {
    let data_coordinates = data.get("GPSCoordinates").or_else(|| data.get("GPSPosition")).and_then(|v| v.as_str());

    let coordinates = data_coordinates.and_then(|v| {
        let mut tokens = v.split(',');
        let lat = tokens.next().map(|s| s.trim());
        let lng = tokens.next().map(|s| s.trim());
        lat.zip(lng)
    });

    let coordinates = coordinates.or_else(|| {
        let lat = data.get("GPSLatitude").and_then(|s| s.as_str());
        let lng = data.get("GPSLongitude").and_then(|s| s.as_str());
        lat.zip(lng)
    });

    coordinates.and_then(|(lat, lng)| Some((parse_dms_decimal(lat), parse_dms_decimal(lng))))
}

fn parse_dms_decimal(dms: &str) -> f64 {
    let tokens: Vec<&str> = dms.split(' ').filter(|s| !s.is_empty() && *s != "deg").collect();
    let degrees: f64 = tokens[0].trim_end_matches('°').parse().unwrap();
    let minutes: f64 = tokens[1].trim_end_matches('\'').parse().unwrap();
    let seconds: f64 = tokens[2].trim_end_matches('\"').parse().unwrap();

    let decimal = degrees + (minutes / 60.0) + (seconds / 3600.0);

    if dms.ends_with('S') || dms.ends_with('W') {
        -decimal
    } else {
        decimal
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct MediaMetadata {
    #[serde(rename = "Make")]
    make: Option<String>,
    #[serde(rename = "Model")]
    model: Option<String>,
    #[serde(rename = "Software")]
    software: Option<String>,

    #[serde(rename = "ImageHeight")]
    image_height: usize,
    #[serde(rename = "ImageWidth")]
    image_width: usize,

    #[serde(rename = "Duration")]
    duration: Option<String>,
    #[serde(rename = "MediaDuration")]
    media_duration: Option<String>,
    #[serde(rename = "VideoFrameRate")]
    frame_rate: Option<f32>,

    #[serde(rename = "DateTimeOriginal")]
    date_time: Option<DateTime<FixedOffset>>,
    #[serde(rename = "Orientation")]
    orientation: Option<String>,
    #[serde(rename = "Rotation")]
    rotation: Option<u64>,

    #[serde(rename = "ISO")]
    iso: Option<usize>,
    #[serde(rename = "ShutterSpeed")]
    shutter_speed: Option<String>,
    #[serde(rename = "Aperture")]
    aperture: Option<f32>,
    #[serde(rename = "FNumber")]
    f_number: Option<f32>,
    #[serde(rename = "ExposureTime")]
    exposure_time: Option<String>,

    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct FileMetadata {
    pub file_name: String,
    pub r2_path: String,
    pub thumbnail_path: String,
    pub metadata: MediaMetadata,
    pub size: usize,
    pub user_id: String,
    pub media_type: MediaType,
}

fn save_metadata(
    user_id: String,
    src: &PathBuf,
    metadata_path: PathBuf,
    file_path: PathBuf,
    r2_path: String,
    thumbnail_filename: String,
    media_type: MediaType,
    metadata: MediaMetadata,
) -> AppResult<()> {
    let fs_meta = src.metadata().map_err(|err| ErrType::FsError.err(err, "Failed to fs metadata"))?;

    let file_name =
        file_path.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("Invalid file path without name"))?;

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
    drop(metadata_bytes);
    let _ = metadata_file.flush();

    Ok(())
}
