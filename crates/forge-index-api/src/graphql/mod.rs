//! Auto-generated GraphQL API from user schema.

pub mod filters;
pub mod handler;
pub mod pagination;
pub mod resolver;
pub mod schema_gen;
pub mod types;

pub use handler::{graphql_handler, graphql_playground, GraphqlState};
pub use schema_gen::GraphqlSchema;
