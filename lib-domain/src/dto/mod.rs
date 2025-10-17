use chrono::{DateTime, Utc};
use ser_mapper::impl_dto;
use serde::{Deserialize, Serialize};
use utoipa::{
    openapi::{schema::SchemaType, KnownFormat, ObjectBuilder, RefOr, Schema, SchemaFormat, Type},
    PartialSchema, ToSchema,
};
use uuid::Uuid;

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
    pub struct Id<Uuid> {
        __pad: u64,
    }
);

impl IdSerializer for Uuid {
    fn dto_serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.serialize(serializer)
    }
}

impl ToSchema for Id {}

impl PartialSchema for Id {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        String::schema()
    }
}

#[derive(Serialize, Deserialize)]
#[repr(transparent)]
pub struct DtoUuid(pub Uuid);

impl ToSchema for DtoUuid {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::from("Uuid")
    }
}

impl PartialSchema for DtoUuid {
    fn schema() -> RefOr<Schema> {
        RefOr::T(Schema::Object(ObjectBuilder::new().schema_type(SchemaType::Type(Type::String)).build()))
    }
}
