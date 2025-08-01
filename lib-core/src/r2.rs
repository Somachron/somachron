use std::path::PathBuf;

use aws_config::Region;
use aws_sdk_s3::{
    config::{
        endpoint::{Endpoint, EndpointFuture, Params, ResolveEndpoint},
        Credentials,
    },
    presigning::PresigningConfig,
    primitives::ByteStream,
    types::{Delete, ObjectIdentifier},
    Client, Config,
};

use crate::{config::R2Config, AppResult, ErrType};

#[derive(Debug)]
struct R2Endpoint {
    account_id: String,
    bucket_name: String,
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
    bucket_name: String,
}

impl R2Storage {
    pub(super) fn new() -> Self {
        let config = R2Config::new();

        let creds = Credentials::new(config.access_key, config.secret_key, None, None, "static");
        let endpoint_resolver = R2Endpoint {
            account_id: config.account_id,
            bucket_name: config.bucket_name.clone(),
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
        let builder = self.client.put_object().bucket(&self.bucket_name);
        let result = builder.key(format!("{path}/fd.dat")).body(stream).send().await;
        result.map_err(|err| ErrType::r2_put(err, "Failed to create dir"))?;
        Ok(())
    }

    pub(super) async fn generate_upload_signed_url(&self, path: &str) -> AppResult<String> {
        let config = PresigningConfig::expires_in(std::time::Duration::from_secs(60 * 60))
            .map_err(|err| ErrType::R2Error.err(err, "Failed to generate presign config"))?;

        let request = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(path)
            .presigned(config)
            .await
            .map_err(|err| ErrType::r2_put(err, "Failed to generate upload presigned URL"))?;

        Ok(request.uri().to_string())
    }

    pub(super) async fn generate_download_signed_url(&self, path: &str) -> AppResult<String> {
        let config = PresigningConfig::expires_in(std::time::Duration::from_secs(3 * 60 * 60))
            .map_err(|err| ErrType::R2Error.err(err, "Failed to generate presign config"))?;

        let request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(path)
            .presigned(config)
            .await
            .map_err(|err| ErrType::r2_get(err, "Faiedl to generate download presigned URL"))?;

        Ok(request.uri().to_string())
    }

    pub(super) async fn upload_photo(&self, path: &str, from_path: PathBuf) -> AppResult<()> {
        let stream = ByteStream::read_from()
            .path(from_path)
            .buffer_size(4096)
            .build()
            .await
            .map_err(|err| ErrType::FsError.err(err, "Failed from create byte stream from path"))?;
        let builder = self.client.put_object().bucket(&self.bucket_name);
        let result = builder.key(path).body(stream).send().await;
        result.map_err(|err| ErrType::r2_put(err, "Failed to upload photo"))?;
        Ok(())
    }

    pub(super) async fn download_media(&self, path: &str) -> AppResult<ByteStream> {
        let builder = self.client.get_object().bucket(&self.bucket_name);
        let result =
            builder.clone().key(path).send().await.map_err(|err| ErrType::r2_get(err, "Failed to download media"))?;
        Ok(result.body)
    }

    pub(super) async fn delete_folder(&self, path: &str) -> AppResult<()> {
        let objects = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket_name)
            .prefix(path)
            .send()
            .await
            .map_err(|err| ErrType::r2_list_err(err, "Failed to list objects"))?;

        let mut delete_objects = Vec::<ObjectIdentifier>::new();
        for obj in objects.contents().into_iter() {
            if let Some(key) = obj.key() {
                let id = ObjectIdentifier::builder()
                    .key(key)
                    .build()
                    .map_err(|err| ErrType::R2Error.err(err, "Failed to build object identifier"))?;
                delete_objects.push(id);
            }
        }

        let delete = Delete::builder()
            .set_objects(Some(delete_objects))
            .build()
            .map_err(|err| ErrType::R2Error.err(err, "Failed to create delete param"))?;
        let _ = self
            .client
            .delete_objects()
            .bucket(&self.bucket_name)
            .delete(delete)
            .send()
            .await
            .map_err(|err| ErrType::R2Error.err(err.into_service_error(), "Failed to delete folder objects"))?;
        Ok(())
    }

    pub(super) async fn delete_key(&self, path: &str) -> AppResult<()> {
        let builder = self.client.delete_object().bucket(&self.bucket_name);
        let _ = builder.key(path).send().await.map_err(|err| ErrType::r2_delete(err, "Failed to delete object"))?;
        Ok(())
    }
}
