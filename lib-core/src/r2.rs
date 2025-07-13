use aws_config::Region;
use aws_sdk_s3::{
    config::{
        endpoint::{Endpoint, EndpointFuture, Params, ResolveEndpoint},
        Credentials,
    },
    presigning::PresigningConfig,
    primitives::ByteStream,
    Client, Config,
};

use crate::{config::R2Config, AppError, AppResult, ErrType};

/// Max video part to download - 5 MB
const VIDEO_PREVIEW_SIZE: usize = 5 * 1024 * 1024;

#[derive(Debug)]
struct R2Endpoint {
    account_id: &'static str,
    bucket_name: &'static str,
}

impl ResolveEndpoint for R2Endpoint {
    fn resolve_endpoint<'a>(&'a self, _params: &'a Params) -> EndpointFuture<'a> {
        EndpointFuture::ready(Ok(Endpoint::builder()
            .url(format!("https://{}.r2.cloudflarestorage.com/{}", self.account_id, self.bucket_name))
            .build()))
    }
}

/// Client for handling functions for R2
/// storage providers
pub(super) struct R2Storage {
    /// R2 client
    client: Client,

    /// Bucket name - user configured from secrets
    bucket_name: &'static str,
}

impl R2Storage {
    pub(super) fn new() -> Self {
        let config = R2Config::new();

        let creds = Credentials::new(config.access_key, config.secret_key, None, None, "static");
        let endpoint_resolver = R2Endpoint {
            account_id: config.account_id,
            bucket_name: config.bucket_name,
        };

        let client_config = Config::builder()
            .region(Region::from_static("auto"))
            .endpoint_resolver(endpoint_resolver)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();

        Self {
            client: Client::from_conf(client_config),
            bucket_name: config.bucket_name,
        }
    }

    pub(super) async fn create_folder(&self, path: &str) -> AppResult<()> {
        let stream = ByteStream::from("fd".as_bytes().to_vec());
        let builder = self.client.put_object().bucket(self.bucket_name);
        let result = builder.key(format!("{path}/fd.dat")).body(stream).send().await;
        result.map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to create dir"))?;
        Ok(())
    }

    pub(super) async fn generate_upload_signed_url(&self, path: &str) -> AppResult<String> {
        let config = PresigningConfig::expires_in(std::time::Duration::from_secs(60 * 60))
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to generate presign config"))?;

        let request = self
            .client
            .put_object()
            .bucket(self.bucket_name)
            .key(path)
            .presigned(config)
            .await
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to generate upload presigned URL"))?;

        Ok(request.uri().to_string())
    }

    pub(super) async fn download_photo(&self, path: &str) -> AppResult<Vec<u8>> {
        let builder = self.client.get_object().bucket(self.bucket_name);
        let result = builder
            .key(path)
            .send()
            .await
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to download photo"))?;

        result
            .body
            .collect()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to collect bytes"))
    }

    pub(super) async fn download_video(&self, path: &str) -> AppResult<Vec<u8>> {
        let builder = self.client.get_object().bucket(self.bucket_name);
        let result = builder
            .key(path)
            .range(format!("bytes=0-{VIDEO_PREVIEW_SIZE}"))
            .send()
            .await
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to download video"))?;

        result
            .body
            .collect()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to collect bytes"))
    }
}
