pub mod res {
    use chrono::{DateTime, Utc};
    use ser_mapper::impl_dto;

    use crate::datastore::user::User;

    impl_dto!(
        pub struct UserResponse<User> {
            id: String = id,
            created_at: DateTime<Utc> = created_at,
            updated_at: DateTime<Utc> = updated_at,
            given_name: String = given_name,
            email: String = email,
            picture_url: String = picture_url,
        }
    );
}
