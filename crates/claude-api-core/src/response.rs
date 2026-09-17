//! Response metadata.

use http::HeaderMap;
use jiff::Timestamp;

/// A decoded body and the headers that matter.
#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    /// The decoded body.
    pub body: T,
    /// Response metadata.
    pub meta: ResponseMeta,
}

/// Headers worth keeping from every response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseMeta {
    /// `request-id`. On the Compliance API this equals the `request_id` of the
    /// `compliance_api_accessed` activity the call itself generates, so a consumer can recognise its
    /// own reads in the Activity Feed.
    pub request_id: Option<String>,
    /// `anthropic-organization-id`: the organization UUID the key belongs to.
    pub organization_id: Option<String>,
    /// The shared request budget. Not guaranteed on every response (one 403 was observed without it).
    pub rate_limit: Option<RateLimit>,
}

/// `anthropic-ratelimit-requests-*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    /// Requests per window.
    pub limit: u32,
    /// Requests left in the current window.
    pub remaining: u32,
    /// When the full budget is restored.
    pub reset: Option<Timestamp>,
}

impl ResponseMeta {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            request_id: header(headers, "request-id").map(str::to_owned),
            organization_id: header(headers, "anthropic-organization-id").map(str::to_owned),
            rate_limit: RateLimit::from_headers(headers),
        }
    }
}

impl RateLimit {
    fn from_headers(headers: &HeaderMap) -> Option<Self> {
        Some(Self {
            limit: header(headers, "anthropic-ratelimit-requests-limit")?.parse().ok()?,
            remaining: header(headers, "anthropic-ratelimit-requests-remaining")?.parse().ok()?,
            reset: header(headers, "anthropic-ratelimit-requests-reset").and_then(|value| value.parse().ok()),
        })
    }
}

pub(crate) fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn reads_observed_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("request-id", HeaderValue::from_static("req_011Cexample"));
        headers.insert("anthropic-organization-id", HeaderValue::from_static("00000000-0000-4000-8000-000000000001"));
        headers.insert("anthropic-ratelimit-requests-limit", HeaderValue::from_static("600"));
        headers.insert("anthropic-ratelimit-requests-remaining", HeaderValue::from_static("599"));
        headers.insert("anthropic-ratelimit-requests-reset", HeaderValue::from_static("2026-09-17T05:49:58Z"));

        let meta = ResponseMeta::from_headers(&headers);
        assert_eq!(meta.request_id.as_deref(), Some("req_011Cexample"));
        let rate = meta.rate_limit.expect("rate limit");
        assert_eq!((rate.limit, rate.remaining), (600, 599));
        assert_eq!(rate.reset, Some("2026-09-17T05:49:58Z".parse().unwrap()));
    }

    #[test]
    fn missing_rate_limit_headers_are_none() {
        assert_eq!(ResponseMeta::from_headers(&HeaderMap::new()).rate_limit, None);
    }
}
