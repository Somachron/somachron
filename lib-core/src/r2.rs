use aws_config::Region;
use aws_sdk_s3::{
    config::{
        endpoint::{Endpoint, EndpointFuture, Params, ResolveEndpoint},
        Credentials,
    },
    Client, Config,
};

use crate::config::R2Config;

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
pub struct R2Storage {
    /// R2 client
    client: Client,

    /// Bucket name - user configured from secrets
    bucket_name: &'static str,
}

impl R2Storage {
    pub fn new(config: R2Config) -> Self {
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
}
