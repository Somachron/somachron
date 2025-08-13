use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use super::{config::ClerkConfig, AppResult, ErrType};

#[derive(Deserialize, Clone)]
pub struct TokenClaims {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub picture: String,
    pub updated_at: f64,
}

pub struct ClerkAuth {
    rsa_key: DecodingKey,
    validation: Validation,
}

impl ClerkAuth {
    pub fn new() -> Self {
        let config = ClerkConfig::new();
        let decoding_key = DecodingKey::from_rsa_pem(config.pem.as_bytes()).expect("Failed to init decoding pem");

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&[config.aud.as_str()]);
        validation.validate_exp = true;
        validation.validate_nbf = true;

        Self {
            rsa_key: decoding_key,
            validation,
        }
    }

    pub fn validate_token_for_claims(&self, token: &str) -> AppResult<TokenClaims> {
        decode::<TokenClaims>(token, &self.rsa_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|err| ErrType::Unauthorized.err(err, "Invalid token"))
    }
}

pub mod webhook {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, Validate, ToSchema)]
    pub struct EventUpdateData {
        id: String,
        image_url: Option<String>,
        first_name: String,
    }

    #[derive(Deserialize, Validate, ToSchema)]
    pub struct UserUpdateEvent {
        data: EventUpdateData,
    }

    impl UserUpdateEvent {
        pub fn get_data_update(self) -> UserUpdate {
            UserUpdate {
                id: self.data.id,
                name: self.data.first_name,
                picture_url: self.data.image_url.unwrap_or_default(),
            }
        }
    }

    pub struct UserUpdate {
        pub id: String,
        pub name: String,
        pub picture_url: String,
    }
}
