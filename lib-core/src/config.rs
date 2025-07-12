use dotenv_codegen::dotenv;

pub fn get_host_addr() -> String {
    let port = std::env::var("PORT").unwrap_or("8080".into());
    format!("[::]:{port}")
}

pub fn get_db_url() -> &'static str {
    dotenv!("DATABASE_URL", "missing db url")
}

pub fn get_volume_path() -> &'static str {
    dotenv!("VOLUME_PATH", "missing volume path")
}

pub(crate) struct R2Config {
    pub account_id: &'static str,
    pub bucket_name: &'static str,
    pub access_key: &'static str,
    pub secret_key: &'static str,
}

impl R2Config {
    pub(crate) fn new() -> Self {
        Self {
            account_id: dotenv!("R2_ACCOUNT_ID", "missing R2 account ID"),
            bucket_name: dotenv!("R2_BUCKET", "missing R2 bucket"),
            access_key: dotenv!("R2_ACCESS_KEY", "missing R2 access key"),
            secret_key: dotenv!("R2_SECRET_KEY", "missing R2 secret key"),
        }
    }
}

pub(crate) struct GoogleConfig {
    pub client_id: &'static str,
    pub client_secret: &'static str,
    pub redirect_uri: &'static str,
}

impl GoogleConfig {
    pub(crate) fn new() -> Self {
        Self {
            client_id: dotenv!("GOOGLE_CLIENT_ID", "missing google client ID"),
            client_secret: dotenv!("GOOGLE_CLIENT_SECRET", "missing google client secret"),
            redirect_uri: dotenv!("GOOGLE_REDIRECT_URI", "missing google redirect uri"),
        }
    }
}
