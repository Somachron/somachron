use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use aws_sdk_s3::primitives::ByteStream;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

use super::{config, media, r2::R2Storage, AppResult, ErrType};

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";
const FS_TAG: &str = "fs::";

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Image,
    Video,
}

/// Wrapper to enforce hash type
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Hash(String);
impl Hash {
    pub fn new(path: &str) -> Self {
        Self(sha256::digest(path.trim_matches('/')))
    }

    pub fn get(self) -> String {
        self.0
    }

    pub fn get_ref(&self) -> &str {
        &self.0
    }
}

pub struct FileData {
    pub file_name: String,
    pub path: String,
    pub thumbnail_file_name: String,
    pub metadata: media::MediaMetadata,
    pub size: usize,
    pub media_type: MediaType,

    /// sha256 hash from r2_path without file_name
    pub folder_hash: Hash,
}

/// Manage storage operations
///
/// Mimic the file structure from [`R2Storage`] in attached volume
pub struct Storage {
    /// /mounted/volume/[`ROOT_DATA`]
    root_path: PathBuf,

    /// /mounted/volume/[`ROOT_DATA`]/[`SPACES_PATH`]
    spaces_path: PathBuf,

    /// Root folder for R2: [`ROOT_DATA`]/[`SPACES_PATH`],
    r2_spaces: PathBuf,

    /// R2 client
    r2: R2Storage,
}

async fn create_dir(dir: &PathBuf) -> AppResult<()> {
    tokio::fs::create_dir_all(&dir).await.map_err(|err| ErrType::FsError.err(err, "Failed to create dir"))
}

async fn create_file(file_path: &PathBuf) -> AppResult<tokio::fs::File> {
    tokio::fs::File::create(&file_path).await.map_err(|err| ErrType::FsError.err(err, "Failed to create/truncate file"))
}

async fn remove_file(file_path: &PathBuf) -> AppResult<()> {
    tokio::fs::remove_file(file_path).await.map_err(|err| ErrType::FsError.err(err, "Failed to remove file"))
}

impl Storage {
    pub async fn new() -> Self {
        let volume_path = config::get_volume_path();
        let volume_path = Path::new(&volume_path);

        // create necessary volumes
        let root_path = volume_path.join(ROOT_DATA);
        create_dir(&root_path).await.unwrap();

        let spaces_path = root_path.join(SPACES_PATH);
        create_dir(&root_path).await.unwrap();

        Self {
            root_path,
            spaces_path,
            r2_spaces: PathBuf::from(ROOT_DATA).join(SPACES_PATH),
            r2: R2Storage::new(),
        }
    }

    // ---------------------- MIGRATION

    pub async fn migration_lock(&self) -> AppResult<()> {
        let path = self.root_path.join("mg.lock");
        {
            let mut file = create_file(&path).await?;
            file.write_all(&[65]).await.unwrap();
            let _ = file.flush().await;
        }
        Ok(())
    }

    pub async fn migration_exists(&self) -> bool {
        let path = self.root_path.join("mg.lock");
        tokio::fs::try_exists(path).await.unwrap_or(true)
    }

    pub async fn upload_thumbnail(&self, space_id: &str, file_name: &str, thumbnail_path: &str) -> AppResult<()> {
        let mut folders = PathBuf::from(thumbnail_path);
        let thumbnail_ext = folders.extension().and_then(|s| s.to_str()).expect("No ext").to_owned();
        folders.set_file_name("");

        let file_path = PathBuf::from(file_name);
        let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap().to_owned();

        let path_key =
            self.r2_spaces.join(space_id).join(folders).join(format!("thumbnail_{file_stem}.{thumbnail_ext}"));
        let source_path = self.spaces_path.join(space_id).join(thumbnail_path);
        self.r2.upload_photo(path_key.to_str().unwrap(), &source_path).await
    }

    // ---------------------- MIGRATION

    async fn save_tmp_file(&self, space_id: &str, mut bytes_stream: ByteStream) -> AppResult<PathBuf> {
        let tmp_dir_path = self.root_path.join(space_id).join("tmp");
        create_dir(&tmp_dir_path).await?;

        let id = nanoid!(8);
        let tmp_file_path = tmp_dir_path.join(format!("tmp_f_{id}"));
        {
            let tmp_file = create_file(&tmp_file_path).await?;
            let mut buf_writer = tokio::io::BufWriter::new(tmp_file);

            while let Some(chunk) = bytes_stream.next().await {
                let bytes = chunk.map_err(|err| ErrType::R2Error.err(err, "Failed to read next chunk stream"))?;

                buf_writer
                    .write_all(&bytes)
                    .await
                    .map_err(|err| ErrType::FsError.err(err, "Failed to write tmp media file"))?;
            }
            let _ = buf_writer.flush().await;
        }

        Ok(tmp_file_path)
    }

    /// Cleans path for fs operations
    ///
    /// * Remove `/` from start and end
    /// * Remove [`FS_TAG`] from start
    /// * Replace `..` with empty from start and end
    pub fn clean_path(&self, path: &str) -> AppResult<String> {
        let path = urlencoding::decode(path).map_err(|err| ErrType::FsError.err(err, "Invalid path"))?;
        Ok(path.trim_start_matches(FS_TAG).replace("..", "").trim_matches('/').to_owned())
    }

    /// Get path prefix
    pub fn get_spaces_path(&self, space_id: &str) -> String {
        let path = self.r2_spaces.join(space_id);
        path.to_str().map(|s| s.to_owned()).unwrap_or_default()
    }

    /// Creates space folder
    pub async fn create_space_folder(&self, space_id: &str) -> AppResult<()> {
        let r2_path = self.r2_spaces.join(space_id);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;
        self.r2.create_folder(r2_path).await
    }

    /// Generate presigned URL for uploading media
    ///
    /// To be used by frontend
    pub async fn generate_upload_signed_url(&self, space_id: &str, file_path: &str) -> AppResult<String> {
        let file_path = self.clean_path(file_path)?;

        let file_path = self.r2_spaces.join(space_id).join(file_path);
        let file_path = file_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        self.r2.generate_upload_signed_url(file_path).await
    }

    /// Generate presigned URL for steaming media
    ///
    /// To be used by frontend
    pub async fn generate_download_signed_url(&self, path: &str) -> AppResult<String> {
        let path = self.clean_path(path)?;
        self.r2.generate_download_signed_url(&path).await
    }

    /// List items in the `dir` path
    ///
    /// * Skips `tmp`
    /// * Skips `.*` files
    /// * Processes only `*.json` files
    ///
    /// Returns vec [`String`]
    #[deprecated]
    pub async fn list_dir(&self, space_id: &str) -> AppResult<(PathBuf, Vec<PathBuf>)> {
        let dir_path = self.spaces_path.join(space_id);
        let dirs = self.collect_dirs(dir_path.clone()).await?;
        Ok((
            self.r2_spaces.join(space_id),
            dirs.into_iter()
                .map(|p| {
                    let path = p.strip_prefix(&dir_path).unwrap();
                    path.to_path_buf()
                })
                .collect(),
        ))
    }

    /// Process the uploaded media
    ///
    /// * prepares the directory in mounted volume
    /// * download the media from R2
    /// * create and save thumbnail
    /// * extract metadata
    pub async fn process_upload_completion(
        &self,
        space_id: &str,
        file_path: &str,
        file_size: usize,
    ) -> AppResult<Vec<FileData>> {
        let file_path = self.clean_path(&file_path)?;
        let file_path = file_path.as_str();

        // prepare r2 path
        let r2_path = self.r2_spaces.join(space_id).join(file_path);
        let mut r2_thumbnail = r2_path.clone();
        let mut r2_folder = r2_path.clone();
        r2_folder.set_file_name("");

        // get file extension
        let ext = r2_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;

        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;
        let r2_folder = r2_folder.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        // prepare path
        let file_name =
            r2_thumbnail.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("No file name"))?.to_owned();
        r2_thumbnail.set_file_name(format!("thumbnail_{file_name}"));

        // process thumbnail and metadata
        let media_type = media::get_media_type(ext);
        let bytes_stream = self.r2.download_media(r2_path).await?;
        let tmp_path = self.save_tmp_file(space_id, bytes_stream).await?;

        match media_type {
            infer::MatcherType::Video => {
                r2_thumbnail.set_extension("jpeg");
            }
            _ => (),
        };

        // extract media metadata
        let metadata_result = self.process_media(space_id, file_path, ext, &tmp_path, &r2_thumbnail, media_type).await;
        let _ = remove_file(&tmp_path).await;
        let (metadata, paths) = metadata_result?;

        let thumbnail_file_name = r2_thumbnail.file_name().and_then(|s| s.to_str()).unwrap().to_owned();
        let folder_hash = Hash::new(r2_folder);

        let all_metadata = paths
            .into_iter()
            .map(|(processed_file_name, processed_thumbnail_file_name)| FileData {
                file_name: processed_file_name.unwrap_or(file_name.to_owned()),
                path: r2_folder.to_owned(),
                thumbnail_file_name: processed_thumbnail_file_name.unwrap_or(thumbnail_file_name.clone()),
                metadata: metadata.clone(),
                size: file_size,
                media_type: match media_type {
                    infer::MatcherType::Video => MediaType::Video,
                    _ => MediaType::Image,
                },
                folder_hash: folder_hash.clone(),
            })
            .collect();

        Ok(all_metadata)
    }

    async fn process_media(
        &self,
        space_id: &str,
        file_path: &str,
        ext: &str,
        tmp_path: &PathBuf,
        r2_thumbnail: &PathBuf,
        media_type: infer::MatcherType,
    ) -> AppResult<(media::MediaMetadata, Vec<(Option<String>, Option<String>)>)> {
        let metadata = media::extract_metadata(&tmp_path).await?;

        let path = PathBuf::from(file_path);
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap();
        let thumbnail_file_name = r2_thumbnail.file_stem().and_then(|s| s.to_str()).unwrap();
        let r2_path = self.r2_spaces.join(space_id).join(file_path);

        let mut media_data = Vec::new();

        // create thumbnail
        let heif_paths = media::run_thumbnailer(&tmp_path, media_type, &metadata).await?;
        match heif_paths {
            Some(paths) => {
                for (i, tmp_path) in paths.into_iter().enumerate() {
                    let tmp_path = PathBuf::from(tmp_path);
                    let mut tmp_thumbnail_path = tmp_path.clone();
                    let tmp_thumbnail_file =
                        tmp_thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap().to_owned();
                    tmp_thumbnail_path.set_file_name(format!("{tmp_thumbnail_file}_{i}.jpeg"));

                    let (file_name, thumbnail_file_name) = if i > 0 {
                        (format!("{file_name}_{i}.{ext}"), format!("{thumbnail_file_name}_{i}.jpeg"))
                    } else {
                        (format!("{file_name}.{ext}"), format!("{thumbnail_file_name}.jpeg"))
                    };

                    let mut r2_path = r2_path.clone();
                    r2_path.set_file_name(&file_name);
                    let r2_path = r2_path.to_str().unwrap();

                    let mut r2_thumbnail = r2_thumbnail.clone();
                    r2_thumbnail.set_file_name(&thumbnail_file_name);
                    let r2_thumbnail = r2_thumbnail.to_str().unwrap();

                    self.r2.upload_photo(r2_path, &tmp_path).await?;
                    self.r2.upload_photo(r2_thumbnail, &tmp_thumbnail_path).await?;
                    let _ = remove_file(&tmp_path).await;
                    let _ = remove_file(&tmp_thumbnail_path).await;

                    media_data.push((Some(file_name), Some(thumbnail_file_name)));
                }
            }
            None => {
                let r2_thumbnail = r2_thumbnail.to_str().unwrap();
                self.r2.upload_photo(r2_thumbnail, &tmp_path).await?;
                media_data.push((None, None));
            }
        };

        Ok((metadata, media_data))
    }

    /// Matches over spaces path
    ///
    /// Returns `is_dir` and cleaned `path`
    pub fn delete_path_type(&self, space_id: &str, path: &str) -> AppResult<(String, bool)> {
        let path = self.clean_path(path)?;
        let fs_path = self.r2_spaces.join(space_id).join(&path);
        Ok((path, fs_path.extension().is_none()))
    }

    pub async fn delete_folder(&self, space_id: &str, dir_path: &str) -> AppResult<()> {
        let path = self.clean_path(dir_path)?;

        let mut r2_path = self.r2_spaces.join(space_id).join(path);
        if let Some(_) = r2_path.extension() {
            r2_path.set_file_name("");
        }
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;

        self.r2.delete_folder(r2_path).await
    }

    pub async fn delete_file(&self, r2_file: String, r2_thumbnail: String) -> AppResult<()> {
        self.r2.delete_key(&r2_file).await?;
        self.r2.delete_key(&r2_thumbnail).await?;
        Ok(())
    }

    async fn collect_dirs(&self, dir_path: PathBuf) -> AppResult<Vec<PathBuf>> {
        struct DirDepth {
            path: PathBuf,
            depth: u32,
        }
        let mut folders = Vec::new();
        folders.push(DirDepth {
            path: dir_path.clone(),
            depth: 0,
        });

        let mut queue = VecDeque::new();
        queue.push_back((dir_path, 0));

        while let Some((dir, depth)) = queue.pop_front() {
            let mut rd = tokio::fs::read_dir(&dir)
                .await
                .map_err(|err| ErrType::FsError.err(err, format!("Failed to read dir: {:?}", dir)))?;

            while let Some(entry) =
                rd.next_entry().await.map_err(|err| ErrType::FsError.err(err, "Failed to iter dir"))?
            {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(DirDepth {
                        path: path.clone(),
                        depth: depth + 1,
                    });
                    queue.push_back((path, depth + 1));
                }
            }
        }

        folders.sort_by(|a: &DirDepth, b: &DirDepth| b.depth.cmp(&a.depth).then_with(|| a.path.cmp(&b.path)));

        Ok(folders.into_iter().map(|f| f.path).collect())
    }
}
