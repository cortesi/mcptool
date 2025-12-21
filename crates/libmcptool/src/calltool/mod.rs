//! Tool argument parsing from different sources (command line, interactive, JSON).

/// Command line argument parsing.
pub mod cmdline;
/// Interactive argument prompting.
pub mod interactive;
/// JSON argument parsing from stdin.
pub mod json;
