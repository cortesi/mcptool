//! Error types for mcptool.

use std::io;

use rustyline::error::ReadlineError;
use thiserror::Error;

use crate::storage::StorageError;

use std::result;

/// Type alias for Results using our Error type.
pub type Result<T> = result::Result<T, Error>;

/// The main error type for mcptool operations.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O errors from file operations, network operations, etc.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization errors.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Storage-related errors.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// MCP protocol or connection errors.
    #[error("MCP error: {0}")]
    Other(String),

    /// Readline errors from rustyline.
    #[error("Readline error: {0}")]
    Readline(#[from] ReadlineError),

    /// MCP client errors.
    #[error("MCP client error: {0}")]
    MpcClient(#[from] tmcp::Error),

    /// Format errors for invalid user input.
    #[error("Invalid format: {0}")]
    Format(String),

    /// Errors that should be rare, and are not expected to be handled by the user.
    #[error("MCP error: {0}")]
    Internal(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}
