use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use aws_sdk_s3::primitives::ByteStream;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
    pub r2_path: String,
    pub thumbnail_path: String,
    pub metadata: media::MediaMetadata,
    pub size: usize,
    pub media_type: MediaType,

    /// sha256 hash from r2_path
    pub file_hash: Hash,
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
    fn clean_path(&self, path: &str) -> AppResult<String> {
        let path = urlencoding::decode(path).map_err(|err| ErrType::FsError.err(err, "Invalid path"))?;
        Ok(path.trim_start_matches(FS_TAG).replace("..", "").trim_matches('/').to_owned())
    }

    pub async fn validate_user_drive(&self, user_id: &str) -> AppResult<()> {
        let user_dir = self.root_path.join(user_id);
        create_dir(&user_dir).await
    }

    /// sha256(R2 based `path`)
    pub fn get_folder_hash(&self, space_id: &str, path: &str) -> AppResult<Hash> {
        let path = self.clean_path(path)?;
        let mut path = self.r2_spaces.join(space_id).join(path);

        // cannot be check using `path.is_dir()` as path is r2_path
        // if we want folder_hash, but path has extension,
        if let Some(_) = path.extension() {
            // then remove file name
            path.set_file_name("");
        } // else path without extension could be dir

        Ok(Hash::new(path.to_str().unwrap()))
    }

    /// Creates space folder
    pub async fn create_space_folder(&self, space_id: &str) -> AppResult<()> {
        let r2_path = self.r2_spaces.join(space_id);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;
        self.r2.create_folder(r2_path).await?;

        let folder_path = self.spaces_path.join(space_id);
        create_dir(&folder_path).await
    }

    /// Creates folder in `space_id`
    ///
    /// * `folder_path`: some/existing/path/new_folder
    pub async fn create_folder(&self, space_id: &str, folder_path: &str) -> AppResult<()> {
        let folder_path = self.clean_path(folder_path)?;

        let r2_path = self.r2_spaces.join(space_id).join(&folder_path);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;
        self.r2.create_folder(r2_path).await?;

        let folder_path = self.spaces_path.join(space_id).join(folder_path);
        create_dir(&folder_path).await
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
        // let r2_path = self.r2_spaces.join(space_id).join(path);
        // let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;
        self.r2.generate_download_signed_url(&path).await
    }

    /// List items in the `dir` path
    ///
    /// * Skips `tmp`
    /// * Skips `.*` files
    /// * Processes only `*.json` files
    ///
    /// Returns vec [`String`]
    pub async fn list_dir(&self, space_id: &str, dir: &str) -> AppResult<Vec<String>> {
        let dir = self.clean_path(dir)?;
        let dir_path = self.spaces_path.join(space_id).join(&dir);

        let mut rd = tokio::fs::read_dir(&dir_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to read dir: {:?}", dir_path.file_name())))?;

        let mut entries = Vec::<String>::new();

        while let Some(dir) = rd.next_entry().await.map_err(|err| ErrType::FsError.err(err, "Failed to iter dir"))? {
            let file_name = dir.file_name();
            let file_name =
                file_name.to_str().ok_or(ErrType::FsError.new(format!("Failed to get file_name: {:?}", file_name)))?;

            match file_name {
                // skip tmp files
                x if x.starts_with("tmp") => continue,

                // skip hidden files
                x if x.starts_with('.') => continue,
                _ => (),
            };

            let ft = dir.file_type().await.map_err(|err| ErrType::FsError.err(err, "Failed to get file type"))?;

            if ft.is_dir() {
                entries.push(file_name.to_owned());
            }
        }

        entries.sort();
        Ok(entries)
    }

    /// Get requested file from filesystem with extension
    pub async fn get_file(&self, space_id: &str, file_path: &str) -> AppResult<(Vec<u8>, String)> {
        let file_path = self.clean_path(file_path)?;
        let fs_path = self.spaces_path.join(space_id).join(&file_path);
        let ext = fs_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        if !fs_path.exists() {
            return Err(ErrType::NotFound.new(format!("File not found: {file_path}")));
        }
        match ext {
            "jpeg" | "jpg" | "JPEG" | "JPG" => (),
            "png" | "PNG" => (),
            "json" => return Err(ErrType::BadRequest.new(format!("Invalid file requested: {file_path}"))),
            _ => return Err(ErrType::NotFound.new("Not found")),
        };

        let mut file = tokio::fs::File::open(&fs_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to open file: {file_path}")))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await.map_err(|err| ErrType::FsError.err(err, "Failed to read file"))?;

        Ok((buffer, ext.to_owned()))
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

        // prepare media directory
        let media_path = self.spaces_path.join(space_id).join(file_path);
        if let Some(parent) = media_path.parent() {
            create_dir(&parent.to_path_buf()).await?;
        }

        // get file extension
        let ext = media_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;

        let file_name = media_path.file_name().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("No file name"))?;

        // prepare r2 path
        let r2_path = self.r2_spaces.join(space_id).join(file_path);
        let mut r2_folder = r2_path.clone();
        r2_folder.set_file_name("");
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;
        let r2_folder = r2_folder.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        // prepare path
        let mut thumbnail_path = self.spaces_path.join(space_id).join(file_path);
        let file_stem = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let mut thumbnail_file_name = PathBuf::from(format!("{file_stem}_thumbnail.{ext}"));
        thumbnail_path.set_file_name(&thumbnail_file_name);

        // process thumbnail and metadata
        let media_type = media::get_media_type(ext);
        let bytes_stream = self.r2.download_media(r2_path).await?;
        let tmp_path = self.save_tmp_file(space_id, bytes_stream).await?;
        let tmp_file_path = tmp_path.clone();

        // update extension for video
        match media_type {
            infer::MatcherType::Video => {
                thumbnail_path.set_extension("jpeg");
                thumbnail_file_name.set_extension("jpeg");
            }
            _ => (),
        };

        // extract media metadata
        let metadata_result = self.process_media(space_id, file_path, ext, &tmp_path, thumbnail_path, media_type).await;
        let _ = remove_file(&tmp_file_path).await;
        let (metadata, paths) = metadata_result?;

        let thumbnail_file_name = thumbnail_file_name.to_str().map(|s| s.to_owned()).unwrap();
        let folder_hash = Hash::new(r2_folder);

        let all_metadata = paths
            .into_iter()
            .map(|(r2_path, processed_file_name, processed_thumbnail_file_name)| {
                let file_hash = Hash::new(&r2_path);
                FileData {
                    file_name: processed_file_name.unwrap_or(file_name.to_owned()),
                    r2_path: r2_path.to_owned(),
                    thumbnail_path: {
                        let mut path = PathBuf::from(file_path);
                        path.set_file_name(processed_thumbnail_file_name.unwrap_or(thumbnail_file_name.clone()));
                        path.to_str().map(|s| s.to_owned()).unwrap()
                    },
                    metadata: metadata.clone(),
                    size: file_size,
                    media_type: match media_type {
                        infer::MatcherType::Video => MediaType::Video,
                        _ => MediaType::Image,
                    },
                    file_hash,
                    folder_hash: folder_hash.clone(),
                }
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
        thumbnail_path: PathBuf,
        media_type: infer::MatcherType,
    ) -> AppResult<(media::MediaMetadata, Vec<(String, Option<String>, Option<String>)>)> {
        let metadata = media::extract_metadata(&tmp_path).await?;

        let path = PathBuf::from(file_path);
        let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap();
        let thumbnail_file_name = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let r2_path = self.r2_spaces.join(space_id).join(file_path);

        let mut media_data = Vec::new();

        // create thumbnail
        let heif_paths = media::run_thumbnailer(&tmp_path, &thumbnail_path, media_type, &metadata).await?;
        match heif_paths {
            Some(paths) => {
                for (i, tmp_path) in paths.into_iter().enumerate() {
                    let tmp_path = PathBuf::from(tmp_path);
                    let (file_name, thumbnail_file_name) = if i > 0 {
                        (format!("{file_name}_{i}.{ext}"), format!("{thumbnail_file_name}_{i}.jpeg"))
                    } else {
                        (format!("{file_name}.{ext}"), format!("{thumbnail_file_name}.jpeg"))
                    };

                    let mut r2_path = r2_path.clone();
                    r2_path.set_file_name(&file_name);
                    let r2_path = r2_path.to_str().unwrap();

                    self.r2.upload_photo(r2_path, &tmp_path).await?;
                    let _ = remove_file(&tmp_path).await;

                    media_data.push((r2_path.to_owned(), Some(file_name), Some(thumbnail_file_name)));
                }
            }
            None => {
                media_data.push((r2_path.to_str().unwrap().to_owned(), None, None));
            }
        };

        Ok((metadata, media_data))
    }

    /// Matches over spaces path
    ///
    /// Returns `is_dir`
    pub fn delete_path_type(&self, space_id: &str, path: &str) -> AppResult<bool> {
        let path = self.clean_path(path)?;
        let fs_path = self.spaces_path.join(space_id).join(path);
        Ok(fs_path.is_dir())
    }

    pub async fn delete_folder(&self, space_id: &str, dir_path: &str) -> AppResult<Vec<Hash>> {
        let path = self.clean_path(dir_path)?;

        let fs_path = self.spaces_path.join(space_id).join(path);

        let folders = self.collect_dirs(fs_path.clone()).await?;
        let mut folder_hashes = Vec::with_capacity(folders.len());

        for folder in folders.into_iter() {
            let abs_path = folder
                .strip_prefix(&self.spaces_path)
                .map_err(|err| ErrType::FsError.err(err, "Failed to strip prefix"))?;
            let mut r2_path = self.r2_spaces.join(abs_path);
            if let Some(_) = r2_path.extension() {
                r2_path.set_file_name("");
            }
            let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;

            self.r2.delete_folder(r2_path).await?;

            folder_hashes.push(Hash::new(r2_path));
        }

        tokio::fs::remove_dir_all(&fs_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to delete path: {:?}", fs_path)))?;

        Ok(folder_hashes)
    }

    pub async fn delete_file(&self, space_id: &str, path: &str) -> AppResult<(Hash, Hash)> {
        let path = self.clean_path(path)?;

        let fs_path = self.spaces_path.join(space_id).join(&path);
        let r2_path = self.r2_spaces.join(space_id).join(path);
        let mut r2_folder = r2_path.clone();
        r2_folder.set_file_name("");
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;
        let r2_folder = r2_folder.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;

        let file_stem =
            fs_path.file_stem().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("Failed to get file_stem"))?;

        let ext = fs_path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;
        let ext = match ext {
            "heic" | "HEIC" => "jpeg",
            _ => ext,
        };

        let mut thumbnail_path = fs_path.clone();
        thumbnail_path.set_file_name(format!("{file_stem}_thumbnail.{ext}"));

        self.r2.delete_key(r2_path).await?;
        let _ = remove_file(&thumbnail_path).await;

        let file_hash = Hash::new(r2_path);
        let folder_hash = Hash::new(r2_folder);
        Ok((file_hash, folder_hash))
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
