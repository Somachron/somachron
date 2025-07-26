use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use aws_sdk_s3::primitives::ByteStream;
use nanoid::nanoid;
use sonic_rs::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use utoipa::ToSchema;

use super::{config, media, r2::R2Storage, AppResult, ErrType};

const ROOT_DATA: &str = "somachron-data";
const SPACES_PATH: &str = "spaces";
const FS_TAG: &str = "fs::";

#[derive(Serialize, Deserialize, ToSchema)]
pub enum MediaType {
    Image,
    Video,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct FileMetadata {
    pub file_name: String,
    pub r2_path: String,
    pub thumbnail_path: String,
    pub metadata: sonic_rs::Value,
    pub size: usize,
    pub user_id: String,
    pub media_type: MediaType,
}

#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum FileEntry {
    Dir {
        tag: String,
        name: String,
    },
    File {
        tag: String,
        data: FileMetadata,
    },
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

    pub fn spaces_path(&self) -> PathBuf {
        self.spaces_path.clone()
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
    pub async fn generate_download_signed_url(&self, space_id: &str, path: &str) -> AppResult<String> {
        let path = self.clean_path(path)?;
        let r2_path = self.r2_spaces.join(space_id).join(path);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;
        self.r2.generate_download_signed_url(r2_path).await
    }

    /// List items in the `dir` path
    ///
    /// * Skips `tmp`
    /// * Skips `.*` files
    /// * Processes only `*.json` files
    ///
    /// Returns vec [`FileEntry`]
    pub async fn list_dir(&self, space_id: &str, dir: &str) -> AppResult<Vec<FileEntry>> {
        let dir = self.clean_path(dir)?;
        let dir_path = self.spaces_path.join(space_id).join(dir);

        let mut rd = tokio::fs::read_dir(&dir_path)
            .await
            .map_err(|err| ErrType::FsError.err(err, format!("Failed to read dir: {:?}", dir_path)))?;

        let mut entries = Vec::<FileEntry>::new();

        while let Some(dir) = rd.next_entry().await.map_err(|err| ErrType::FsError.err(err, "Failed to iter dir"))? {
            let file_name = dir.file_name();
            let file_name =
                file_name.to_str().ok_or(ErrType::FsError.new(format!("Failed to get file_name: {:?}", dir_path)))?;

            match file_name {
                // skip tmp files
                x if x.starts_with("tmp") => continue,

                // skip hidden files
                x if x.starts_with('.') => continue,
                _ => (),
            };

            let ft = dir.file_type().await.map_err(|err| ErrType::FsError.err(err, "Failed to get file type"))?;

            if ft.is_dir() {
                entries.push(FileEntry::Dir {
                    tag: "dir".to_owned(),
                    name: file_name.to_owned(),
                });
                continue;
            }

            if ft.is_file() {
                let path = dir.path();
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

                if ext.ends_with("json") {
                    let data: FileMetadata = {
                        let mut file = tokio::fs::File::open(&path)
                            .await
                            .map_err(|err| ErrType::FsError.err(err, format!("Failed to open file: {:?}", path)))?;
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer)
                            .await
                            .map_err(|err| ErrType::FsError.err(err, format!("Failed to read file: {:?}", path)))?;

                        sonic_rs::from_slice(&buffer)
                            .map_err(|err| ErrType::FsError.err(err, "Failed to deserialize metadata"))?
                    };

                    entries.push(FileEntry::File {
                        tag: "file".to_owned(),
                        data,
                    });
                }

                continue;
            }
        }

        entries.sort_by(|a: &FileEntry, b: &FileEntry| match (a, b) {
            (
                FileEntry::Dir {
                    name,
                    ..
                },
                FileEntry::Dir {
                    name: b_name,
                    ..
                },
            ) => name.cmp(b_name),
            (
                FileEntry::Dir {
                    ..
                },
                FileEntry::File {
                    ..
                },
            ) => std::cmp::Ordering::Less,
            (
                FileEntry::File {
                    ..
                },
                FileEntry::Dir {
                    ..
                },
            ) => std::cmp::Ordering::Greater,
            (
                FileEntry::File {
                    data,
                    ..
                },
                FileEntry::File {
                    data: b_data,
                    ..
                },
            ) => data.file_name.cmp(&b_data.file_name),
        });

        Ok(entries)
    }

    /// Get requested file from filesystem
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
    /// * extract and save exif data
    pub async fn process_upload_skeleton_thumbnail(
        &self,
        user_id: &str,
        space_id: &str,
        file_path: &str,
        file_size: usize,
    ) -> AppResult<()> {
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
        let file_name = media_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(ErrType::FsError.new("Invalid file path without name"))?;

        // prepare r2 path
        let r2_path = self.r2_spaces.join(space_id).join(file_path);
        let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from file path"))?;

        // prepare path
        let mut thumbnail_path = self.spaces_path.join(space_id).join(file_path);
        let file_stem = thumbnail_path.file_stem().and_then(|s| s.to_str()).unwrap();
        let mut thumbnail_file_name = PathBuf::from(format!("{file_stem}_thumbnail.{ext}"));
        thumbnail_path.set_file_name(&thumbnail_file_name);

        // process thumbnail and metadata
        let media_type = media::get_media_type(ext);

        let bytes_stream = self.r2.download_media(r2_path).await?;
        let tmp_path = self.save_tmp_file(space_id, bytes_stream).await?;
        let metadata = media::extract_metadata(&tmp_path).await?;

        match media_type {
            infer::MatcherType::Video => {
                thumbnail_path.set_extension("jpeg");
                thumbnail_file_name.set_extension("jpeg");
            }
            _ => (),
        };

        // create thumbnail
        match media::run_thumbnailer(&tmp_path, &thumbnail_path, media_type, &metadata).await {
            Ok(was_heic) => {
                if was_heic {
                    thumbnail_file_name.set_extension("jpeg");
                    self.r2.upload_photo(r2_path, &tmp_path).await?;
                }
                let _ = remove_file(&tmp_path);
            }
            Err(err) => {
                let _ = remove_file(&tmp_path);
                return Err(err);
            }
        };

        // save metadata
        {
            // prepare path
            let mut metadata_path = self.spaces_path.join(space_id).join(file_path);
            metadata_path.set_extension(format!("{ext}.json"));

            // serialize metadata to vec
            let metadata = FileMetadata {
                file_name: file_name.to_owned(),
                r2_path: r2_path.to_string(),
                thumbnail_path: {
                    let mut path = PathBuf::from(file_path);
                    path.set_file_name(thumbnail_file_name);
                    path.to_str().map(|s| s.to_owned()).unwrap()
                },
                metadata,
                size: file_size,
                user_id: user_id.to_string(),
                media_type: match media_type {
                    infer::MatcherType::Video => MediaType::Video,
                    _ => MediaType::Image,
                },
            };
            let metadata_bytes =
                sonic_rs::to_vec(&metadata).map_err(|err| ErrType::FsError.err(err, "Failed to serialize metadata"))?;

            // save metadata
            let mut metadata_file = create_file(&metadata_path).await?;
            metadata_file
                .write(&metadata_bytes)
                .await
                .map_err(|err| ErrType::FsError.err(err, "Failed to write metadata bytes"))?;
            let _ = metadata_file.flush();
        }

        Ok(())
    }

    pub async fn delete_path(&self, space_id: &str, path: &str) -> AppResult<()> {
        let path = self.clean_path(path)?;

        let fs_path = self.spaces_path.join(space_id).join(&path);
        if fs_path.is_dir() {
            let folders = self.collect_dirs(fs_path.clone()).await?;
            for folder in folders.into_iter() {
                let r2_path = folder
                    .strip_prefix(&self.spaces_path)
                    .map_err(|err| ErrType::FsError.err(err, "Failed to strip prefix"))?;
                let r2_path = self.r2_spaces.join(r2_path);
                let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;

                self.r2.delete_folder(r2_path).await?;
            }

            tokio::fs::remove_dir_all(&fs_path)
                .await
                .map_err(|err| ErrType::FsError.err(err, format!("Failed to delete path: {:?}", fs_path)))
        } else {
            let r2_path = self.r2_spaces.join(space_id).join(path);
            let r2_path = r2_path.to_str().ok_or(ErrType::FsError.new("Failed to get str from folder path"))?;

            let file_stem =
                fs_path.file_stem().and_then(|s| s.to_str()).ok_or(ErrType::FsError.new("Failed to get file_stem"))?;
            let ext = fs_path
                .extension()
                .and_then(|s| s.to_str())
                .ok_or(ErrType::FsError.new("Invalid file path without extenstion"))?;

            let mut thumbnail_path = fs_path.clone();
            thumbnail_path.set_file_name(format!("{file_stem}_thumbnail.{ext}"));

            let mut json_path = fs_path.clone();
            json_path.set_extension(format!("{ext}.json"));

            self.r2.delete_key(r2_path).await?;
            let _ = remove_file(&thumbnail_path);
            let _ = remove_file(&json_path);
            Ok(())
        }
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
