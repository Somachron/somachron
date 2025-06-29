use std::collections::BTreeMap;

use jsonwebtoken::{jwk::JwkSet, DecodingKey};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{config::GoogleConfig, AppError, AppResult, ErrType};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CERTS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

pub struct GoogleAuth {
    config: GoogleConfig,
    client: reqwest::Client,
    decoding_keys: BTreeMap<String, DecodingKey>,
}

impl GoogleAuth {
    pub async fn new() -> Self {
        Self {
            config: GoogleConfig::new(),
            client: reqwest::Client::new(),
            decoding_keys: Self::get_keys().await,
        }
    }

    async fn get_keys() -> BTreeMap<String, DecodingKey> {
        let res = reqwest::get(CERTS_URL).await.expect("Failed to request JWKs");
        let jwkset = match res.status() {
            StatusCode::OK => res.json::<JwkSet>().await.expect("Failed to parse jwks"),
            _ => unreachable!("Failed to fetch jwks"),
        };

        jwkset
            .keys
            .into_iter()
            .map(|jwk| {
                let key = DecodingKey::from_jwk(&jwk).expect("Failed to create decoding key");
                (jwk.common.key_id.unwrap_or_default(), key)
            })
            .collect()
    }

    pub async fn exchange_code(&self, code: String) -> AppResult<AuthCode> {
        let res = self
            .client
            .post(TOKEN_URL)
            .header("Content-Length", 0)
            .header("Accept", "*/*")
            .query(&[
                ("client_id", self.config.client_id),
                ("client_secret", self.config.client_secret),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.config.redirect_uri),
                ("code", &code),
            ])
            .send()
            .await
            .map_err(|err| AppError::err(ErrType::ServerError, err, "Failed to request exchange"))?;

        match res.status() {
            StatusCode::OK => res
                .json::<AuthCode>()
                .await
                .map_err(|err| AppError::err(ErrType::InvalidBody, err, "Failed to parse exchange code response")),
            _ => Err(AppError::new(ErrType::BadRequest, res.text().await.unwrap_or_default())),
        }
    }
}

#[derive(Deserialize)]
pub struct AuthCode {
    pub access_token: String,
    pub expires_in: u16,
    pub id_token: String,
    pub refresh_token: String,
}
