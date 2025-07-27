use std::collections::BTreeMap;

use jsonwebtoken::{decode, decode_header, jwk::JwkSet, DecodingKey, Validation};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{config::GoogleConfig, AppResult, ErrType};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_TOKEN_URL: &str = "https://oauth2.googleapis.com/revoke?token=";
const CERTS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

#[derive(Deserialize)]
pub struct AuthCode {
    pub access_token: String,
    pub expires_in: u16,
    pub id_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
pub struct TokenClaims {
    pub email: String,
    pub given_name: String,
    pub picture: String,
}

pub struct GoogleAuth {
    config: GoogleConfig,
    client: reqwest::Client,
    decoding_keys: BTreeMap<String, DecodingKey>,
    validation: Validation,
}

impl GoogleAuth {
    pub async fn new() -> Self {
        let config = GoogleConfig::new();

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&[&config.client_id]);
        validation.validate_exp = true;
        validation.validate_nbf = true;

        Self {
            config,
            client: reqwest::Client::new(),
            decoding_keys: Self::get_keys().await,
            validation,
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
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("code", &code),
            ])
            .send()
            .await
            .map_err(|err| ErrType::ServerError.err(err, "Failed to request exchange"))?;

        match res.status() {
            StatusCode::OK => res
                .json::<AuthCode>()
                .await
                .map_err(|err| ErrType::InvalidBody.err(err, "Failed to parse exchange code response")),
            _ => Err(ErrType::BadRequest.new(res.text().await.unwrap_or_default())),
        }
    }

    pub async fn refresh_token(&self, refresh_token: String) -> AppResult<AuthCode> {
        let res = self
            .client
            .post(TOKEN_URL)
            .header("Content-Length", 0)
            .header("Accept", "*/*")
            .query(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("refresh_token", &refresh_token),
            ])
            .send()
            .await
            .map_err(|err| ErrType::ServerError.err(err, "Failed to request exchange"))?;

        match res.status() {
            StatusCode::OK => res
                .json::<AuthCode>()
                .await
                .map_err(|err| ErrType::InvalidBody.err(err, "Failed to parse exchange code response")),
            _ => Err(ErrType::BadRequest.new(res.text().await.unwrap_or_default())),
        }
    }

    pub async fn revoke_token(&self, token: &str) -> AppResult<()> {
        let res = self
            .client
            .post(format!("{REVOKE_TOKEN_URL}{token}"))
            .header("Content-Length", 0)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .await
            .map_err(|err| ErrType::ServerError.err(err, "Failed to send revoke request"))?;

        match res.status() {
            StatusCode::OK => Ok(()),
            _ => Err(ErrType::BadRequest.new(res.text().await.unwrap_or_default())),
        }
    }

    pub fn validate_token_for_claims(&self, token: &str) -> AppResult<TokenClaims> {
        let header = decode_header(token).map_err(|err| ErrType::Unauthorized.err(err, "Failed to parse header"))?;
        let kid = header.kid.ok_or(ErrType::Unauthorized.new("Missing kid"))?;

        let decoding_key = self.decoding_keys.get(&kid).ok_or(ErrType::Unauthorized.new("Invalid kid"))?;

        decode::<TokenClaims>(token, decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|err| ErrType::Unauthorized.err(err, "Invalid token"))
    }
}
