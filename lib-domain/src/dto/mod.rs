use chrono::{DateTime, Utc};
use ser_mapper::impl_dto;
use serde::Serialize;
use surrealdb::RecordId;
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

impl_dto!(@define_dto
    pub struct Id<RecordId> {
        __pad: u64,
    }
);

impl IdSerializer for RecordId {
    fn dto_serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let id = self.key().to_string();
        serializer.serialize_str(&id)
    }
}

impl ToSchema for Id {}

impl PartialSchema for Id {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        String::schema()
    }
}
