//! forge-index CLI — the `forge` binary.
//!
//! Provides `forge dev` (hot reload), `forge start` (production),
//! `forge codegen` (type generation), and `forge migrate` commands.

pub mod commands;
pub mod process;
pub mod watcher;

#[cfg(test)]
mod tests;
