//! SQL-over-HTTP: validation, execution, and result types.

pub mod executor;
pub mod parser;
pub mod result;

pub use executor::execute_sql;
pub use parser::{validate_sql, ValidatedSql};
pub use result::SqlResult;

use std::fmt;

/// Errors from SQL validation or execution.
#[derive(Debug)]
pub enum SqlError {
    /// The SQL statement is not a SELECT.
    InvalidStatement(String),
    /// A forbidden keyword was found (e.g. DROP, DELETE).
    ForbiddenKeyword(String),
    /// The query exceeds the maximum allowed length.
    TooLong { len: usize, max: usize },
    /// The query timed out.
    Timeout,
    /// A database error occurred.
    Database(sqlx::Error),
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlError::InvalidStatement(msg) => write!(f, "{}", msg),
            SqlError::ForbiddenKeyword(kw) => write!(f, "Forbidden keyword: {}", kw),
            SqlError::TooLong { len, max } => {
                write!(f, "Query too long: {} chars (max {})", len, max)
            }
            SqlError::Timeout => write!(f, "Query execution timed out"),
            SqlError::Database(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for SqlError {}
