//! Unofficial Rust types and signature verification for Anthropic's Claude inference hooks.
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! *Built from Anthropic's documentation (fetched 2026-09-17); not yet verified against live deliveries.*
//!
//! Inference hooks (beta, Claude Enterprise) route every governed prompt through an **AI security
//! server** you operate. Before inference runs, Anthropic sends a signed HTTPS `POST` carrying the
//! conversation transcript; your server answers with an allow or deny verdict, and a denied request
//! never reaches the model. One hook covers claude.ai, Cowork and Claude Code; field names, request
//! shapes and headers may change during the beta.
//!
//! This crate is the receiving side, with no HTTP server framework: hand it the raw body bytes and
//! an [`http::HeaderMap`], get back a verified [`HookRequest`], and answer with a [`Verdict`].
//!
//! ```
//! use claude_inference_hooks::{HookRequest, SigningSecret, Verdict, Verifier};
//! use http::HeaderMap;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let secret: SigningSecret = "whsec_c2VjcmV0LWJ5dGVz".parse()?;
//! let verifier = Verifier::new(secret.clone());
//!
//! // What your server received (built here so the example runs).
//! let body = br#"{"type":"prompt","request_id":"req_1","actor":{"type":"user"},
//!     "source":{"application":"claude-ai"},"messages":[]}"#;
//! let now = 1_789_000_000;
//! let mut headers = HeaderMap::new();
//! headers.insert("webhook-id", "req_1".parse()?);
//! headers.insert("webhook-timestamp", now.to_string().parse()?);
//! headers.insert("webhook-signature", secret.sign("req_1", &now.to_string(), body).parse()?);
//!
//! // In a server, pass `std::time::SystemTime::now()` to `verify_request` instead.
//! let verified = verifier.verify_request_at(&headers, body, now)?;
//! let verdict = match &verified.request {
//!     HookRequest::Prompt(frame) if frame.messages.is_empty() => Verdict::allow(),
//!     HookRequest::Prompt(_) => Verdict::deny("Remove the payment card number and try again."),
//!     // Unknown event types get an allow, not an error status.
//!     HookRequest::Other { .. } => Verdict::allow(),
//! };
//! let response = verdict.to_http_response(); // 200, application/json
//! assert_eq!(response.body(), br#"{"action":"allow"}"#);
//! # Ok(())
//! # }
//! ```
//!
//! # Operational contract
//!
//! - **Signatures.** Once the organization has a signing secret every request is signed, including
//!   connection tests; reject unsigned requests ([`VerifyError::Unsigned`]). An organization that
//!   enabled hooks before secrets were required sends unsigned requests until an administrator
//!   generates one. During secret rotation, accept both secrets for the switchover.
//! - **Verdicts.** Always HTTP 200 with a JSON verdict. Anything else — another status, a redirect,
//!   an unparseable body, an `action` other than `allow`/`deny`, a body over 64 KiB, a timeout — is a
//!   *webhook failure*, never a deny. Failure handling (fail closed: **Block the request**; fail
//!   open: **Allow the request**) then decides, and sustained failures trip a circuit breaker.
//! - **Timeout.** Administrator-set, 1–10,000 ms (default 5,000 ms), covering connection, TLS,
//!   request and response.
//! - **Retries.** Exactly once, after 100 ms, and only when the connection attempt fails; the retry
//!   carries the same `webhook-id` and signature. Deduplicate on `webhook-id`.
//! - **Shadow mode.** Requests and verdicts flow as when enforcing, but nothing is blocked. The
//!   server cannot tell from the request.
//! - **`config-test`.** **Test connection** and circuit-breaker recovery checks (at most about once
//!   a minute, starting 10 minutes after a trip) send a signed synthetic request with
//!   `source.application` = [`Application::ConfigTest`]. Answer normally; any valid verdict resets
//!   the breaker.
//! - **Body size.** Bodies are untruncated: usually under about 10 MB, up to
//!   [`MAX_REQUEST_BODY_BYTES`] (64 MiB). A body your server rejects for size is a webhook failure.
//! - **Hosting.** `https://` on port 443, publicly routable, publicly trusted certificate, no
//!   redirects, no reverse tunnels. Requests come from [`SOURCE_IP_RANGE`]; allowlisting narrows
//!   exposure but does not replace signature verification.
//! - **Forward compatibility.** Unknown top-level fields, `metadata` keys, `source.application`
//!   values, `actor.type` values and block types parse into open types here and must never cause a
//!   rejection.

mod request;
mod signature;
mod string_enum;
mod verdict;

pub use request::{
    Actor, Application, AttachmentBlock, ContentBlock, HookRequest, Message, PromptFrame, Role, Source, TextBlock,
    ToolResultBlock, ToolUseBlock, UserActor,
};
pub use signature::{
    DEFAULT_TOLERANCE_SECONDS, InvalidSecretError, ReceiveError, SigningSecret, VerifiedDelivery, VerifiedRequest,
    Verifier, VerifyError, WEBHOOK_ID, WEBHOOK_SIGNATURE, WEBHOOK_TIMESTAMP,
};
pub use verdict::{
    Deny, MAX_DENY_REASON_CHARS, MAX_REFERENCE_ID_CHARS, MAX_VERDICT_BODY_BYTES, VERDICT_CONTENT_TYPE, VERDICT_STATUS,
    Verdict, is_valid_reference_id,
};

/// The largest request body the protocol allows: 64 MiB.
pub const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024 * 1024;

/// The `User-Agent` Anthropic sends.
pub const USER_AGENT: &str = "anthropic-dlp/1";

/// The block requests originate from, part of Anthropic's published outbound IP ranges.
pub const SOURCE_IP_RANGE: &str = "160.79.106.0/24";
