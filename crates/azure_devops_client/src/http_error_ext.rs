//! Extension trait that classifies an `azure_core::Error` into `AzureError`.
//!
//! Callers replace `.map_err(|e| AzureError::Http(e.to_string()))` with
//! `.map_err(AzureErrorExt::into_azure)` and get a structured `HttpError` back.

use azure_core::Error as SdkError;
use azure_core::error::ErrorKind;

use crate::AzureError;
use crate::http_error::HttpError;

pub trait AzureErrorExt {
    fn into_azure(self) -> AzureError;
}

impl AzureErrorExt for SdkError {
    fn into_azure(self) -> AzureError {
        let kind = self.kind().clone();
        let display = self.to_string();
        let http = match kind {
            ErrorKind::HttpResponse { status, .. } => match u16::from(status) {
                404 => HttpError::NotFound,
                409 => HttpError::Conflict,
                _ => HttpError::Transport(format!("HTTP {status}: {display}")),
            },
            ErrorKind::DataConversion => HttpError::Decode(display),
            ErrorKind::Connection | ErrorKind::Io => HttpError::Transport(display),
            ErrorKind::Credential => HttpError::Transport(format!("credential: {display}")),
            ErrorKind::Other => HttpError::Transport(display),
        };
        AzureError::Http(http)
    }
}
