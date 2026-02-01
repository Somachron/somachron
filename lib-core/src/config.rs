pub fn get_host_addr() -> String {
    let port = std::env::var("PORT").unwrap_or("8080".into());
    format!("[::]:{port}")
}

pub fn get_volume_path() -> String {
    std::env::var("VOLUME_PATH").unwrap_or_default()
}

#[derive(Debug)]
pub struct SIConfig {
    pub pub_pem: String,
    pub priv_pem: String,
    pub backend_url: String,
    pub mq_url: String,
}
impl SIConfig {
    pub fn new() -> Self {
        Self {
            pub_pem: std::env::var("SI_PUB").unwrap_or_default(),
            priv_pem: std::env::var("SI_PRIV").unwrap_or_default(),
            backend_url: std::env::var("SI_BACKEND_URL").unwrap_or_default(),
            mq_url: std::env::var("SI_MQ_URL").unwrap_or_default(),
        }
    }
}

#[derive(Default)]
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

#[derive(Debug)]
pub(crate) struct S3Config {
    pub bucket_name: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
    pub region: String,
}

impl S3Config {
    pub(crate) fn new() -> Self {
        Self {
            bucket_name: std::env::var("S3_BUCKET").unwrap_or_default(),
            access_key: std::env::var("S3_ACCESS_KEY_ID").unwrap_or_default(),
            secret_key: std::env::var("S3_SECRET_ACCESS_KEY").unwrap_or_default(),
            endpoint: std::env::var("S3_ENDPOINT").unwrap_or_default(),
            region: std::env::var("S3_REGION").unwrap_or(String::from("auto")),
        }
    }
}

pub(crate) struct ClerkConfig {
    pub aud: String,
    pub pem: String,
    pub publishable_key: String,
}

impl ClerkConfig {
    pub(crate) fn new() -> Self {
        Self {
            aud: std::env::var("CLERK_AUD").unwrap_or_default(),
            pem: std::env::var("CLERK_PEM").unwrap_or_default(),
            publishable_key: std::env::var("CLERK_PUBLISHABLE_KEY").unwrap_or_default(),
        }
    }
}
