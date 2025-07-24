pub mod res {
    use ser_mapper::impl_dto;
    use utoipa::ToSchema;

    use crate::{
        datastore::{space::Space, user_space::UserSpace, SpaceRole},
        dto::Datetime,
    };

    impl_dto!(
        #[derive(ToSchema)]
        pub struct SpaceResponse<Space> {
            id: String = id,
            created_at: Datetime = created_at,
            updated_at: Datetime = created_at,

            name: String = name,
            description: String = description,
            picture_url: String = picture_url,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct UserSpaceResponse<UserSpace> {
            id: String = id,
            name: String = name,
            description: String = description,
            picture_url: String = picture_url,
            role: SpaceRole = role,
        }
    );
}

pub mod req {
    use sonic_rs::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct SpaceCreateRequest {
        #[validate(length(min = 3, max = 255))]
        pub name: String,
        pub description: String,
    }
}
