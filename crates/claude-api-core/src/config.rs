//! Credentials and client configuration.

use std::fmt;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{DEFAULT_ANTHROPIC_VERSION, DEFAULT_BASE_URL};

/// An Anthropic API key. The secret is never printed by `Debug`.
#[derive(Clone)]
pub struct ApiKey(SecretString);

impl ApiKey {
    /// Wraps a key string.
    pub fn new(key: impl Into<String>) -> Self {
        Self(SecretString::from(key.into()))
    }

    /// The key type, inferred from its prefix. Informational only: the server decides what a key
    /// can reach, and reports it as a 403.
    pub fn kind(&self) -> KeyKind {
        let key = self.0.expose_secret();
        if key.starts_with("sk-ant-admin01-") {
            KeyKind::Admin
        } else if key.starts_with("sk-ant-api01-") {
            KeyKind::Api
        } else {
            KeyKind::Unknown
        }
    }

    pub(crate) fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApiKey").field(&format_args!("<redacted {:?}>", self.kind())).finish()
    }
}

/// The key type, by prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// `sk-ant-admin01-…`: a Claude Console Admin API key. On the Compliance API it reaches the
    /// Activity Feed only.
    Admin,
    /// `sk-ant-api01-…`: a regular API key, or a Compliance Access Key / Analytics key created in
    /// claude.ai (they share the prefix; scopes differ).
    Api,
    /// Any other prefix.
    Unknown,
}

/// Retry behaviour for retryable failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt. `0` disables retrying.
    pub max_retries: u32,
    /// First backoff when the server gives no `retry-after`.
    pub initial_backoff: Duration,
    /// Backoff ceiling.
    pub max_backoff: Duration,
}

impl RetryPolicy {
    /// No retries.
    pub fn none() -> Self {
        Self { max_retries: 0, ..Self::default() }
    }

    pub(crate) fn backoff(&self, retry: u32) -> Duration {
        let factor = 2u32.saturating_pow(retry.min(16));
        self.initial_backoff.saturating_mul(factor).min(self.max_backoff)
    }
}

impl Default for RetryPolicy {
    /// Three retries, 1 s doubling to 60 s: the backoff Anthropic documents for the Compliance API.
    fn default() -> Self {
        Self { max_retries: 3, initial_backoff: Duration::from_secs(1), max_backoff: Duration::from_secs(60) }
    }
}

/// Client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub(crate) base_url: Url,
    pub(crate) anthropic_version: String,
    pub(crate) user_agent: String,
    pub(crate) timeout: Duration,
    pub(crate) retry: RetryPolicy,
}

impl ClientConfig {
    /// Overrides the API origin (tests, proxies).
    pub fn with_base_url(mut self, base_url: Url) -> Self {
        self.base_url = base_url;
        self
    }

    /// Overrides the `anthropic-version` header.
    pub fn with_anthropic_version(mut self, version: impl Into<String>) -> Self {
        self.anthropic_version = version.into();
        self
    }

    /// Overrides the `user-agent` header.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Per-request timeout. The default is generous because filtered Activity Feed queries were
    /// observed taking 3–5 s even on tiny result sets.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retry behaviour.
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: Url::parse(DEFAULT_BASE_URL).expect("default base URL is valid"),
            anthropic_version: DEFAULT_ANTHROPIC_VERSION.to_owned(),
            user_agent: concat!("claude-admin-sdk-rs/", env!("CARGO_PKG_VERSION")).to_owned(),
            timeout: Duration::from_secs(60),
            retry: RetryPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_kind_by_prefix() {
        assert_eq!(ApiKey::new("sk-ant-admin01-abc").kind(), KeyKind::Admin);
        assert_eq!(ApiKey::new("sk-ant-api01-abc").kind(), KeyKind::Api);
        assert_eq!(ApiKey::new("something").kind(), KeyKind::Unknown);
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let printed = format!("{:?}", ApiKey::new("sk-ant-admin01-supersecret"));
        assert!(!printed.contains("supersecret"), "{printed}");
        assert!(printed.contains("Admin"), "{printed}");
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.backoff(0), Duration::from_secs(1));
        assert_eq!(policy.backoff(1), Duration::from_secs(2));
        assert_eq!(policy.backoff(5), Duration::from_secs(32));
        assert_eq!(policy.backoff(6), Duration::from_secs(60));
        assert_eq!(policy.backoff(40), Duration::from_secs(60));
    }
}
