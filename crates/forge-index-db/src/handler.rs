//! Handler function traits that receive `DbContext`.
//!
//! These are the production handler traits — handlers receive a real `DbContext`
//! to read/write the database, instead of a placeholder `serde_json::Value`.

use crate::context::DbContext;
use forge_index_core::abi::decoder::DecodedEvent;
use std::future::Future;
use std::pin::Pin;

/// A handler function that processes a decoded event with database access.
pub trait EventHandlerFn: Send + Sync + 'static {
    /// Invokes the handler with the decoded event and database context.
    fn call(
        &self,
        event: DecodedEvent,
        ctx: DbContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;
}

/// Blanket implementation for async functions/closures.
impl<F, Fut> EventHandlerFn for F
where
    F: Fn(DecodedEvent, DbContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), anyhow::Error>> + Send + 'static,
{
    fn call(
        &self,
        event: DecodedEvent,
        ctx: DbContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> {
        Box::pin((self)(event, ctx))
    }
}

/// A setup handler that runs once before indexing with database access.
pub trait SetupEventHandlerFn: Send + Sync + 'static {
    /// Invokes the setup handler with database context.
    fn call(
        &self,
        ctx: DbContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;
}

/// Blanket implementation for async functions/closures.
impl<F, Fut> SetupEventHandlerFn for F
where
    F: Fn(DbContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), anyhow::Error>> + Send + 'static,
{
    fn call(
        &self,
        ctx: DbContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> {
        Box::pin((self)(ctx))
    }
}
