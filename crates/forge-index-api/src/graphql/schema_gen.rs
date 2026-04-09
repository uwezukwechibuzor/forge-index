//! Auto-generates an async-graphql schema from a forge-index-config Schema.

use async_graphql::dynamic::*;
use async_graphql::Value as GqlValue;
use forge_index_config::{ColumnType, Schema};
use sqlx::PgPool;

use crate::graphql::resolver;
use crate::graphql::types::{to_camel_case, to_pascal_case};

/// Holds the dynamically generated async-graphql schema.
pub struct GraphqlSchema {
    /// The built async-graphql schema.
    pub schema: async_graphql::dynamic::Schema,
}

/// Gets a field from a GqlValue::Object parent.
fn get_parent_field(ctx: &ResolverContext, field: &str) -> Option<GqlValue> {
    let parent = ctx.parent_value.try_downcast_ref::<GqlValue>().ok()?;
    if let GqlValue::Object(map) = parent {
        map.get(field).cloned()
    } else {
        None
    }
}

/// Creates a simple passthrough field that reads from parent object.
fn passthrough_field(name: &str, type_ref: TypeRef) -> Field {
    let field_name = name.to_string();
    Field::new(name, type_ref, move |ctx| {
        let field_name = field_name.clone();
        FieldFuture::new(async move { Ok(get_parent_field(&ctx, &field_name)) })
    })
}

impl GraphqlSchema {
    /// Builds a GraphQL schema from the user's data schema.
    pub fn build(schema: &Schema, pool: PgPool, pg_schema: String) -> Result<Self, String> {
        let mut query = Object::new("Query");

        // Shared types
        let order_dir_enum = Enum::new("OrderDirection")
            .item(EnumItem::new("ASC"))
            .item(EnumItem::new("DESC"));

        let string_filter = InputObject::new("StringFilter")
            .field(InputValue::new("eq", TypeRef::named(TypeRef::STRING)))
            .field(InputValue::new("contains", TypeRef::named(TypeRef::STRING)))
            .field(InputValue::new("in", TypeRef::named_list(TypeRef::STRING)))
            .field(InputValue::new(
                "notIn",
                TypeRef::named_list(TypeRef::STRING),
            ));

        let int_filter = InputObject::new("IntFilter")
            .field(InputValue::new("eq", TypeRef::named(TypeRef::INT)))
            .field(InputValue::new("gt", TypeRef::named(TypeRef::INT)))
            .field(InputValue::new("gte", TypeRef::named(TypeRef::INT)))
            .field(InputValue::new("lt", TypeRef::named(TypeRef::INT)))
            .field(InputValue::new("lte", TypeRef::named(TypeRef::INT)));

        let bool_filter = InputObject::new("BoolFilter")
            .field(InputValue::new("eq", TypeRef::named(TypeRef::BOOLEAN)));

        let page_info = Object::new("PageInfo")
            .field(passthrough_field(
                "hasNextPage",
                TypeRef::named_nn(TypeRef::BOOLEAN),
            ))
            .field(passthrough_field(
                "hasPreviousPage",
                TypeRef::named_nn(TypeRef::BOOLEAN),
            ))
            .field(passthrough_field(
                "startCursor",
                TypeRef::named(TypeRef::STRING),
            ))
            .field(passthrough_field(
                "endCursor",
                TypeRef::named(TypeRef::STRING),
            ));

        let mut schema_builder = async_graphql::dynamic::Schema::build("Query", None, None)
            .register(page_info)
            .register(order_dir_enum)
            .register(string_filter)
            .register(int_filter)
            .register(bool_filter);

        // Generate per-table types and queries
        for table in &schema.tables {
            let type_name = to_pascal_case(&table.name);
            let page_type_name = format!("{}Page", type_name);
            let filter_type_name = format!("{}Filter", type_name);
            let order_by_name = format!("{}OrderBy", type_name);

            // Find PK column
            let pk_col = table
                .columns
                .iter()
                .find(|c| c.primary_key)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "id".to_string());
            let columns: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();

            // 1. Object type
            let mut obj = Object::new(&type_name);
            for col in &table.columns {
                obj = obj.field(passthrough_field(
                    &col.name,
                    col_type_ref(&col.col_type, col.nullable),
                ));
            }
            schema_builder = schema_builder.register(obj);

            // 2. Page type
            let items_tn = type_name.clone();
            let page_obj = Object::new(&page_type_name)
                .field(passthrough_field(
                    "items",
                    TypeRef::named_nn_list_nn(&items_tn),
                ))
                .field(passthrough_field("pageInfo", TypeRef::named_nn("PageInfo")))
                .field(passthrough_field(
                    "totalCount",
                    TypeRef::named_nn(TypeRef::INT),
                ));
            schema_builder = schema_builder.register(page_obj);

            // 3. Filter input type
            let mut fi = InputObject::new(&filter_type_name);
            for col in &table.columns {
                let ft = match col.col_type {
                    ColumnType::Boolean => "BoolFilter",
                    ColumnType::Int | ColumnType::Float => "IntFilter",
                    _ => "StringFilter",
                };
                fi = fi.field(InputValue::new(&col.name, TypeRef::named(ft)));
            }
            schema_builder = schema_builder.register(fi);

            // 4. OrderBy enum
            let mut obe = Enum::new(&order_by_name);
            for col in &table.columns {
                obe = obe.item(EnumItem::new(&col.name));
            }
            schema_builder = schema_builder.register(obe);

            // 5. Single record query
            {
                let p = pool.clone();
                let ps = pg_schema.clone();
                let t = table.name.clone();
                let pk = pk_col.clone();
                let cols = columns.clone();

                let field = Field::new(
                    to_camel_case(&table.name),
                    TypeRef::named(&type_name),
                    move |ctx| {
                        let p = p.clone();
                        let ps = ps.clone();
                        let t = t.clone();
                        let pk = pk.clone();
                        let cols = cols.clone();
                        FieldFuture::new(async move {
                            let pk_value: String = ctx.args.try_get(&pk)?.string()?.to_string();
                            let result =
                                resolver::resolve_single(&p, &ps, &t, &pk, &pk_value, &cols)
                                    .await
                                    .map_err(async_graphql::Error::new)?;
                            Ok(result.map(json_to_gql_value))
                        })
                    },
                )
                .argument(InputValue::new(&pk_col, TypeRef::named_nn(TypeRef::STRING)));

                query = query.field(field);
            }

            // 6. List query
            {
                let p = pool.clone();
                let ps = pg_schema.clone();
                let t = table.name.clone();
                let pk = pk_col.clone();
                let cols = columns.clone();

                let field = Field::new(
                    format!("{}s", to_camel_case(&table.name)),
                    TypeRef::named_nn(&page_type_name),
                    move |ctx| {
                        let p = p.clone();
                        let ps = ps.clone();
                        let t = t.clone();
                        let pk = pk.clone();
                        let cols = cols.clone();
                        FieldFuture::new(async move {
                            let filter_val = ctx
                                .args
                                .get("where")
                                .map(|v| gql_value_to_json(v.as_value()));
                            let order_by = ctx
                                .args
                                .get("orderBy")
                                .and_then(|v| v.enum_name().ok().map(String::from));
                            let order_dir = ctx
                                .args
                                .get("orderDirection")
                                .and_then(|v| v.enum_name().ok().map(String::from));
                            let limit = ctx.args.get("limit").and_then(|v| v.i64().ok());
                            let after = ctx
                                .args
                                .get("after")
                                .and_then(|v| v.string().ok().map(String::from));
                            let before = ctx
                                .args
                                .get("before")
                                .and_then(|v| v.string().ok().map(String::from));

                            let result = resolver::resolve_list(
                                &p,
                                &ps,
                                &t,
                                &pk,
                                &cols,
                                filter_val.as_ref(),
                                order_by.as_deref(),
                                order_dir.as_deref(),
                                limit,
                                after.as_deref(),
                                before.as_deref(),
                            )
                            .await
                            .map_err(async_graphql::Error::new)?;

                            Ok(Some(json_to_gql_value(result)))
                        })
                    },
                )
                .argument(InputValue::new("where", TypeRef::named(&filter_type_name)))
                .argument(InputValue::new("orderBy", TypeRef::named(&order_by_name)))
                .argument(InputValue::new(
                    "orderDirection",
                    TypeRef::named("OrderDirection"),
                ))
                .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
                .argument(InputValue::new("after", TypeRef::named(TypeRef::STRING)))
                .argument(InputValue::new("before", TypeRef::named(TypeRef::STRING)));

                query = query.field(field);
            }
        }

        // Placeholder if no tables
        if schema.tables.is_empty() {
            query = query.field(Field::new(
                "_empty",
                TypeRef::named(TypeRef::STRING),
                |_| FieldFuture::new(async { Ok(Some(GqlValue::from("no tables defined"))) }),
            ));
        }

        schema_builder = schema_builder.register(query);

        let gql_schema = schema_builder
            .finish()
            .map_err(|e| format!("Failed to build GraphQL schema: {}", e))?;

        Ok(Self { schema: gql_schema })
    }
}

fn col_type_ref(col_type: &ColumnType, nullable: bool) -> TypeRef {
    let base = match col_type {
        ColumnType::Boolean => TypeRef::BOOLEAN,
        ColumnType::Int => TypeRef::INT,
        ColumnType::Float => TypeRef::FLOAT,
        _ => TypeRef::STRING,
    };
    if nullable {
        TypeRef::named(base)
    } else {
        TypeRef::named_nn(base)
    }
}

/// Converts serde_json::Value to async_graphql::Value.
pub fn json_to_gql_value(v: serde_json::Value) -> GqlValue {
    match v {
        serde_json::Value::Null => GqlValue::Null,
        serde_json::Value::Bool(b) => GqlValue::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                GqlValue::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                GqlValue::Number(async_graphql::Number::from_f64(f).unwrap_or_else(|| 0i64.into()))
            } else {
                GqlValue::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => GqlValue::String(s),
        serde_json::Value::Array(arr) => {
            GqlValue::List(arr.into_iter().map(json_to_gql_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: async_graphql::indexmap::IndexMap<async_graphql::Name, GqlValue> = obj
                .into_iter()
                .map(|(k, v)| (async_graphql::Name::new(k), json_to_gql_value(v)))
                .collect();
            GqlValue::Object(map)
        }
    }
}

/// Converts async_graphql::Value to serde_json::Value.
pub fn gql_value_to_json(v: &GqlValue) -> serde_json::Value {
    match v {
        GqlValue::Null => serde_json::Value::Null,
        GqlValue::Enum(e) => serde_json::Value::String(e.to_string()),
        GqlValue::String(s) => serde_json::Value::String(s.clone()),
        GqlValue::Boolean(b) => serde_json::Value::Bool(*b),
        GqlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::json!(i)
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::String(n.to_string())
            }
        }
        GqlValue::List(arr) => {
            serde_json::Value::Array(arr.iter().map(gql_value_to_json).collect())
        }
        GqlValue::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.to_string(), gql_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        GqlValue::Binary(b) => serde_json::Value::String(hex::encode(b)),
    }
}
