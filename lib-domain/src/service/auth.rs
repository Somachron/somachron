use lib_core::{clerk::TokenClaims, AppResult, ErrType, ErrorContext};

use crate::datastore::{native_app::NativeAppDs, user::UserDs};

use super::ServiceWrapper;

pub trait AuthService: Send + Sync {
    fn exchange_code_routine(&self, claims: TokenClaims) -> impl Future<Output = AppResult<()>> + Send;
    fn validate_native_app(&self, identifier: String) -> impl Future<Output = AppResult<()>> + Send;
}

impl<D: UserDs + NativeAppDs> AuthService for ServiceWrapper<'_, D> {
    async fn exchange_code_routine(&self, claims: TokenClaims) -> AppResult<()> {
        match self.ds.get_user_by_clerk_id(&claims.sub).await? {
            Some(user) => {
                if claims.updated_at > user.updated_at.timestamp() as f64 {
                    self.ds
                        .update_user(user.id, &claims.name, "", &claims.picture)
                        .await
                        .context("claims timestamp was updated")
                } else {
                    Ok(user)
                }
            }
            None => self.ds.insert_user(claims).await.context("user by clerk id was null"),
        }
        .and_then(|user| {
            if user.allowed {
                Ok(())
            } else {
                Err(ErrType::Unauthorized.msg("Not allowed"))
            }
        })
    }

    async fn validate_native_app(&self, identifier: String) -> AppResult<()> {
        self.ds.validate_native_app(identifier).await
    }
}
