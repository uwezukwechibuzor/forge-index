//! Context construction for user handlers.

pub use forge_index_db::context::DbContext;
use forge_index_db::WriteBuffer;
use sqlx::PgPool;
use std::sync::Arc;

/// Creates a `DbContext` for use inside event handlers.
#[allow(dead_code)]
pub(crate) fn make_context(buffer: Arc<WriteBuffer>, pool: PgPool, pg_schema: &str) -> DbContext {
    DbContext::new(buffer, pool, pg_schema.to_string())
}
