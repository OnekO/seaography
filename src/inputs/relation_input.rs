use async_graphql::dynamic::{InputObject, InputValue, TypeRef};
use heck::{ToLowerCamelCase, ToSnakeCase};
use sea_orm::EntityTrait;

use crate::{BuilderContext, EntityObjectBuilder};

pub struct RelationInputConfig {
    pub type_suffix: String,
    pub connect_field: String,
    pub disconnect_field: String,
    pub set_field: String,
}

impl std::default::Default for RelationInputConfig {
    fn default() -> Self {
        RelationInputConfig {
            type_suffix: "RelationsInput".into(),
            connect_field: "connect".into(),
            disconnect_field: "disconnect".into(),
            set_field: "set".into(),
        }
    }
}

pub struct RelationInputBuilder {
    pub context: &'static BuilderContext,
}

impl RelationInputBuilder {
    pub fn type_name<T>(&self) -> String
    where
        T: EntityTrait,
    {
        let entity_object_builder = EntityObjectBuilder {
            context: self.context,
        };
        let object_name = entity_object_builder.type_name::<T>();
        format!("{}{}", object_name, self.context.relation_input.type_suffix)
    }

    pub fn relation_field_type_name<T, R>(&self, relation_name: &str) -> String
    where
        T: EntityTrait,
        R: EntityTrait,
    {
        let entity_object_builder = EntityObjectBuilder {
            context: self.context,
        };
        let object_name = entity_object_builder.type_name::<T>();
        let field_name = if cfg!(feature = "field-snake-case") {
            relation_name.to_snake_case()
        } else {
            relation_name.to_lower_camel_case()
        };
        format!("{}{}RelationInput", object_name, field_name.to_lower_camel_case())
    }

    pub fn relation_field_input_object<R>(&self, type_name: &str) -> InputObject
    where
        R: EntityTrait,
    {
        InputObject::new(type_name)
            .field(InputValue::new(
                &self.context.relation_input.connect_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
            .field(InputValue::new(
                &self.context.relation_input.disconnect_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
            .field(InputValue::new(
                &self.context.relation_input.set_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
    }

    #[allow(dead_code)]
    fn get_primary_key_graphql_type<T>(&self) -> &'static str
    where
        T: EntityTrait,
    {
        TypeRef::STRING
    }
}

