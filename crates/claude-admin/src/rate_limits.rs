//! Configured rate limits (the Rate Limits API).
//!
//! - `GET /v1/organizations/rate_limits`
//! - `GET /v1/organizations/workspaces/{workspace_id}/rate_limits`
//!
//! Both lists return every entry in one page when `limit` is omitted.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, string_enum};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};

string_enum! {
    /// The kind of rate-limit group.
    pub enum RateLimitGroupType {
        /// `batch`.
        Batch = "batch",
        /// `files`.
        Files = "files",
        /// `model_group`: a model family, listed in `models`.
        ModelGroup = "model_group",
        /// `skills`.
        Skills = "skills",
        /// `token_count`.
        TokenCount = "token_count",
        /// `web_search`.
        WebSearch = "web_search",
    }
}

/// One organization rate-limit group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationRateLimit {
    /// Always `rate_limit`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Stable identifier of the group within the organization.
    pub id: String,
    /// Group kind.
    pub group_type: RateLimitGroupType,
    /// Limiter values applying to the group.
    pub limits: Vec<RateLimitValue>,
    /// Model names and aliases for `model_group` entries; `None` otherwise.
    #[serde(default)]
    pub models: Option<Vec<String>>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One organization limiter value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitValue {
    /// Limiter type, for example `requests_per_minute` or `input_tokens_per_minute`.
    #[serde(rename = "type")]
    pub limiter_type: String,
    /// Configured value.
    pub value: u64,
}

/// A workspace's overrides for one rate-limit group. Groups without overrides are not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRateLimit {
    /// Always `workspace_rate_limit`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Group kind.
    pub group_type: RateLimitGroupType,
    /// Overridden limiter values; others inherit the organization value.
    pub limits: Vec<WorkspaceRateLimitValue>,
    /// Model names and aliases for `model_group` entries; `None` otherwise.
    #[serde(default)]
    pub models: Option<Vec<String>>,
    /// The organization group's `id`.
    pub rate_limit_id: String,
    /// `wrkspc_…` ID.
    pub workspace_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One workspace limiter override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRateLimitValue {
    /// Limiter type.
    #[serde(rename = "type")]
    pub limiter_type: String,
    /// Organization value for the same limiter; `None` when none is configured.
    #[serde(default)]
    pub org_limit: Option<u64>,
    /// Workspace override value.
    pub value: u64,
}

/// Request builder for `GET /v1/organizations/rate_limits`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListOrganizationRateLimits {
    inner: TokenList,
}

impl ListOrganizationRateLimits {
    token_list_methods!(OrganizationRateLimit, "Page size, 1 to 1000. Omit to get every entry in one page.");

    /// `group_type` filter.
    pub fn group_type(mut self, group_type: RateLimitGroupType) -> Self {
        self.inner.params.set("group_type", group_type.as_str());
        self
    }

    /// `model`: only the entry containing this model (full name or alias). The server answers 404
    /// when the model has no rate limits.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.inner.params.set("model", model);
        self
    }
}

/// Request builder for `GET /v1/organizations/workspaces/{workspace_id}/rate_limits`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListWorkspaceRateLimits {
    inner: TokenList,
}

impl ListWorkspaceRateLimits {
    token_list_methods!(WorkspaceRateLimit, "Page size, 1 to 1000. Omit to get every entry in one page.");

    /// `group_type` filter.
    pub fn group_type(mut self, group_type: RateLimitGroupType) -> Self {
        self.inner.params.set("group_type", group_type.as_str());
        self
    }
}

impl AdminClient {
    /// `GET /v1/organizations/rate_limits`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rate_limits/list>
    pub fn rate_limits(&self) -> ListOrganizationRateLimits {
        let path = Ok(ApiPath::new("v1/organizations/rate_limits"));
        ListOrganizationRateLimits { inner: TokenList::new(self.api.clone(), path, 1000) }
    }

    /// `GET /v1/organizations/workspaces/{workspace_id}/rate_limits`. An invalid `workspace_id` is
    /// reported when the request is sent.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/workspaces/rate_limits/list>
    pub fn workspace_rate_limits(&self, workspace_id: &str) -> ListWorkspaceRateLimits {
        let path = ApiPath::new("v1/organizations/workspaces").id(workspace_id).map(|path| path.then("rate_limits"));
        ListWorkspaceRateLimits { inner: TokenList::new(self.api.clone(), path, 1000) }
    }
}
