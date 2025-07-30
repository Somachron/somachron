pub mod res {
    use lib_core::storage::MediaType;
    use ser_mapper::impl_dto;
    use serde::Serialize;
    use utoipa::{PartialSchema, ToSchema};

    use crate::{
        datastore::storage::{File, Metadata},
        dto::{Datetime, _IdRef},
    };

    #[derive(Serialize, ToSchema)]
    pub struct SignedUrlResponse {
        pub url: String,
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
            thumbnail_path: String = thumbnail_path,
            r2_path: String = r2_path,
            member: String = member => _IdRef,
            metadata: FileMetadataResponse = metadata => _FileMetadataResponseRef,
        }
    );

    #[derive(Serialize)]
    #[serde(untagged)]
    pub enum FileEntryResponse {
        Dir {
            tag: &'static str,
            name: String,
        },
        File {
            tag: &'static str,
            file: _FileResponse,
        },
    }
    impl FileEntryResponse {
        pub fn dir(dir: String) -> Self {
            Self::Dir {
                tag: "dir",
                name: dir,
            }
        }
        pub fn file(file: _FileResponse) -> Self {
            Self::File {
                tag: "file",
                file,
            }
        }
    }

    impl ToSchema for FileEntryResponse {}
    impl PartialSchema for FileEntryResponse {
        fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
            utoipa::openapi::RefOr::T(utoipa::openapi::Schema::OneOf(
                utoipa::openapi::OneOfBuilder::new()
                    .item(
                        utoipa::openapi::ObjectBuilder::new()
                            .property(
                                "tag",
                                utoipa::openapi::ObjectBuilder::new().schema_type(
                                    utoipa::openapi::schema::SchemaType::new(utoipa::openapi::schema::Type::String),
                                ),
                            )
                            .required("tag")
                            .property(
                                "name",
                                utoipa::openapi::ObjectBuilder::new().schema_type(
                                    utoipa::openapi::schema::SchemaType::new(utoipa::openapi::schema::Type::String),
                                ),
                            )
                            .required("name"),
                    )
                    .item(
                        utoipa::openapi::ObjectBuilder::new()
                            .property(
                                "tag",
                                utoipa::openapi::ObjectBuilder::new().schema_type(
                                    utoipa::openapi::schema::SchemaType::new(utoipa::openapi::schema::Type::String),
                                ),
                            )
                            .required("tag")
                            .property(
                                "file",
                                utoipa::openapi::schema::RefBuilder::new()
                                    .ref_location_from_schema_name(FileResponse::name()),
                            )
                            .required("file"),
                    )
                    .build(),
            ))
        }
    }
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct SignedUrlRequest {
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
