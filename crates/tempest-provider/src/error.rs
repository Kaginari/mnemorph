//! What can go wrong with a model call, and whether trying again could help.

use std::fmt;

/// Which wait ran out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// No byte of the body arrived in time.
    FirstByte,
    /// The body stopped mid-stream.
    Stall,
    /// The call's whole deadline passed.
    Turn,
}

/// A failed model call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The request was refused before it was sent; sending it could not have worked.
    Invalid(String),
    /// The provider answered with an error status.
    Api {
        /// The status code.
        status: u16,
        /// The provider's error type (`rate_limit_error`, `overloaded_error`, …).
        kind: String,
        /// Its message.
        message: String,
    },
    /// The connection failed.
    Transport(String),
    /// A wait ran out.
    Timeout(Wait),
    /// The provider reported an error inside the stream.
    Stream {
        /// The provider's error type.
        kind: String,
        /// Its message.
        message: String,
    },
    /// The stream ended without its final event: the answer is incomplete.
    Truncated,
    /// The stream could not be read.
    Decode(String),
}

impl Error {
    /// Whether another attempt could succeed — before any content was shown.
    #[must_use]
    pub fn retryable(&self) -> bool {
        match self {
            Error::Api { status, .. } => crate::retry::retryable(*status),
            Error::Transport(_)
            | Error::Timeout(Wait::FirstByte | Wait::Stall)
            | Error::Truncated => true,
            Error::Stream { kind, .. } => {
                matches!(
                    kind.as_str(),
                    "overloaded_error" | "api_error" | "rate_limit_error" | "finish_reason_error"
                ) || kind.parse::<u16>().is_ok_and(crate::retry::retryable)
            }
            Error::Invalid(_) | Error::Decode(_) | Error::Timeout(Wait::Turn) => false,
        }
    }
}

impl Error {
    /// Whether the provider says the model does not exist here — the cue for a fallback chain
    /// to try its next model, never a retry of the same one.
    #[must_use]
    pub fn model_not_found(&self) -> bool {
        match self {
            Error::Api {
                status: 404,
                kind,
                message,
            } => {
                kind == "not_found_error"
                    || kind == "model_not_found"
                    || message.to_ascii_lowercase().contains("model")
            }
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Invalid(why) => write!(f, "request refused before sending: {why}"),
            Error::Api {
                status,
                kind,
                message,
            } => write!(f, "{status} {kind}: {message}"),
            Error::Transport(why) => write!(f, "connection failed: {why}"),
            Error::Timeout(Wait::FirstByte) => f.write_str("no answer began in time"),
            Error::Timeout(Wait::Stall) => f.write_str("the answer stalled mid-stream"),
            Error::Timeout(Wait::Turn) => f.write_str("the call's deadline passed"),
            Error::Stream { kind, message } => write!(f, "error in the stream: {kind}: {message}"),
            Error::Truncated => f.write_str("the stream ended before its last event"),
            Error::Decode(why) => write!(f, "the stream could not be read: {why}"),
        }
    }
}

impl std::error::Error for Error {}
