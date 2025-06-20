use dotenv_codegen::dotenv;

pub struct R2Config {
    pub account_id: &'static str,
    pub bucket_name: &'static str,
    pub access_key: &'static str,
    pub secret_key: &'static str,
}

impl R2Config {
    pub fn new() -> Self {
        Self {
            account_id: dotenv!("R2_ACCOUNT_ID", "missing R2 account ID"),
            bucket_name: dotenv!("R2_BUCKET", "missing R2 bucket"),
            access_key: dotenv!("R2_ACCESS_KEY", "missing R2 access key"),
            secret_key: dotenv!("R2_SECRET_KEY", "missing R2 secret key"),
        }
    }
}
