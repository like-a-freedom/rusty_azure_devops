//! Classifying wrapper that turns the coarse `azure_core::Error` enumeration
//! into a structured `HttpError` carried inside `AzureError::Http`.
//!
//! The variants of `HttpError` are `pub` so downstream callers and tests can
//! pattern-match on them (e.g. `Err(AzureError::Http(HttpError::Conflict))`),
//! but the only way to construct one is the crate-private `From<&azure_core::Error>`
//! impl — the classification policy (which `ErrorKind` maps to which variant,
//! which status codes are "interesting") stays inside `azure_devops_client`.

use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HttpError {
    /// Generic transport failure (connect, IO, credential, "other") that is
    /// typically safe to retry.
    #[error("{0}")]
    Transport(String),
    /// Retries exhausted. Carries the last error message.
    #[error("timeout after retries: {0}")]
    Timeout(String),
    /// HTTP 404.
    #[error("not found")]
    NotFound,
    /// HTTP 409 — revision conflict. Should not be retried.
    #[error("conflict (rev mismatch?)")]
    Conflict,
    /// The Azure SDK failed to parse the response body. On on-prem servers this
    /// is the heuristic that triggers the empty-project retry in the client.
    #[error("{0}")]
    Decode(String),
    /// On-prem HTTP fallback exhausted every (api-version × auth-variant)
    /// combination; the `Vec` collects the per-attempt errors for diagnostics.
    #[error("fallback attempts failed: {}", errors.join("; "))]
    Fallback { errors: Vec<String> },
}

impl HttpError {
    /// Construct from a generic message — used by `AzureErrorExt` and a few
    /// in-crate sites that produce a `Transport`-style error without going
    /// through `azure_core` (e.g. `reqwest::Client::builder().build()`).
    pub fn transport(msg: impl ToString) -> Self {
        Self::Transport(msg.to_string())
    }

    /// Construct from a generic decode message — used inside `client.rs`
    /// for the heuristic-covered SDK parse errors that previously went through
    /// `is_heuristic_sdk_error`.
    pub fn decode(msg: impl ToString) -> Self {
        Self::Decode(msg.to_string())
    }

    /// Construct a Fallback from the collected per-attempt errors.
    pub fn fallback(errors: Vec<String>) -> Self {
        Self::Fallback { errors }
    }
}
