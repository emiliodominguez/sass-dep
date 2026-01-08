//! Parser error types.
//!
//! This module defines error types that can occur during parsing.

use thiserror::Error;

use super::Location;

/// Error type for parsing failures.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Failed to parse a directive.
    #[error("Failed to parse directive at line {line}, column {column}: {message}")]
    InvalidDirective {
        /// Line number where the error occurred.
        line: usize,
        /// Column number where the error occurred.
        column: usize,
        /// Description of the error.
        message: String,
    },

    /// Unterminated string literal.
    #[error("Unterminated string at line {line}, column {column}")]
    UnterminatedString {
        /// Line number where the string started.
        line: usize,
        /// Column number where the string started.
        column: usize,
    },

    /// Unexpected end of input.
    #[error("Unexpected end of input at line {line}, column {column}")]
    UnexpectedEof {
        /// Line number where input ended.
        line: usize,
        /// Column number where input ended.
        column: usize,
    },

    /// IO error while reading file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl ParseError {
    /// Creates a new InvalidDirective error.
    pub fn invalid_directive(location: &Location, message: impl Into<String>) -> Self {
        Self::InvalidDirective {
            line: location.line,
            column: location.column,
            message: message.into(),
        }
    }

    /// Creates a new UnterminatedString error.
    pub fn unterminated_string(location: &Location) -> Self {
        Self::UnterminatedString {
            line: location.line,
            column: location.column,
        }
    }

    /// Creates a new UnexpectedEof error.
    pub fn unexpected_eof(location: &Location) -> Self {
        Self::UnexpectedEof {
            line: location.line,
            column: location.column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_display() {
        let err = ParseError::InvalidDirective {
            line: 10,
            column: 5,
            message: "expected string".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to parse directive at line 10, column 5: expected string"
        );
    }

    #[test]
    fn unterminated_string_display() {
        let err = ParseError::UnterminatedString { line: 1, column: 1 };
        assert_eq!(err.to_string(), "Unterminated string at line 1, column 1");
    }
}
