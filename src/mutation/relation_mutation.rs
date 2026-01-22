use async_graphql::dynamic::{InputObject, InputValue, ObjectAccessor, TypeRef};
use heck::ToLowerCamelCase;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter,
};

use crate::{BuilderContext, SeaResult};

#[derive(Debug, Clone, Default)]
pub struct RelationOperations {
    pub connect: Vec<String>,
    pub disconnect: Vec<String>,
    pub set: Option<Vec<String>>,
}

impl RelationOperations {
    pub fn has_operations(&self) -> bool {
        !self.connect.is_empty() || !self.disconnect.is_empty() || self.set.is_some()
    }
}

pub fn parse_relation_operations(
    context: &'static BuilderContext,
    accessor: &ObjectAccessor,
) -> SeaResult<RelationOperations> {
    let mut ops = RelationOperations::default();

    if let Some(connect_value) = accessor.get(&context.relation_input.connect_field) {
        if let Ok(list) = connect_value.list() {
            for item in list.iter() {
                if let Ok(id) = item.string() {
                    ops.connect.push(id.to_string());
                }
            }
        }
    }

    if let Some(disconnect_value) = accessor.get(&context.relation_input.disconnect_field) {
        if let Ok(list) = disconnect_value.list() {
            for item in list.iter() {
                if let Ok(id) = item.string() {
                    ops.disconnect.push(id.to_string());
                }
            }
        }
    }

    if let Some(set_value) = accessor.get(&context.relation_input.set_field) {
        if let Ok(list) = set_value.list() {
            let mut ids = Vec::new();
            for item in list.iter() {
                if let Ok(id) = item.string() {
                    ids.push(id.to_string());
                }
            }
            ops.set = Some(ids);
        }
    }

    Ok(ops)
}

pub fn parse_id_to_value(id: &str) -> sea_orm::Value {
    if let Ok(int_id) = id.parse::<i32>() {
        return sea_orm::Value::Int(Some(int_id));
    }
    if let Ok(int_id) = id.parse::<i64>() {
        return sea_orm::Value::BigInt(Some(int_id));
    }
    sea_orm::Value::String(Some(id.to_string().into()))
}

pub type RelationProcessorFn = Box<
    dyn for<'a> Fn(
            &'static BuilderContext,
            &'a sea_orm::DatabaseConnection,
            sea_orm::Value,
            &'a ObjectAccessor<'a>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SeaResult<()>> + Send + 'a>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct M2MRelationDef {
    pub field_name: String,
    pub input_type_name: String,
}

impl M2MRelationDef {
    pub fn new(field_name: impl Into<String>, entity_name: &str) -> Self {
        let field_name = field_name.into();
        let input_type_name = format!(
            "{}{}RelationInput",
            entity_name,
            field_name.to_lower_camel_case()
        );
        Self {
            field_name,
            input_type_name,
        }
    }

    pub fn to_input_object(&self, context: &'static BuilderContext) -> InputObject {
        InputObject::new(&self.input_type_name)
            .field(InputValue::new(
                &context.relation_input.connect_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
            .field(InputValue::new(
                &context.relation_input.disconnect_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
            .field(InputValue::new(
                &context.relation_input.set_field,
                TypeRef::named_list(TypeRef::STRING),
            ))
    }
}

pub trait M2MRelations: EntityTrait {
    fn m2m_relations(_context: &'static BuilderContext) -> Vec<M2MRelationDef> {
        Vec::new()
    }

    fn m2m_processors(
        _context: &'static BuilderContext,
    ) -> std::collections::HashMap<String, RelationProcessorFn> {
        std::collections::HashMap::new()
    }
}

pub struct M2MRelationProcessors<E>
where
    E: EntityTrait,
{
    pub processors: std::collections::HashMap<String, RelationProcessorFn>,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> Default for M2MRelationProcessors<E>
where
    E: EntityTrait,
{
    fn default() -> Self {
        Self {
            processors: std::collections::HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<E> M2MRelationProcessors<E>
where
    E: EntityTrait + M2MRelations,
{
    pub fn from_entity(context: &'static BuilderContext) -> Self {
        Self {
            processors: E::m2m_processors(context),
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn process_relations(
        &self,
        context: &'static BuilderContext,
        db: &sea_orm::DatabaseConnection,
        source_id: sea_orm::Value,
        input_object: &ObjectAccessor<'_>,
    ) -> SeaResult<()> {
        for (field_name, processor) in &self.processors {
            if let Some(relation_input) = input_object.get(field_name) {
                if let Ok(relation_obj) = relation_input.object() {
                    processor(context, db, source_id.clone(), &relation_obj).await?;
                }
            }
        }
        Ok(())
    }
}

pub fn generate_m2m_relation_inputs<E>(context: &'static BuilderContext) -> Vec<InputObject>
where
    E: EntityTrait + M2MRelations,
{
    E::m2m_relations(context)
        .into_iter()
        .map(|rel| rel.to_input_object(context))
        .collect()
}

pub fn add_m2m_fields_to_input<E>(
    context: &'static BuilderContext,
    input: InputObject,
) -> InputObject
where
    E: EntityTrait + M2MRelations,
{
    E::m2m_relations(context)
        .into_iter()
        .fold(input, |input, rel| {
            input.field(InputValue::new(&rel.field_name, TypeRef::named(&rel.input_type_name)))
        })
}

pub struct RelationMutationProcessor {
    pub context: &'static BuilderContext,
}

impl RelationMutationProcessor {
    pub fn new(context: &'static BuilderContext) -> Self {
        Self { context }
    }

    pub async fn process_m2m_relation<J, JA>(
        &self,
        db: &impl ConnectionTrait,
        source_id: sea_orm::Value,
        source_column: J::Column,
        related_column: J::Column,
        operations: RelationOperations,
    ) -> SeaResult<()>
    where
        J: EntityTrait,
        JA: ActiveModelTrait<Entity = J> + sea_orm::ActiveModelBehavior + Default + Send,
        <J as EntityTrait>::Model: IntoActiveModel<JA>,
        J::Column: ColumnTrait,
    {
        if let Some(set_ids) = operations.set {
            J::delete_many()
                .filter(source_column.eq(source_id.clone()))
                .exec(db)
                .await?;

            for related_id in set_ids {
                let related_value = parse_id_to_value(&related_id);
                self.create_junction_row::<J, JA>(
                    db,
                    source_column,
                    source_id.clone(),
                    related_column,
                    related_value,
                )
                .await?;
            }
        } else {
            for related_id in operations.connect {
                let related_value = parse_id_to_value(&related_id);
                let existing = J::find()
                    .filter(source_column.eq(source_id.clone()))
                    .filter(related_column.eq(related_value.clone()))
                    .one(db)
                    .await?;

                if existing.is_none() {
                    self.create_junction_row::<J, JA>(
                        db,
                        source_column,
                        source_id.clone(),
                        related_column,
                        related_value,
                    )
                    .await?;
                }
            }

            for related_id in operations.disconnect {
                let related_value = parse_id_to_value(&related_id);
                J::delete_many()
                    .filter(source_column.eq(source_id.clone()))
                    .filter(related_column.eq(related_value))
                    .exec(db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn create_junction_row<J, JA>(
        &self,
        db: &impl ConnectionTrait,
        source_column: J::Column,
        source_value: sea_orm::Value,
        related_column: J::Column,
        related_value: sea_orm::Value,
    ) -> SeaResult<()>
    where
        J: EntityTrait,
        JA: ActiveModelTrait<Entity = J> + sea_orm::ActiveModelBehavior + Default + Send,
        <J as EntityTrait>::Model: IntoActiveModel<JA>,
    {
        let mut active_model = <JA as Default>::default();
        active_model.try_set(source_column, source_value)?;
        active_model.try_set(related_column, related_value)?;
        active_model.insert(db).await?;
        Ok(())
    }
}
