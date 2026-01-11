pub mod res {
    use lib_core::storage::media::{ImageMeta, MediaType};
    use ser_mapper::impl_dto;
    use serde::Serialize;
    use utoipa::ToSchema;

    use crate::{
        datastore::storage::{FileMeta, FsNode, Metadata, NodeMetadata},
        dto::{Datetime, _IdOptionRef, _IdRef},
    };

    #[derive(Serialize, ToSchema)]
    pub struct InitiateUploadResponse {
        pub url: String,
        pub file_name: String,
    }

    #[derive(Serialize, ToSchema)]
    pub struct StreamedUrlResponse {
        pub url: String,
    }

    impl_dto!(
        #[derive(ToSchema)]
        pub struct MediaMetadataResponse<Metadata> {
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
        pub struct ThumbnailMetadataResponse<ImageMeta> {
            file_name: String = file_name,
            width: u32 = width,
            height: u32 = height,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileMetadataResponse<NodeMetadata> {
            thumbnail_meta: Option<ThumbnailMetadataResponse> = thumbnail_meta => _ThumbnailMetadataResponseOptionRef,
            file_meta: Option<MediaMetadataResponse> = file_meta => _MediaMetadataResponseOptionRef,
            media_type: Option<MediaType> = media_type,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileResponse<FsNode> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            file_name: String = node_name,
            file_size: u64 = node_size,
            path: String = path,
            user: String = user_id => _IdOptionRef,
            space: String = space_id => _IdRef,
            metadata: FileMetadataResponse = metadata => _FileMetadataResponseRef,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FileMetaResponse<FileMeta> {
            id: String = id => _IdRef,
            updated_at: Datetime = updated_at,

            file_name: String = file_name,
            media_type: MediaType = media_type,
            user: Option<String> = user => _IdOptionRef,
            width: u32 = width,
            height: u32 = height,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct FolderResponse<FsNode> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            name: String = node_name,
            path: String = path,
        }
    );
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    use crate::dto::DtoUuid;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct InitiateUploadRequest {
        pub folder_id: DtoUuid,

        #[validate(length(min = 3))]
        pub file_name: String,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct UploadCompleteRequest {
        pub folder_id: DtoUuid,

        #[validate(length(min = 3))]
        pub file_name: String,
        pub file_size: usize,
        pub updated_millis: i64,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct CreateFolderRequest {
        pub parent_folder_id: DtoUuid,

        #[validate(length(min = 3))]
        pub folder_name: String,
    }
}
