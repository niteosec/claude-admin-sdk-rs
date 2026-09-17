//! Standard Webhooks signature verification.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit as _, Mac as _};
use http::HeaderMap;
use sha2::Sha256;

use crate::request::HookRequest;

type HmacSha256 = Hmac<Sha256>;

/// `webhook-id`: unique per delivery, equal to the body's `request_id`.
pub const WEBHOOK_ID: &str = "webhook-id";
/// `webhook-timestamp`: Unix seconds, as a decimal string, when the request was signed.
pub const WEBHOOK_TIMESTAMP: &str = "webhook-timestamp";
/// `webhook-signature`: one or more space-separated `v1,<base64>` values.
pub const WEBHOOK_SIGNATURE: &str = "webhook-signature";

/// The documented timestamp tolerance: reject a timestamp more than five minutes from your clock,
/// in either direction.
pub const DEFAULT_TOLERANCE_SECONDS: u64 = 300;

const SECRET_PREFIX: &str = "whsec_";
const SIGNATURE_LEN: usize = 32;

/// A signing secret, decoded to key bytes. `Debug` never shows the key.
#[derive(Clone)]
pub struct SigningSecret {
    key: Vec<u8>,
}

impl SigningSecret {
    /// Parses the secret as revealed in claude.ai: `whsec_` followed by **standard** base64
    /// (`+` and `/`, padded). The prefix is optional. A URL-safe spelling (`-`, `_`) is rejected
    /// rather than silently decoded to the wrong key.
    pub fn parse(secret: &str) -> Result<Self, InvalidSecretError> {
        let encoded = secret.strip_prefix(SECRET_PREFIX).unwrap_or(secret);
        let key = STANDARD.decode(encoded).map_err(|_| InvalidSecretError::NotStandardBase64)?;
        if key.is_empty() {
            return Err(InvalidSecretError::Empty);
        }
        Ok(Self { key })
    }

    /// Computes the `v1,<base64>` signature for a delivery, for tests and local tooling.
    pub fn sign(&self, webhook_id: &str, webhook_timestamp: &str, body: &[u8]) -> String {
        let tag = self.mac(webhook_id, webhook_timestamp, body).finalize().into_bytes();
        format!("v1,{}", STANDARD.encode(tag))
    }

    fn mac(&self, webhook_id: &str, webhook_timestamp: &str, body: &[u8]) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC accepts keys of any length");
        mac.update(webhook_id.as_bytes());
        mac.update(b".");
        mac.update(webhook_timestamp.as_bytes());
        mac.update(b".");
        mac.update(body);
        mac
    }
}

impl fmt::Debug for SigningSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningSecret(<redacted>)")
    }
}

impl FromStr for SigningSecret {
    type Err = InvalidSecretError;

    fn from_str(secret: &str) -> Result<Self, Self::Err> {
        Self::parse(secret)
    }
}

/// A signing secret could not be decoded. Never contains the secret.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidSecretError {
    /// The value after `whsec_` is not canonical standard-alphabet, padded base64.
    #[error("signing secret is not standard base64 after the `whsec_` prefix")]
    NotStandardBase64,
    /// The secret decodes to zero bytes.
    #[error("signing secret is empty")]
    Empty,
}

/// Why a delivery failed verification. Reject the request in every case, except that
/// [`VerifyError::Unsigned`] may be tolerated for an organization that enabled Inference hooks
/// before a secret was required, only until its administrator confirms the secret exists.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    /// None of the three `webhook-*` headers is present.
    #[error("request is unsigned: no webhook-* headers")]
    Unsigned,
    /// A `webhook-*` header is absent or empty while another is present.
    #[error("missing `{0}` header")]
    MissingHeader(&'static str),
    /// A `webhook-*` header value is not visible ASCII.
    #[error("`{0}` header is not visible ASCII")]
    MalformedHeader(&'static str),
    /// `webhook-timestamp` is not a decimal integer.
    #[error("`webhook-timestamp` is not a decimal integer")]
    MalformedTimestamp,
    /// The timestamp is further in the past than the tolerance: a replay, or skewed clocks.
    #[error("webhook-timestamp {timestamp} is too old (now {now})")]
    TimestampTooOld {
        /// The header value.
        timestamp: i64,
        /// The time verification ran with.
        now: i64,
    },
    /// The timestamp is further in the future than the tolerance: skewed clocks.
    #[error("webhook-timestamp {timestamp} is in the future (now {now})")]
    TimestampTooNew {
        /// The header value.
        timestamp: i64,
        /// The time verification ran with.
        now: i64,
    },
    /// `webhook-signature` contains no well-formed `v1,<base64 of 32 bytes>` value.
    #[error("`webhook-signature` has no well-formed v1 signature")]
    MalformedSignature,
    /// No `v1` signature matches any configured secret.
    #[error("no signature matches")]
    NoMatchingSignature,
}

/// Why a delivery could not be accepted by [`Verifier::verify_request`].
#[derive(Debug, thiserror::Error)]
pub enum ReceiveError {
    /// The signature did not verify.
    #[error(transparent)]
    Verify(#[from] VerifyError),
    /// The signed body is not a valid request.
    #[error("invalid request body: {0}")]
    Body(#[from] serde_json::Error),
    /// The body's `request_id` differs from the `webhook-id` header, which the documentation says
    /// are equal.
    #[error("webhook-id {webhook_id:?} does not equal request_id {request_id:?}")]
    WebhookIdMismatch {
        /// The `webhook-id` header.
        webhook_id: String,
        /// The body's `request_id`.
        request_id: String,
    },
}

/// The verified headers of a delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedDelivery {
    /// `webhook-id`. Unique per delivery and reused by a connection-failure retry: use it as an
    /// idempotency key.
    pub webhook_id: String,
    /// `webhook-timestamp`, Unix seconds.
    pub timestamp: i64,
}

impl VerifiedDelivery {
    /// Checks the documented invariant that `webhook-id` equals the body's `request_id`. A body with
    /// no `request_id` (possible only for an unknown event type) passes.
    pub fn check_request_id(&self, request: &HookRequest) -> Result<(), ReceiveError> {
        match request.request_id() {
            Some(request_id) if request_id != self.webhook_id => Err(ReceiveError::WebhookIdMismatch {
                webhook_id: self.webhook_id.clone(),
                request_id: request_id.to_owned(),
            }),
            _ => Ok(()),
        }
    }
}

/// A verified, parsed delivery.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedRequest {
    /// The verified headers.
    pub delivery: VerifiedDelivery,
    /// The parsed body.
    pub request: HookRequest,
}

/// Verifies deliveries against one or more signing secrets.
///
/// Rotation is an immediate cutover, but requests signed with the previous secret can arrive for
/// about a minute afterwards, plus anything in flight. Add both secrets during the switchover
/// ([`Verifier::with_secret`]), then drop the old one.
#[derive(Debug, Clone)]
pub struct Verifier {
    secrets: Vec<SigningSecret>,
    tolerance_seconds: u64,
}

impl Verifier {
    /// A verifier for one secret, with the documented five-minute tolerance.
    pub fn new(secret: SigningSecret) -> Self {
        Self { secrets: vec![secret], tolerance_seconds: DEFAULT_TOLERANCE_SECONDS }
    }

    /// Also accepts signatures made with `secret`.
    pub fn with_secret(mut self, secret: SigningSecret) -> Self {
        self.secrets.push(secret);
        self
    }

    /// Overrides the timestamp tolerance. The documented value is [`DEFAULT_TOLERANCE_SECONDS`].
    pub fn with_tolerance_seconds(mut self, tolerance_seconds: u64) -> Self {
        self.tolerance_seconds = tolerance_seconds;
        self
    }

    /// Verifies the `webhook-*` headers against the raw body bytes, exactly as received, at `now`.
    ///
    /// Header lookup is case-insensitive (`HeaderMap` normalizes names). If a header is repeated,
    /// the first value is used.
    pub fn verify(&self, headers: &HeaderMap, body: &[u8], now: SystemTime) -> Result<VerifiedDelivery, VerifyError> {
        self.verify_at(headers, body, unix_seconds(now))
    }

    /// [`Verifier::verify`] with the current time given as Unix seconds.
    pub fn verify_at(
        &self,
        headers: &HeaderMap,
        body: &[u8],
        now_unix_seconds: i64,
    ) -> Result<VerifiedDelivery, VerifyError> {
        let names = [WEBHOOK_ID, WEBHOOK_TIMESTAMP, WEBHOOK_SIGNATURE];
        if names.iter().all(|name| !headers.contains_key(*name)) {
            return Err(VerifyError::Unsigned);
        }
        let [webhook_id, timestamp_text, signatures] = names.map(|name| header(headers, name));
        let (webhook_id, timestamp_text, signatures) = (webhook_id?, timestamp_text?, signatures?);

        let timestamp: i64 = timestamp_text.parse().map_err(|_| VerifyError::MalformedTimestamp)?;
        if now_unix_seconds.abs_diff(timestamp) > self.tolerance_seconds {
            let (timestamp, now) = (timestamp, now_unix_seconds);
            return Err(if timestamp < now {
                VerifyError::TimestampTooOld { timestamp, now }
            } else {
                VerifyError::TimestampTooNew { timestamp, now }
            });
        }

        let candidates: Vec<Vec<u8>> = signatures
            .split_ascii_whitespace()
            .filter_map(|value| value.strip_prefix("v1,"))
            .filter_map(|encoded| STANDARD.decode(encoded).ok())
            .filter(|tag| tag.len() == SIGNATURE_LEN)
            .collect();
        if candidates.is_empty() {
            return Err(VerifyError::MalformedSignature);
        }

        // Every secret against every candidate, without an early exit, so timing does not reveal
        // which comparison matched. Each comparison is constant time (`verify_slice`).
        let mut matched = false;
        for secret in &self.secrets {
            let mac = secret.mac(webhook_id, timestamp_text, body);
            for tag in &candidates {
                matched |= mac.clone().verify_slice(tag).is_ok();
            }
        }
        if !matched {
            return Err(VerifyError::NoMatchingSignature);
        }
        Ok(VerifiedDelivery { webhook_id: webhook_id.to_owned(), timestamp })
    }

    /// Verifies the delivery, parses the body, and checks that `webhook-id` equals `request_id`.
    pub fn verify_request(
        &self,
        headers: &HeaderMap,
        body: &[u8],
        now: SystemTime,
    ) -> Result<VerifiedRequest, ReceiveError> {
        self.verify_request_at(headers, body, unix_seconds(now))
    }

    /// [`Verifier::verify_request`] with the current time given as Unix seconds.
    pub fn verify_request_at(
        &self,
        headers: &HeaderMap,
        body: &[u8],
        now_unix_seconds: i64,
    ) -> Result<VerifiedRequest, ReceiveError> {
        let delivery = self.verify_at(headers, body, now_unix_seconds)?;
        let request = HookRequest::from_slice(body)?;
        delivery.check_request_id(&request)?;
        Ok(VerifiedRequest { delivery, request })
    }
}

fn header<'a>(headers: &'a HeaderMap, name: &'static str) -> Result<&'a str, VerifyError> {
    let value = headers.get(name).ok_or(VerifyError::MissingHeader(name))?;
    let value = value.to_str().map_err(|_| VerifyError::MalformedHeader(name))?;
    if value.trim().is_empty() {
        return Err(VerifyError::MissingHeader(name));
    }
    Ok(value)
}

fn unix_seconds(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |secs| -secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_key() {
        let secret = SigningSecret::parse("whsec_c2VjcmV0").unwrap();
        assert_eq!(format!("{secret:?}"), "SigningSecret(<redacted>)");
        assert!(!format!("{:?}", Verifier::new(secret)).contains("c2VjcmV0"));
    }

    #[test]
    fn prefix_is_optional_and_padding_is_required() {
        assert!(SigningSecret::parse("c2VjcmV0").is_ok());
        assert_eq!(SigningSecret::parse("whsec_c2VjcmV").unwrap_err(), InvalidSecretError::NotStandardBase64);
        assert_eq!(SigningSecret::parse("whsec_").unwrap_err(), InvalidSecretError::Empty);
    }

    #[test]
    fn unix_seconds_handles_both_sides_of_the_epoch() {
        use std::time::Duration;
        assert_eq!(unix_seconds(UNIX_EPOCH + Duration::from_secs(5)), 5);
        assert_eq!(unix_seconds(UNIX_EPOCH - Duration::from_secs(5)), -5);
    }
}
