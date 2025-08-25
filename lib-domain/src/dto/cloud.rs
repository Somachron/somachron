pub mod res {
    use lib_core::storage::MediaType;
    use ser_mapper::impl_dto;
    use serde::Serialize;
    use utoipa::ToSchema;

    use crate::{
        datastore::storage::{File, FileMeta, Folder, Metadata},
        dto::{Datetime, _IdOptionRef, _IdRef},
    };

    #[derive(Serialize, ToSchema)]
    pub struct InitiateUploadResponse {
        pub url: String,
        pub file_name: String,
    }

    #[derive(Serialize, ToSchema)]
    pub struct StreamedUrlsResponse {
        pub original_stream: String,
        pub thumbnail_stream: String,
    }

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileMetadataResponse<Metadata> {
            make: Option<String> = make,
            model: Option<String> = model,
            software: Option<String> = software,

            image_height: u64 = image_height,
            image_width: u64 = image_width,

            duration: Option<String> = duration,
            media_duration: Option<String> = media_duration,
            frame_rate: Option<f64> = frame_rate,

            date_time: Option<Datetime> = date_time,
            iso: Option<u64> = iso,
            shutter_speed: Option<String> = shutter_speed,
            aperture: Option<f64> = aperture,
            f_number: Option<f64> = f_number,
            exposure_time: Option<String> = exposure_time,

            latitude: Option<f64> = latitude,
            longitude: Option<f64> = longitude,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileResponse<File> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            file_name: String = file_name,
            file_size: u64 = file_size,
            media_type: MediaType = media_type,
            thumbnail_file_name: String = thumbnail_file_name,
            path: String = path,
            user: String = user => _IdOptionRef,
            space: String = space => _IdRef,
            metadata: FileMetadataResponse = metadata => _FileMetadataResponseRef,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileMetaResponse<FileMeta> {
            id: String = id => _IdRef,

            file_name: String = file_name,
            media_type: MediaType = media_type,
            user: Option<String> = user => _IdOptionRef,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FolderResponse<Folder> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            name: String = name,
        }
    );
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct InitiateUploadRequest {
        #[validate(length(equal = 64))]
        pub folder_id: String,

        #[validate(length(min = 3))]
        pub file_name: String,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct UploadCompleteRequest {
        #[validate(length(min = 3))]
        pub folder_id: String,
        pub file_name: String,
        pub file_size: usize,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct CreateFolderRequest {
        #[validate(length(equal = 64))]
        pub parent_folder_id: String,

        #[validate(length(min = 3))]
        pub folder_name: String,
    }
}
