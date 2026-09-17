//! The HTTP client.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use jiff::Timestamp;
use serde::de::DeserializeOwned;
use tracing::{debug, warn};
use url::Url;

use crate::config::{ApiKey, ClientConfig, KeyKind};
use crate::download::Download;
use crate::error::{ErrorHeaders, api_error, excerpt};
use crate::path::ApiPath;
use crate::response::{ApiResponse, RateLimit, ResponseMeta, header};
use crate::{Error, Result};

/// A cheap-to-clone client bound to one API key.
///
/// Clones share the connection pool and the rate-limit state. Anthropic's Compliance API budget is
/// shared per parent organization across every key and endpoint, so share one client per key rather
/// than creating one per task.
#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    key: ApiKey,
    config: Arc<ClientConfig>,
    rate_limit: Arc<Mutex<Option<RateLimit>>>,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient").field("key", &self.key).field("config", &self.config).finish_non_exhaustive()
    }
}

impl ApiClient {
    /// A client with the default configuration.
    pub fn new(key: ApiKey) -> Result<Self> {
        Self::with_config(key, ClientConfig::default())
    }

    /// A client with a custom configuration.
    pub fn with_config(key: ApiKey, config: ClientConfig) -> Result<Self> {
        let http = reqwest::Client::builder().timeout(config.timeout).user_agent(config.user_agent.clone()).build()?;
        Ok(Self::with_http_client(key, config, http))
    }

    /// A client over a caller-supplied `reqwest::Client` (proxies, custom TLS). The client's own
    /// timeout and user agent apply; the ones in `config` are not re-applied.
    pub fn with_http_client(key: ApiKey, config: ClientConfig, http: reqwest::Client) -> Self {
        Self { http, key, config: Arc::new(config), rate_limit: Arc::new(Mutex::new(None)) }
    }

    /// The key type, by prefix.
    pub fn key_kind(&self) -> KeyKind {
        self.key.kind()
    }

    /// The most recent rate-limit state reported by the server, if any.
    pub fn last_rate_limit(&self) -> Option<RateLimit> {
        *self.rate_limit.lock().expect("rate-limit lock poisoned")
    }

    /// `GET path?query`, decoded as JSON, with throttling and retries.
    ///
    /// Query keys are sent verbatim, so array parameters keep their literal brackets
    /// (`activity_types[]`) and nested parameters their dots (`created_at.gte`); values are
    /// percent-encoded.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &ApiPath,
        query: &[(&str, String)],
    ) -> Result<ApiResponse<T>> {
        let (response, meta) = self.get(path, query, "application/json").await?;
        let body = response.bytes().await?;
        match serde_json::from_slice(&body) {
            Ok(body) => Ok(ApiResponse { body, meta }),
            Err(source) => Err(Error::Decode { source, request_id: meta.request_id, body_excerpt: excerpt(&body) }),
        }
    }

    /// `GET path?query` for a binary body, with throttling and retries up to the response headers.
    pub async fn get_download(&self, path: &ApiPath, query: &[(&str, String)]) -> Result<Download> {
        let (response, meta) = self.get(path, query, "*/*").await?;
        Ok(Download::new(meta, response))
    }

    /// Sends a GET and returns the first successful response, retrying per the policy.
    async fn get(
        &self,
        path: &ApiPath,
        query: &[(&str, String)],
        accept: &'static str,
    ) -> Result<(reqwest::Response, ResponseMeta)> {
        let url = self.url(path, query)?;
        let retry = &self.config.retry;
        let mut attempt = 0u32;
        loop {
            self.throttle().await;
            let outcome = self
                .http
                .get(url.clone())
                .header("x-api-key", self.key.expose())
                .header("anthropic-version", &self.config.anthropic_version)
                .header(reqwest::header::ACCEPT, accept)
                .send()
                .await;

            let response = match outcome {
                Ok(response) => response,
                Err(error) if attempt < retry.max_retries && (error.is_timeout() || error.is_connect()) => {
                    let delay = retry.backoff(attempt);
                    warn!(%url, attempt, ?delay, %error, "transport failure, retrying");
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            let meta = ResponseMeta::from_headers(response.headers());
            if let Some(rate_limit) = meta.rate_limit {
                *self.rate_limit.lock().expect("rate-limit lock poisoned") = Some(rate_limit);
            }
            let status = response.status();
            if status.is_success() {
                return Ok((response, meta));
            }

            let headers = ErrorHeaders {
                request_id: meta.request_id.clone(),
                retry_after: header(response.headers(), "retry-after")
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .map(Duration::from_secs),
                should_retry: header(response.headers(), "x-should-retry").and_then(|value| value.parse::<bool>().ok()),
            };
            let body = response.bytes().await?;
            let error = api_error(status, headers, &body);
            if attempt < retry.max_retries && error.is_retryable() {
                let delay = error.retry_after.unwrap_or_else(|| retry.backoff(attempt));
                warn!(%url, attempt, ?delay, %error, "retryable API error, retrying");
                tokio::time::sleep(delay).await;
                attempt += 1;
                continue;
            }
            return Err(error.into());
        }
    }

    fn url(&self, path: &ApiPath, query: &[(&str, String)]) -> Result<Url> {
        let mut url = self.config.base_url.clone();
        url.path_segments_mut()
            .map_err(|()| Error::InvalidArgument(format!("base URL {} cannot carry a path", self.config.base_url)))?
            .pop_if_empty()
            .extend(path.segments());
        if !query.is_empty() {
            let encoded: Vec<String> = query
                .iter()
                .map(|(key, value)| {
                    format!("{key}={}", url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>())
                })
                .collect();
            url.set_query(Some(&encoded.join("&")));
        }
        Ok(url)
    }

    /// Waits for the window reset when the last response reported no requests left.
    async fn throttle(&self) {
        let Some(RateLimit { remaining: 0, reset: Some(reset), .. }) = self.last_rate_limit() else {
            return;
        };
        let wait = reset.duration_since(Timestamp::now());
        if wait.is_positive() {
            let wait = Duration::try_from(wait).unwrap_or_default().min(self.config.retry.max_backoff);
            debug!(?wait, "request budget exhausted, waiting for the window to reset");
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> ApiClient {
        ApiClient::new(ApiKey::new("sk-ant-api01-test")).unwrap()
    }

    #[test]
    fn query_keys_stay_literal_and_values_are_encoded() {
        let url = client()
            .url(
                &ApiPath::new("v1/compliance/activities"),
                &[
                    ("activity_types[]", "claude_chat_created".into()),
                    ("created_at.gte", "2026-09-17T05:40:00+02:00".into()),
                ],
            )
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.anthropic.com/v1/compliance/activities?activity_types[]=claude_chat_created&created_at.gte=2026-09-17T05%3A40%3A00%2B02%3A00"
        );
    }

    #[test]
    fn identifiers_are_one_encoded_segment() {
        let path = ApiPath::new("v1/compliance/apps/chats").id("a/../b?c#d").unwrap().then("messages");
        let url = client().url(&path, &[]).unwrap();
        assert_eq!(url.as_str(), "https://api.anthropic.com/v1/compliance/apps/chats/a%2F..%2Fb%3Fc%23d/messages");
    }

    #[test]
    fn dot_segments_and_empty_identifiers_are_rejected() {
        for id in ["", ".", ".."] {
            assert!(ApiPath::new("v1/x").id(id).is_err(), "{id:?}");
        }
    }

    #[test]
    fn a_base_url_with_a_path_prefix_keeps_it() {
        let config = ClientConfig::default().with_base_url("https://proxy.example/anthropic/".parse().unwrap());
        let client = ApiClient::with_config(ApiKey::new("k"), config).unwrap();
        let url = client.url(&ApiPath::new("v1/compliance/activities"), &[]).unwrap();
        assert_eq!(url.as_str(), "https://proxy.example/anthropic/v1/compliance/activities");
    }
}
