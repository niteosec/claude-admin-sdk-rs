//! Shared transport for unofficial Rust clients of Anthropic's organization-level APIs.
//!
//! The Compliance API, the Claude Enterprise Analytics API and the Admin API all live on
//! `https://api.anthropic.com`, authenticate with an `x-api-key` header, return errors in the same
//! envelope and report the same `anthropic-ratelimit-*` headers. This crate owns that shared
//! behaviour so the per-API crates only describe endpoints and records.
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! What it does for every request:
//!
//! - sends `x-api-key`, `anthropic-version` and a `user-agent`;
//! - throttles before sending when the last response reported an exhausted request budget;
//! - retries 429, 5xx and transport failures with the documented contract: honour `retry-after`,
//!   otherwise exponential backoff from 1 s doubling to 60 s, and never retry when the server says
//!   `x-should-retry: false`;
//! - maps failures to [`ApiError`], keeping the `request-id` for support escalation.

mod client;
mod config;
mod error;
mod pagination;
mod response;

pub use client::ApiClient;
pub use config::{ApiKey, ClientConfig, KeyKind, RetryPolicy};
pub use error::{ApiError, ApiErrorKind, Error};
pub use pagination::{Cursor, CursorPage};
pub use response::{ApiResponse, RateLimit, ResponseMeta};

/// Production API origin.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// The `anthropic-version` header value sent by default.
pub const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Result alias for this crate and the API crates built on it.
pub type Result<T, E = Error> = std::result::Result<T, E>;
