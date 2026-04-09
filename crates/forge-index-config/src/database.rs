//! Database connection configuration.

use serde::{Deserialize, Serialize};

/// Database backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseConfig {
    /// PostgreSQL database.
    Postgres {
        /// The connection string (e.g., `postgres://user:pass@host:port/db`).
        connection_string: String,
        /// The PostgreSQL schema to use.
        schema: String,
        /// Maximum number of connections in the pool.
        pool_max_connections: u32,
    },
    /// Embedded PGlite database (file-based).
    PGlite {
        /// The directory to store the PGlite data files.
        directory: String,
    },
}

impl DatabaseConfig {
    /// Creates a Postgres config with default schema ("public") and pool size (10).
    pub fn postgres(connection_string: impl Into<String>) -> Self {
        Self::Postgres {
            connection_string: connection_string.into(),
            schema: "public".to_string(),
            pool_max_connections: 10,
        }
    }

    /// Creates a PGlite config with the given directory.
    pub fn pglite(directory: impl Into<String>) -> Self {
        Self::PGlite {
            directory: directory.into(),
        }
    }
}
