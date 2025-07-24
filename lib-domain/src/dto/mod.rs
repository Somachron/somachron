use chrono::{DateTime, Utc};
use sonic_rs::Serialize;
use utoipa::{
    openapi::{schema::SchemaType, KnownFormat, ObjectBuilder, RefOr, Schema, SchemaFormat, Type},
    PartialSchema, ToSchema,
};

pub mod auth;
pub mod cloud;
pub mod space;
pub mod user;

#[derive(Serialize)]
pub struct Datetime(pub DateTime<Utc>);

impl ToSchema for Datetime {}

impl PartialSchema for Datetime {
    fn schema() -> RefOr<Schema> {
        RefOr::T(Schema::Object(
            ObjectBuilder::new()
                .schema_type(SchemaType::Type(Type::String))
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
                .build(),
        ))
    }
}
