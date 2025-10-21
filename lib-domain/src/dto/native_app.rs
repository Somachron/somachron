pub mod res {
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(Serialize, ToSchema)]
    pub struct NativeAppIdentifierResponse {
        pub data: String,
    }
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct NativeAppIdentifierRequest {
        pub identifier: String,
    }
}
