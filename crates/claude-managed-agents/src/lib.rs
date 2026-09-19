//! Unofficial, read-only Rust client for Anthropic's
//! [Claude Managed Agents API](https://platform.claude.com/docs/en/managed-agents/overview) (beta).
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! Managed Agents are agents defined and run through the Claude API: an agent's model, system prompt,
//! tools, MCP servers and skills; the vaults holding the credentials those tools use; the
//! environments and deployments that run it; and the sessions, memory and runs it produces. This
//! crate reads that estate. It never creates, updates, archives or deletes anything.
//!
//! Every request carries the `managed-agents-2026-04-01` beta header. Resources belong to a
//! workspace: a key bound to one workspace needs nothing more, and a key that spans several selects
//! one with [`ManagedAgentsClient::in_workspace`].
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

pub use claude_api_core::{
    ApiClient, ApiError, ApiErrorKind, ApiKey, ApiResponse, ByteStream, ClientConfig, Cursor, CursorPage, Download,
    Error, KeyKind, PageToken, RateLimit, RequestOptions, ResponseMeta, Result, RetryPolicy, TokenPage,
};

/// The beta every Managed Agents endpoint requires.
pub const MANAGED_AGENTS_BETA: &str = "managed-agents-2026-04-01";

/// Client for the Managed Agents API.
#[derive(Debug, Clone)]
pub struct ManagedAgentsClient {
    api: ApiClient,
    workspace_id: Option<String>,
}

impl ManagedAgentsClient {
    /// A client with the default configuration, for a regular Claude API key.
    pub fn new(key: impl Into<String>) -> Result<Self> {
        Ok(Self::from_api_client(ApiClient::new(ApiKey::new(key))?))
    }

    /// A client over a configured [`ApiClient`] (custom base URL, retries, HTTP client). The beta
    /// header is added per request, so the configuration does not need it.
    pub fn from_api_client(api: ApiClient) -> Self {
        Self { api, workspace_id: None }
    }

    /// A copy of this client whose requests target `workspace_id` (`anthropic-workspace-id`), for
    /// keys that span several workspaces. Clones share the connection pool and rate-limit state.
    pub fn in_workspace(&self, workspace_id: impl Into<String>) -> Self {
        Self { api: self.api.clone(), workspace_id: Some(workspace_id.into()) }
    }

    /// The workspace requests target, when one was selected.
    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }

    /// The underlying transport.
    pub fn api_client(&self) -> &ApiClient {
        &self.api
    }

    /// Headers for every request: the Managed Agents beta and the selected workspace.
    pub(crate) fn options(&self) -> RequestOptions {
        let options = RequestOptions::default().beta(MANAGED_AGENTS_BETA);
        match &self.workspace_id {
            Some(workspace_id) => options.workspace_id(workspace_id.clone()),
            None => options,
        }
    }
}
