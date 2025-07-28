use lib_core::{google::TokenClaims, AppResult, ErrType};

use crate::extension::IdStr;

use super::Service;

impl Service {
    pub async fn exchange_code_routine(&self, claims: TokenClaims) -> AppResult<String> {
        match self.ds.get_user_by_email(&claims.email).await? {
            Some(user) => self.ds.update_user(user.id, &claims.given_name, &claims.picture).await,
            None => self.ds.insert_user(&claims.given_name, &claims.email, &claims.picture).await,
        }
        .and_then(|user| {
            if user.allowed {
                Ok(user.id.id())
            } else {
                Err(ErrType::Unauthorized.new("Not allowed"))
            }
        })
    }
}
