//! Error types.

use std::fmt;
use std::time::Duration;

use http::StatusCode;
use serde::Deserialize;

/// Errors returned by clients built on this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API answered with a non-success status (after any retries).
    #[error(transparent)]
    Api(Box<ApiError>),
    /// Transport failure: connect, TLS, timeout, body read.
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    /// A success response whose body did not match the expected shape.
    #[error("failed to decode response body (request-id {request_id:?}): {source}")]
    Decode {
        /// The serde error.
        #[source]
        source: serde_json::Error,
        /// The `request-id` header, if present.
        request_id: Option<String>,
        /// The start of the body, for diagnosis.
        body_excerpt: String,
    },
    /// A request argument was rejected before sending.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// A URL could not be built.
    #[error(transparent)]
    Url(#[from] url::ParseError),
}

impl Error {
    /// The API error, when this is one.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(api) => Some(api),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Error::Api(Box::new(error))
    }
}

/// A non-success API response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// HTTP status.
    pub status: StatusCode,
    /// `error.type` from the body. Part of the API contract; match on it and on `status`.
    pub kind: ApiErrorKind,
    /// `error.message` from the body, or the raw body when it is not the standard envelope.
    /// Anthropic may reword messages; prefer `status` + `kind`.
    pub message: String,
    /// `request-id` header, falling back to `request_id` in the body. Quote it to Anthropic support.
    pub request_id: Option<String>,
    /// `retry-after`, in seconds.
    pub retry_after: Option<Duration>,
    /// `x-should-retry`. Every observed 4xx carries `false`.
    pub should_retry: Option<bool>,
}

impl ApiError {
    /// A 403 caused by the key lacking a scope ("Missing required scopes. Got: … Needed one of: …").
    ///
    /// Shares `permission_error` with [`Self::is_organization_out_of_scope`]; the two differ only by
    /// message, so this check is message-based.
    pub fn is_missing_scope(&self) -> bool {
        self.status == StatusCode::FORBIDDEN && self.message.starts_with("Missing required scopes")
    }

    /// A 403 caused by filtering on an organization the key does not cover ("Cannot query
    /// compliance data for organizations outside this API key's scope"). Message-based, like
    /// [`Self::is_missing_scope`].
    pub fn is_organization_out_of_scope(&self) -> bool {
        self.status == StatusCode::FORBIDDEN && self.message.contains("outside this API key's scope")
    }

    /// Whether the documented retry contract allows retrying this response.
    ///
    /// `x-should-retry` decides when present. Otherwise 429, 500, 502, 503, 504 and 529 are
    /// retryable and everything else is not.
    pub fn is_retryable(&self) -> bool {
        match self.should_retry {
            Some(decision) => decision,
            None => matches!(self.status.as_u16(), 429 | 500 | 502 | 503 | 504 | 529),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status.as_u16(), self.kind, self.message)?;
        if let Some(request_id) = &self.request_id {
            write!(f, " (request-id {request_id})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// `error.type` values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApiErrorKind {
    /// `invalid_request_error`
    InvalidRequest,
    /// `authentication_error`
    Authentication,
    /// `permission_error`
    Permission,
    /// `not_found_error`
    NotFound,
    /// `rate_limit_error`
    RateLimit,
    /// `api_error`
    Api,
    /// `overloaded_error`
    Overloaded,
    /// A value this crate does not know, or an empty string when the body was not the standard
    /// envelope.
    Other(String),
}

impl ApiErrorKind {
    fn parse(value: &str) -> Self {
        match value {
            "invalid_request_error" => Self::InvalidRequest,
            "authentication_error" => Self::Authentication,
            "permission_error" => Self::Permission,
            "not_found_error" => Self::NotFound,
            "rate_limit_error" => Self::RateLimit,
            "api_error" => Self::Api,
            "overloaded_error" => Self::Overloaded,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The wire value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalid_request_error",
            Self::Authentication => "authentication_error",
            Self::Permission => "permission_error",
            Self::NotFound => "not_found_error",
            Self::RateLimit => "rate_limit_error",
            Self::Api => "api_error",
            Self::Overloaded => "overloaded_error",
            Self::Other(other) => other,
        }
    }
}

impl fmt::Display for ApiErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `{"type":"error","error":{"type":…,"message":…,"details":…},"request_id":…}`. `details` and the
/// top-level `type` are ignored.
#[derive(Deserialize)]
struct Envelope {
    error: EnvelopeError,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Deserialize)]
struct EnvelopeError {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: String,
}

pub(crate) struct ErrorHeaders {
    pub(crate) request_id: Option<String>,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) should_retry: Option<bool>,
}

pub(crate) fn api_error(status: StatusCode, headers: ErrorHeaders, body: &[u8]) -> ApiError {
    let (kind, message, body_request_id) = match serde_json::from_slice::<Envelope>(body) {
        Ok(envelope) => (ApiErrorKind::parse(&envelope.error.kind), envelope.error.message, envelope.request_id),
        Err(_) => (ApiErrorKind::Other(String::new()), excerpt(body), None),
    };
    ApiError {
        status,
        kind,
        message,
        request_id: headers.request_id.or(body_request_id),
        retry_after: headers.retry_after,
        should_retry: headers.should_retry,
    }
}

pub(crate) fn excerpt(body: &[u8]) -> String {
    const MAX: usize = 512;
    let text = String::from_utf8_lossy(body);
    match text.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers() -> ErrorHeaders {
        ErrorHeaders { request_id: None, retry_after: None, should_retry: None }
    }

    #[test]
    fn parses_the_envelope_and_falls_back_to_the_body_request_id() {
        let body = br#"{"type":"error","error":{"type":"authentication_error","message":"API key is invalid."},"request_id":null}"#;
        let error = api_error(StatusCode::UNAUTHORIZED, headers(), body);
        assert_eq!(error.kind, ApiErrorKind::Authentication);
        assert_eq!(error.message, "API key is invalid.");
        assert_eq!(error.request_id, None);
        assert!(!error.is_retryable());
    }

    #[test]
    fn non_envelope_body_keeps_the_text() {
        let error = api_error(StatusCode::BAD_GATEWAY, headers(), b"<html>bad gateway</html>");
        assert_eq!(error.kind, ApiErrorKind::Other(String::new()));
        assert_eq!(error.message, "<html>bad gateway</html>");
        assert!(error.is_retryable());
    }

    #[test]
    fn should_retry_header_overrides_the_status_rule() {
        let mut error = api_error(StatusCode::INTERNAL_SERVER_ERROR, headers(), b"{}");
        assert!(error.is_retryable());
        error.should_retry = Some(false);
        assert!(!error.is_retryable());
    }
}
