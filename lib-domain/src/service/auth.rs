use lib_core::{clerk::TokenClaims, AppResult, ErrType};

use super::Service;

impl Service {
    pub async fn exchange_code_routine(&self, claims: TokenClaims) -> AppResult<()> {
        match self.ds.get_user_by_clerk_id(&claims.sub).await? {
            Some(user) => {
                if claims.updated_at > user.updated_at.timestamp() as f64 {
                    self.ds.update_user(user.id, &claims.name, &claims.picture).await
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
}
