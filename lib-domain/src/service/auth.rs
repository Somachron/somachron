use lib_core::{clerk::TokenClaims, AppResult, ErrType};

use crate::datastore::{native_app::NativeAppDs, user::UserDs};

use super::Service;

impl<D: UserDs + NativeAppDs> Service<D> {
    pub async fn exchange_code_routine(&self, claims: TokenClaims) -> AppResult<()> {
        match self.ds.get_user_by_clerk_id(&claims.sub).await? {
            Some(user) => {
                if claims.updated_at > user.updated_at.timestamp() as f64 {
                    self.ds.update_user(user.id, &claims.name, "", &claims.picture).await
                } else {
                    Ok(user)
                }
            }
            None => self.ds.insert_user(claims).await,
        }
        .and_then(|user| {
            if user.allowed {
                Ok(())
            } else {
                Err(ErrType::Unauthorized.msg("Not allowed"))
            }
        })
    }

    pub async fn validate_native_app(&self, identifier: String) -> AppResult<()> {
        self.ds.validate_native_app(identifier).await
    }
}
