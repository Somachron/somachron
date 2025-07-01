pub mod res {
    use lib_core::google::AuthCode;
    use ser_mapper::impl_dto;
    use utoipa::ToSchema;

    impl_dto!(
        #[derive(ToSchema)]
        pub struct AuthTokenResponse<AuthCode> {
            access_token: String = id_token,
            refresh_token: String = refresh_token => |rt: &Option<String>| -> &str {
                match rt {
                    Some(t) => t.as_str(),
                    None => ""
                }
            },
            expires_in: u16 = expires_in,
        }
    );
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct ExchangeCodeRequest {
        #[validate(length(min = 64, max = 127))]
        pub code: String,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct RefreshTokenRequest {
        #[validate(length(min = 64, max = 127))]
        pub refresh_token: String,
    }
}
