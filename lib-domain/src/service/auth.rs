use lib_core::{google::TokenClaims, AppResult};

use super::Service;

impl Service {
    pub async fn exchange_code_routine(&self, claims: TokenClaims) -> AppResult<()> {
        match self.ds.get_user_by_email(&claims.email).await? {
            Some(user) => self.ds.update_user(&user.id, &claims.given_name, &claims.picture).await.map(|_| ()),
            None => self.ds.insert_user(&claims.given_name, &claims.email, &claims.picture).await.map(|_| ()),
        }
    }
}
