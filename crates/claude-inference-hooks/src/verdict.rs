//! The verdict your AI security server returns.

use serde::{Deserialize, Serialize};

/// The status every verdict must be returned with. Any other status, redirects included, is a
/// webhook failure, never a deny.
pub const VERDICT_STATUS: http::StatusCode = http::StatusCode::OK;

/// The verdict `Content-Type`.
pub const VERDICT_CONTENT_TYPE: &str = "application/json";

/// Anthropic reads at most this many bytes (64 KiB) of the response body, which must be
/// uncompressed.
pub const MAX_VERDICT_BODY_BYTES: usize = 64 * 1024;

/// `deny_reason` longer than this many characters is truncated by Anthropic.
pub const MAX_DENY_REASON_CHARS: usize = 500;

/// `reference_id` longer than this many characters is dropped by Anthropic.
pub const MAX_REFERENCE_ID_CHARS: usize = 50;

/// An allow or deny verdict, discriminated by `action`.
///
/// Serializes to exactly the documented shapes: `{"action":"allow"}`, and for a deny the
/// `deny_reason` and `reference_id` fields when set. Deserializing ignores unknown fields, as
/// Anthropic does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Verdict {
    /// `allow`: inference proceeds.
    Allow,
    /// `deny`: the request is rejected.
    Deny(Deny),
}

impl Verdict {
    /// An allow verdict.
    pub fn allow() -> Self {
        Verdict::Allow
    }

    /// A deny verdict with a reason for the end user.
    pub fn deny(reason: impl Into<String>) -> Self {
        Verdict::Deny(Deny::new().with_reason(reason))
    }

    /// The JSON body.
    pub fn to_json(&self) -> Vec<u8> {
        // A unit variant or a struct of optional strings always serializes.
        serde_json::to_vec(self).expect("a verdict always serializes")
    }

    /// A complete response: status 200, `Content-Type: application/json`, the JSON body.
    pub fn to_http_response(&self) -> http::Response<Vec<u8>> {
        let mut response = http::Response::new(self.to_json());
        *response.status_mut() = VERDICT_STATUS;
        response.headers_mut().insert(http::header::CONTENT_TYPE, http::HeaderValue::from_static(VERDICT_CONTENT_TYPE));
        response
    }
}

/// The body of a deny verdict. Anthropic never discards a deny over formatting: an oversize
/// `deny_reason` is truncated and a malformed `reference_id` is silently dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deny {
    /// Shown to the end user, followed by a blank line and the organization's custom blocked
    /// prompt message. At most [`MAX_DENY_REASON_CHARS`] characters are kept. Write it for the user:
    /// say what to change rather than emitting a scanner code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_reason: Option<String>,
    /// Your own identifier for this evaluation, recorded on the `inference_hooks_request_denied`
    /// compliance activity and never shown to the user. Keep it opaque: no request content, no
    /// personal data. See [`is_valid_reference_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl Deny {
    /// A deny with neither field set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `deny_reason`.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.deny_reason = Some(reason.into());
        self
    }

    /// Sets `reference_id`. Not validated here; Anthropic drops a malformed value, so check with
    /// [`is_valid_reference_id`] if it matters.
    pub fn with_reference_id(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }
}

impl From<Deny> for Verdict {
    fn from(deny: Deny) -> Self {
        Verdict::Deny(deny)
    }
}

/// Whether `reference_id` meets the documented constraint: at most 50 characters from
/// `[A-Za-z0-9._:/-]`.
///
/// The page does not say whether an empty string is valid; this returns `false` for it.
pub fn is_valid_reference_id(reference_id: &str) -> bool {
    !reference_id.is_empty()
        && reference_id.len() <= MAX_REFERENCE_ID_CHARS
        && reference_id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'/' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_id_rules() {
        assert!(is_valid_reference_id("scan_01HXPT4R9V"));
        assert!(is_valid_reference_id("a.b_c:d/e-f"));
        assert!(is_valid_reference_id(&"x".repeat(50)));
        assert!(!is_valid_reference_id(&"x".repeat(51)));
        assert!(!is_valid_reference_id("has space"));
        assert!(!is_valid_reference_id("é"));
        assert!(!is_valid_reference_id(""));
    }

    #[test]
    fn http_response_shape() {
        let response = Verdict::allow().to_http_response();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()[http::header::CONTENT_TYPE], "application/json");
        assert_eq!(response.body(), br#"{"action":"allow"}"#);
    }
}
