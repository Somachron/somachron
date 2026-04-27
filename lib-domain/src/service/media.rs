use std::path::Path;

use chrono::DateTime;
use lib_core::{
    interconnect::ServiceInterconnect,
    smq_dto::{
        self,
        req::ProcessMediaRequest,
        res::{FileData, ImageData, MediaData},
        MediaDatetime, MediaMetadata,
    },
    storage::Storage,
    AppResult, ErrType,
};
use reqwest::Response;
use uuid::Uuid;

use crate::{
    datastore::{storage::StorageDs, user_space::SpaceRole},
    dto::cloud::{
        req::{InitiateUploadRequest, QueueMediaProcessRequest},
        res::{
            _AlbumResponse, _AlbumResponseVec, _FileMetaResponseVec, DownloadUrlResponse, InitiateUploadResponse,
            StreamedUrlResponse,
        },
    },
    extension::{SpaceCtx, UserId},
};

use super::ServiceWrapper;

pub trait MediaService: Send + Sync {
    fn create_album(
        &self,
        user_id: UserId,
        space_ctx: SpaceCtx,
        album_name: String,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn initiate_upload(
        &self,
        space_ctx: SpaceCtx,
        storage: &Storage,
        dto: InitiateUploadRequest,
    ) -> impl Future<Output = AppResult<InitiateUploadResponse>> + Send;

    fn queue_media_process(
        &self,
        user_id: UserId,
        space_ctx: SpaceCtx,
        storage: &Storage,
        interconnect: &ServiceInterconnect,
        dto: QueueMediaProcessRequest,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn complete_media_queue(&self, space_id: Uuid, media_data: MediaData)
        -> impl Future<Output = AppResult<()>> + Send;

    fn list_files(
        &self,
        space_ctx: SpaceCtx,
        album_id: Uuid,
    ) -> impl Future<Output = AppResult<_FileMetaResponseVec>> + Send;

    fn list_files_gallery(&self, space_ctx: SpaceCtx) -> impl Future<Output = AppResult<_FileMetaResponseVec>> + Send;

    fn list_albums(&self, space_ctx: SpaceCtx) -> impl Future<Output = AppResult<_AlbumResponseVec>> + Send;

    fn get_album(&self, space_ctx: SpaceCtx, album_id: Uuid) -> impl Future<Output = AppResult<_AlbumResponse>> + Send;

    fn link_album_files(
        &self,
        space_ctx: SpaceCtx,
        album_id: Uuid,
        file_ids: Vec<Uuid>,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn unlink_album_files(
        &self,
        space_ctx: SpaceCtx,
        album_id: Uuid,
        file_ids: Vec<Uuid>,
    ) -> impl Future<Output = AppResult<()>> + Send;

    fn generate_thumbnail_preview_signed_urls(
        &self,
        space_ctx: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<StreamedUrlResponse>> + Send;

    fn generate_download_signed_url(
        &self,
        space_ctx: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<DownloadUrlResponse>> + Send;

    fn delete_album(&self, space_ctx: SpaceCtx, album_id: Uuid) -> impl Future<Output = AppResult<()>> + Send;

    fn delete_file(
        &self,
        space_ctx: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> impl Future<Output = AppResult<()>> + Send;
}

impl<D: StorageDs> MediaService for ServiceWrapper<'_, D> {
    async fn create_album(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        album_name: String,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot create album: Unauthorized read role"));
        }

        self.ds.create_album(&user_id, space_id, album_name).await.map(|_| ())
    }

    async fn initiate_upload(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        InitiateUploadRequest {
            file_name,
            hash,
        }: InitiateUploadRequest,
    ) -> AppResult<InitiateUploadResponse> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot upload: Unauthorized read role"));
        }

        let file_name = sanitize_file_name(file_name);
        let object_key = get_canonical_object_key(&hash, &file_name);

        let url = storage.generate_upload_signed_url(&space_id.to_string(), &object_key).await?;
        Ok(InitiateUploadResponse {
            url,
            file_name,
        })
    }

    async fn queue_media_process(
        &self,
        UserId(user_id): UserId,
        SpaceCtx {
            space_id,
            role,
            ..
        }: SpaceCtx,
        storage: &Storage,
        interconnect: &ServiceInterconnect,
        QueueMediaProcessRequest {
            file_name,
            hash,
            updated_millis,
            ..
        }: QueueMediaProcessRequest,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot queue media: Unauthorized read role"));
        }

        let Some(updated_date) = DateTime::from_timestamp_millis(updated_millis) else {
            return Err(ErrType::BadRequest.msg("Invalid timestamp"));
        };

        let file_name = sanitize_file_name(file_name);
        let object_key = get_canonical_object_key(&hash, &file_name);

        let space_id_str = space_id.to_string();
        let remote_path = storage.get_remote_path(&space_id_str, &object_key)?;

        let file = self
            .ds
            .get_or_create_file(
                &user_id,
                &space_id,
                &hash,
                file_name,
                object_key,
                updated_date,
                FileData {
                    file_name: String::new(),
                    thumbnail: ImageData::default(),
                    preview: ImageData::default(),
                    metadata: MediaMetadata::default(),
                    size: 0,
                    media_type: smq_dto::MediaType::Image,
                },
            )
            .await?;

        let payload_token = interconnect.get_sending_token()?;
        let mq_url = interconnect.mq_uri("/v1/queue");

        let response = request_mq_retry_until_ok(
            &mq_url,
            &payload_token,
            ProcessMediaRequest {
                file_id: file.id,
                updated_date: MediaDatetime(updated_date),
                space_id,
                s3_file_path: remote_path,
            },
        )
        .await?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(ErrType::ServerError
                .msg(format!("Unable to queue media for processing: {:?}", status.canonical_reason())))
        }
    }

    async fn complete_media_queue(
        &self,
        space_id: Uuid,
        MediaData {
            file_id,
            updated_date,
            mut file_data,
        }: MediaData,
    ) -> AppResult<()> {
        let file = self
            .ds
            .get_file(space_id, file_id)
            .await?
            .ok_or(ErrType::NotFound.msg("File not found while processing completion"))?;

        // Preserve canonical display name; MQ file_name is derived from object key.
        file_data.file_name = file.file_name.clone();

        let thumbnail_key = file_data
            .thumbnail
            .file_name
            .is_empty()
            .then_some(None)
            .unwrap_or_else(|| Some(join_key_dir(&file.object_key, &file_data.thumbnail.file_name)));

        let preview_key = file_data
            .preview
            .file_name
            .is_empty()
            .then_some(None)
            .unwrap_or_else(|| Some(join_key_dir(&file.object_key, &file_data.preview.file_name)));

        self.ds.update_file(file_id, &space_id, updated_date.0, file_data, thumbnail_key, preview_key).await?;

        Ok(())
    }

    async fn list_files(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        album_id: Uuid,
    ) -> AppResult<_FileMetaResponseVec> {
        let _ = self.ds.get_album(&space_id, &album_id).await?.ok_or(ErrType::NotFound.msg("Album not found"))?;

        let mut files = self.ds.list_files(&space_id, &album_id).await?;
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(_FileMetaResponseVec(files))
    }

    async fn list_files_gallery(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<_FileMetaResponseVec> {
        let files = self.ds.list_files_gallery(&space_id).await?;
        let files: Vec<_> = files.into_iter().map(|g| g.0).collect();
        Ok(_FileMetaResponseVec(files))
    }

    async fn list_albums(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
    ) -> AppResult<_AlbumResponseVec> {
        self.ds.list_albums(space_id).await.map(_AlbumResponseVec)
    }

    async fn get_album(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        album_id: Uuid,
    ) -> AppResult<_AlbumResponse> {
        self.ds
            .get_album(&space_id, &album_id)
            .await?
            .ok_or(ErrType::NotFound.msg("Album not found"))
            .map(_AlbumResponse)
    }

    async fn link_album_files(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        album_id: Uuid,
        file_ids: Vec<Uuid>,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot link files: Unauthorized read role"));
        }

        let _ = self.ds.get_album(&space_id, &album_id).await?.ok_or(ErrType::NotFound.msg("Album not found"))?;

        self.ds.link_album_files(&space_id, &album_id, &file_ids).await
    }

    async fn unlink_album_files(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        album_id: Uuid,
        file_ids: Vec<Uuid>,
    ) -> AppResult<()> {
        if let SpaceRole::Read = role {
            return Err(ErrType::Unauthorized.msg("Cannot unlink files: Unauthorized read role"));
        }

        let _ = self.ds.get_album(&space_id, &album_id).await?.ok_or(ErrType::NotFound.msg("Album not found"))?;

        self.ds.unlink_album_files(&space_id, &album_id, &file_ids).await
    }

    async fn generate_thumbnail_preview_signed_urls(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> AppResult<StreamedUrlResponse> {
        let Some(stream_keys) = self.ds.get_thumbnail_preview_stream_keys(&space_id, file_id).await? else {
            return Err(ErrType::NotFound.msg("Requested file not found"));
        };

        let thumbnail_key =
            stream_keys.thumbnail_key.ok_or(ErrType::NotFound.msg("Thumbnail key not found for file"))?;
        let preview_key = stream_keys.preview_key.ok_or(ErrType::NotFound.msg("Preview key not found for file"))?;

        let space_id_str = space_id.to_string();
        let thumbnail_stream = storage.generate_stream_signed_url(&space_id_str, &thumbnail_key).await?;
        let preview_stream = storage.generate_stream_signed_url(&space_id_str, &preview_key).await?;

        Ok(StreamedUrlResponse {
            thumbnail_url: thumbnail_stream,
            preview_url: preview_stream,
        })
    }

    async fn generate_download_signed_url(
        &self,
        SpaceCtx {
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> AppResult<DownloadUrlResponse> {
        let Some(stream_key) = self.ds.get_download_stream_key(&space_id, file_id).await? else {
            return Err(ErrType::NotFound.msg("Requested file not found"));
        };

        let space_id_str = space_id.to_string();
        let download_stream = storage.generate_stream_signed_url(&space_id_str, &stream_key).await?;

        Ok(DownloadUrlResponse {
            url: download_stream,
        })
    }

    async fn delete_album(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        album_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        let _ = self
            .ds
            .get_album(&space_id, &album_id)
            .await?
            .ok_or(ErrType::NotFound.msg("Album not found for deletion"))?;

        self.ds.delete_album(&space_id, &album_id).await
    }

    async fn delete_file(
        &self,
        SpaceCtx {
            role,
            space_id,
            ..
        }: SpaceCtx,
        storage: &Storage,
        file_id: Uuid,
    ) -> AppResult<()> {
        match role {
            SpaceRole::Read | SpaceRole::Upload => {
                return Err(ErrType::Unauthorized.msg("Cannot delete: Unauthorized read|upload role"))
            }
            _ => (),
        };

        if let Some(file) = self.ds.get_file(space_id, file_id).await? {
            storage.delete_file(&space_id.to_string(), file.object_key, file.thumbnail_key, file.preview_key).await?;

            self.ds.delete_file(&file.id, &space_id).await?;

            return Ok(());
        }

        Err(ErrType::NotFound.msg("File not found for deletion"))
    }
}

async fn request_mq_retry_until_ok(
    mq_url: &str,
    payload_token: &str,
    body: ProcessMediaRequest,
) -> AppResult<Response> {
    let max_retries = 3u8;
    let mut retries = 0u8;
    let duration_millis = 255u64;

    loop {
        let response = reqwest::Client::new().post(mq_url).bearer_auth(payload_token).json(&body).send().await;
        match response {
            Ok(response) => return Ok(response),
            Err(err) => {
                if retries >= max_retries {
                    return Err(ErrType::ServerError.err(err, "Failed to request media queue"));
                }
                retries += 1;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(duration_millis * retries as u64)).await;
    }
}

fn sanitize_file_name(file_name: String) -> String {
    Path::new(&file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|v| v.to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or(file_name)
}

fn get_canonical_object_key(hash: &str, file_name: &str) -> String {
    format!("space/{}_{}", hash, file_name)
}

fn join_key_dir(object_key: &str, file_name: &str) -> String {
    if let Some(parent) = Path::new(object_key).parent().and_then(|p| p.to_str())
        && !parent.is_empty()
        && parent != "."
    {
        return format!("{parent}/{file_name}");
    }

    file_name.to_owned()
}
