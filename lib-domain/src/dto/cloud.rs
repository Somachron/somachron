pub mod res {
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(Serialize, ToSchema)]
    pub struct UploadSignedUrlResponse {
        pub url: String,
    }
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct UploadSignedUrlRequest {
        #[validate(length(min = 3))]
        pub file_path: String,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct UploadCompleteRequest {
        #[validate(length(min = 3))]
        pub file_path: String,

        pub file_size: usize,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct CreateFolderRequest {
        #[validate(length(min = 3))]
        pub folder_path: String,
    }
}
