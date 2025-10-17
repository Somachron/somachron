pub mod res {
    use ser_mapper::impl_dto;
    use utoipa::ToSchema;

    use crate::{
        datastore::{
            space::Space,
            user_space::{SpaceRole, SpaceUser, UserSpace},
        },
        dto::{
            Datetime, _IdRef,
            user::res::{UserResponse, _UserResponseRef},
        },
    };

    impl_dto!(
        #[derive(ToSchema)]
        pub struct SpaceResponse<Space> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            name: String = name,
            description: String = description,
            picture_url: String = picture_url,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct UserSpaceResponse<UserSpace> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            role: SpaceRole = role,
            space: SpaceResponse = space => _SpaceResponseRef,
            folder: String = root_folder,
        }
    );

    impl_dto!(
        #[derive(ToSchema)]
        pub struct SpaceUserResponse<SpaceUser> {
            id: String = id => _IdRef,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,

            role: SpaceRole = role,
            user: UserResponse = user => _UserResponseRef,
        }
    );
}

pub mod req {
    use serde::Deserialize;
    use utoipa::ToSchema;
    use validator::Validate;

    use crate::{datastore::user_space::SpaceRole, dto::DtoUuid};

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct SpaceCreateRequest {
        #[validate(length(min = 3, max = 255))]
        pub name: String,
        pub description: String,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct SpaceMemberRequest {
        pub user_id: DtoUuid,
    }

    #[derive(Deserialize, ToSchema, Validate)]
    pub struct UpdateSpaceMemberRoleRequest {
        pub user_id: DtoUuid,

        pub role: SpaceRole,
    }
}
