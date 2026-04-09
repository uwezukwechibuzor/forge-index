//! Handler function trait and storage.

use crate::abi::decoder::DecodedEvent;
use std::future::Future;
use std::pin::Pin;

/// A boxed handler function that processes a decoded event.
///
/// Handlers receive a `DecodedEvent` and a `serde_json::Value` context
/// (to avoid coupling with `forge-index-db`'s `DbContext` at the core level).
pub trait HandlerFn: Send + Sync + 'static {
    /// Invokes the handler with the given event and context.
    fn call(
        &self,
        event: DecodedEvent,
        ctx: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;
}

/// Blanket implementation for async closures/functions.
impl<F, Fut> HandlerFn for F
where
    F: Fn(DecodedEvent, serde_json::Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), anyhow::Error>> + Send + 'static,
{
    fn call(
        &self,
        event: DecodedEvent,
        ctx: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> {
        Box::pin((self)(event, ctx))
    }
}

/// A setup handler function that runs once before indexing begins.
pub trait SetupHandlerFn: Send + Sync + 'static {
    /// Invokes the setup handler.
    fn call(
        &self,
        ctx: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>>;
}

/// Blanket implementation for async closures/functions.
impl<F, Fut> SetupHandlerFn for F
where
    F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), anyhow::Error>> + Send + 'static,
{
    fn call(
        &self,
        ctx: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send>> {
        Box::pin((self)(ctx))
    }
}
