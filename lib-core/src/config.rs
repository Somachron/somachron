pub fn get_host_addr() -> String {
    let port = std::env::var("PORT").unwrap_or("8080".into());
    format!("[::]:{port}")
}

pub fn get_volume_path() -> String {
    std::env::var("VOLUME_PATH").unwrap_or_default()
}

pub struct DbConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}
impl DbConfig {
    pub fn new() -> Self {
        Self {
            url: std::env::var("DATABASE_URL").unwrap_or_default(),
            username: std::env::var("DATABASE_USERNAME").unwrap_or_default(),
            password: std::env::var("DATABASE_PASSWORD").unwrap_or_default(),
        }
    }
}

pub(crate) struct R2Config {
    pub account_id: String,
    pub bucket_name: String,
    pub access_key: String,
    pub secret_key: String,
}

impl R2Config {
    pub(crate) fn new() -> Self {
        Self {
            account_id: std::env::var("R2_ACCOUNT_ID").unwrap_or_default(),
            bucket_name: std::env::var("R2_BUCKET").unwrap_or_default(),
            access_key: std::env::var("R2_ACCESS_KEY").unwrap_or_default(),
            secret_key: std::env::var("R2_SECRET_KEY").unwrap_or_default(),
        }
    }
}

pub(crate) struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl GoogleConfig {
    pub(crate) fn new() -> Self {
        Self {
            client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            redirect_uri: std::env::var("GOOGLE_REDIRECT_URI").unwrap_or_default(),
        }
    }
}
