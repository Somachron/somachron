use lib_core::{google::AuthCode, AppResult};

use crate::dto::auth::res::_AuthTokenResponse;

use super::Service;

impl Service {
    pub async fn exchange_code_routine(&self, auth_code: AuthCode) -> AppResult<_AuthTokenResponse> {
        Ok(_AuthTokenResponse(auth_code))
    }
}
