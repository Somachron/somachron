pub mod res {
    use ser_mapper::impl_dto;
    use utoipa::ToSchema;

    use crate::{datastore::user::User, dto::Datetime};

    impl_dto!(
        #[derive(ToSchema)]
        pub struct UserResponse<User> {
            id: String = id,
            created_at: Datetime = created_at,
            updated_at: Datetime = updated_at,
            given_name: String = given_name,
            email: String = email,
            picture_url: String = picture_url,
        }
    );
}
