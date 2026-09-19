//! MCP tunnels and their certificates:
//!
//! - `GET /v1/tunnels`
//! - `GET /v1/tunnels/{tunnel_id}`
//! - `GET /v1/tunnels/{tunnel_id}/certificates`
//! - `GET /v1/tunnels/{tunnel_id}/certificates/{certificate_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! A tunnel routes MCP server URLs under its Anthropic-assigned domain to servers inside a private
//! network. The Tunnels API is a research preview: every request adds the [`MCP_TUNNELS_BETA`]
//! header, on top of the Managed Agents beta. The connector token that runs a tunnel is only
//! returned by the reveal and rotate endpoints, which this read-only client does not call.

use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{TokenList, token_list_methods};

const PATH: &str = "v1/tunnels";
const MAX_LIMIT: u32 = 1000;

/// The `anthropic-beta` value the Tunnels API requires.
pub const MCP_TUNNELS_BETA: &str = "mcp-tunnels-2026-06-22";

/// The stream returned by [`ListTunnels::stream`].
pub type TunnelStream = Pin<Box<dyn Stream<Item = Result<Tunnel>> + Send + 'static>>;

/// The stream returned by [`ListTunnelCertificates::stream`].
pub type TunnelCertificateStream = Pin<Box<dyn Stream<Item = Result<TunnelCertificate>> + Send + 'static>>;

/// An MCP tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tunnel {
    /// Always `tunnel`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `tnl_…` ID.
    pub id: String,
    /// When the tunnel was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the tunnel was created.
    pub created_at: Timestamp,
    /// Human-readable name (1 to 255 characters); `null` when unset.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Anthropic-assigned hostname. MCP server URLs whose host is a subdomain of it are routed
    /// through the tunnel. Never reused, even after archiving.
    pub domain: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A CA certificate registered on a tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelCertificate {
    /// Always `tunnel_certificate`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `tcrt_…` ID.
    pub id: String,
    /// When the certificate was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the certificate was registered.
    pub created_at: Timestamp,
    /// When the certificate expires.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// Lowercase hex SHA-256 fingerprint of the DER encoding.
    pub fingerprint: String,
    /// The tunnel the certificate is registered on.
    pub tunnel_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/tunnels`. Newest first.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListTunnels {
    inner: TokenList,
}

impl ListTunnels {
    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(Tunnel, TokenPage<Tunnel>, TunnelStream, "Page size, 1 to 1000 (server default 20).");
}

/// Request builder for `GET /v1/tunnels/{tunnel_id}/certificates`. Newest first.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListTunnelCertificates {
    inner: TokenList,
}

impl ListTunnelCertificates {
    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(
        TunnelCertificate,
        TokenPage<TunnelCertificate>,
        TunnelCertificateStream,
        "Page size, 1 to 1000 (server default 20)."
    );
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/tunnels`. Adds the [`MCP_TUNNELS_BETA`] header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/tunnels/list)
    pub fn tunnels(&self) -> ListTunnels {
        ListTunnels { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), Some(MAX_LIMIT)).beta(MCP_TUNNELS_BETA) }
    }

    /// `GET /v1/tunnels/{tunnel_id}`. Adds the [`MCP_TUNNELS_BETA`] header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/tunnels/retrieve)
    pub async fn tunnel(&self, tunnel_id: &str) -> Result<ApiResponse<Tunnel>> {
        let path = ApiPath::new(PATH).id(tunnel_id)?;
        self.api.get_json_with(&path, &[], &self.options().beta(MCP_TUNNELS_BETA)).await
    }

    /// `GET /v1/tunnels/{tunnel_id}/certificates`. Adds the [`MCP_TUNNELS_BETA`] header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/tunnels/certificates/list)
    pub fn tunnel_certificates(&self, tunnel_id: &str) -> ListTunnelCertificates {
        let path = ApiPath::new(PATH).id(tunnel_id).map(|path| path.then("certificates"));
        ListTunnelCertificates { inner: TokenList::new(self, path, Some(MAX_LIMIT)).beta(MCP_TUNNELS_BETA) }
    }

    /// `GET /v1/tunnels/{tunnel_id}/certificates/{certificate_id}`. Adds the [`MCP_TUNNELS_BETA`]
    /// header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/tunnels/certificates/retrieve)
    pub async fn tunnel_certificate(
        &self,
        tunnel_id: &str,
        certificate_id: &str,
    ) -> Result<ApiResponse<TunnelCertificate>> {
        let path = ApiPath::new(PATH).id(tunnel_id)?.then("certificates").id(certificate_id)?;
        self.api.get_json_with(&path, &[], &self.options().beta(MCP_TUNNELS_BETA)).await
    }
}
