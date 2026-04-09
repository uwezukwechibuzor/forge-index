//! Realtime sync engine — processes live blocks via WebSocket subscription.

pub mod finality;
pub mod processor;
pub mod subscriber;

pub use finality::FinalityTracker;
pub use processor::RealtimeProcessor;
pub use subscriber::NewBlockSubscriber;
