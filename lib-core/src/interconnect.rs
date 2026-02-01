use base64::Engine;
use openssl::{
    pkey::{Private, Public},
    rsa::{Padding, Rsa},
};
use uuid::Uuid;

use crate::{config::SIConfig, AppResult, ErrType};

pub struct ServiceInterconnect {
    rsa_pub: Rsa<Public>,
    rsa_priv: Rsa<Private>,
    backend_url: String,
    mq_url: String,
}

impl ServiceInterconnect {
    pub fn new() -> Self {
        let config = SIConfig::new();
        let pub_key = bas64_decode(config.pub_pem.as_bytes()).expect("Failed to decode pub key pem");
        let priv_key = bas64_decode(config.priv_pem.as_bytes()).expect("Failed to decode priv key pem");

        let rsa_pub = Rsa::public_key_from_pem(&pub_key).expect("Failed to generate rsa from public key");

        let rsa_priv = Rsa::private_key_from_pem(&priv_key).expect("Failed to generate rsa from private key");

        Self {
            rsa_pub,
            rsa_priv,
            backend_url: config.backend_url,
            mq_url: config.mq_url,
        }
    }

    pub fn validate_token(&self, token: &str) -> AppResult<()> {
        let bytes = bas64_decode(token.as_bytes())?;

        let mut decrypted = vec![0; bytes.len()];
        self.rsa_pub
            .public_decrypt(&bytes, &mut decrypted, Padding::PKCS1)
            .map_err(|err| ErrType::Unauthorized.err(err, "Tampered token"))?;

        Uuid::from_slice(&decrypted[..16]).map_err(|err| ErrType::Unauthorized.err(err, "Invalid token"))?;

        Ok(())
    }

    pub fn get_sending_token(&self) -> AppResult<String> {
        let token = Uuid::now_v7();
        let mut encrypted = vec![0; self.rsa_priv.size() as usize];
        self.rsa_priv
            .private_encrypt(token.as_bytes(), &mut encrypted, Padding::PKCS1)
            .map_err(|err| ErrType::ServerError.err(err, "Error encrypting sending token"))?;

        Ok(base64_encode(&encrypted))
    }

    pub fn backend_uri(&self, uri: impl Into<String>) -> String {
        format!("{}{}", self.backend_url, uri.into())
    }

    pub fn mq_uri(&self, uri: impl Into<String>) -> String {
        format!("{}{}", self.mq_url, uri.into())
    }

    #[warn(unused)]
    pub fn generate_key() {
        let rsa = Rsa::generate(4096).unwrap();
        let pub_pem = rsa.public_key_to_pem().unwrap();
        let pub_pem = String::from_utf8(pub_pem).unwrap();
        println!("pub:\n{pub_pem}");

        let priv_pem = rsa.private_key_to_pem().unwrap();
        let priv_pem = String::from_utf8(priv_pem).unwrap();
        println!("priv:\n{priv_pem}");
    }
}

fn base64_encode(buf: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(buf)
}

fn bas64_decode(buf: &[u8]) -> AppResult<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(buf)
        .map_err(|err| ErrType::ServerError.err(err, "Error decoding base64"))
}
