//! Event and handler registry.

pub mod event_registry;
pub mod handler;
pub mod setup;

pub use event_registry::EventRegistry;
pub use handler::{HandlerFn, SetupHandlerFn};
