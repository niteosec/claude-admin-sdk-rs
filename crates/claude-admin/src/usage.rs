//! Usage reports: Messages API token usage and Claude Code analytics.
//!
//! - `GET /v1/organizations/usage_report/messages`
//! - `GET /v1/organizations/usage_report/claude_code`
//!
//! Claude Console credentials only; Claude Enterprise organizations use the Claude Enterprise
//! Analytics API instead. Usage data typically appears within 5 minutes; Claude Code data only once it
//! is older than 1 hour.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, string_enum};
use jiff::Timestamp;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};
use crate::union::tagged_union;

string_enum! {
    /// Time bucket granularity.
    pub enum BucketWidth {
        /// `1d`: default 7 buckets, at most 31.
        Day = "1d",
        /// `1h`: default 24 buckets, at most 168.
        Hour = "1h",
        /// `1m`: default 60 buckets, at most 1440.
        Minute = "1m",
    }
}

impl BucketWidth {
    /// The documented maximum `limit` for this width.
    fn max_limit(&self) -> Option<u32> {
        match self {
            Self::Day => Some(31),
            Self::Hour => Some(168),
            Self::Minute => Some(1440),
            Self::Other(_) => None,
        }
    }
}

string_enum! {
    /// Context window band.
    pub enum ContextWindow {
        /// `0-200k`.
        UpTo200k = "0-200k",
        /// `200k-1M`.
        From200kTo1M = "200k-1M",
    }
}

string_enum! {
    /// Inference geo reported on usage and cost rows.
    pub enum UsageInferenceGeo {
        /// `global`.
        Global = "global",
        /// `not_available`: the model does not support `inference_geo`.
        NotAvailable = "not_available",
        /// `us`.
        Us = "us",
    }
}

string_enum! {
    /// Service tier.
    pub enum ServiceTier {
        /// `batch`.
        Batch = "batch",
        /// `flex`.
        Flex = "flex",
        /// `flex_discount`.
        FlexDiscount = "flex_discount",
        /// `priority`.
        Priority = "priority",
        /// `priority_on_demand`.
        PriorityOnDemand = "priority_on_demand",
        /// `standard`.
        Standard = "standard",
    }
}

string_enum! {
    /// Request speed (fast mode research preview).
    pub enum Speed {
        /// `standard`.
        Standard = "standard",
        /// `fast`.
        Fast = "fast",
    }
}

string_enum! {
    /// A Messages usage report grouping dimension.
    pub enum UsageGroupBy {
        /// `account_id`.
        AccountId = "account_id",
        /// `api_key_id`.
        ApiKeyId = "api_key_id",
        /// `context_window`.
        ContextWindow = "context_window",
        /// `inference_geo`.
        InferenceGeo = "inference_geo",
        /// `model`.
        Model = "model",
        /// `service_account_id`.
        ServiceAccountId = "service_account_id",
        /// `service_tier`.
        ServiceTier = "service_tier",
        /// `speed`. Sends the `fast-mode-2026-02-01` beta header the API requires for it.
        Speed = "speed",
        /// `workspace_id`.
        WorkspaceId = "workspace_id",
    }
}

/// One time bucket of the Messages usage report. Buckets with no usage have empty `results`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagesUsageBucket {
    /// Bucket start (inclusive).
    pub starting_at: Timestamp,
    /// Bucket end (exclusive).
    pub ending_at: Timestamp,
    /// One row per group; several when grouping.
    pub results: Vec<MessagesUsage>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One usage row. Dimension fields are `None` unless grouped by that dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagesUsage {
    /// User account ID; also `None` for non-OAuth requests.
    #[serde(default)]
    pub account_id: Option<String>,
    /// API key ID; also `None` for Console usage.
    #[serde(default)]
    pub api_key_id: Option<String>,
    /// Cache creation input tokens.
    pub cache_creation: CacheCreation,
    /// Input tokens read from the cache.
    pub cache_read_input_tokens: u64,
    /// Context window band.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// Inference geo.
    #[serde(default)]
    pub inference_geo: Option<UsageInferenceGeo>,
    /// Model.
    #[serde(default)]
    pub model: Option<String>,
    /// Output tokens.
    pub output_tokens: u64,
    /// Server-side tool usage.
    pub server_tool_use: ServerToolUse,
    /// Service account ID; also `None` for non-OIDC-federation requests.
    #[serde(default)]
    pub service_account_id: Option<String>,
    /// Service tier.
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    /// Uncached input tokens.
    pub uncached_input_tokens: u64,
    /// Workspace ID; also `None` for the default workspace.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Cache creation input tokens by cache lifetime.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheCreation {
    /// Tokens written to 1-hour cache entries.
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
    /// Tokens written to 5-minute cache entries.
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
}

/// Server-side tool usage counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerToolUse {
    /// Web search requests.
    pub web_search_requests: u64,
}

/// The beta that enables the `speeds[]` filter and `group_by[]=speed`.
pub const FAST_MODE_BETA: &str = "fast-mode-2026-02-01";

/// Request builder for `GET /v1/organizations/usage_report/messages`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct MessagesUsageReport {
    inner: TokenList,
}

impl MessagesUsageReport {
    token_list_methods!(
        MessagesUsageBucket,
        "Maximum buckets per page. The bound follows `bucket_width`: 31 for `1d` (the default), 168 for `1h`, 1440 for `1m`."
    );

    /// `ending_at`: only buckets that end before this instant.
    pub fn ending_at(mut self, at: Timestamp) -> Self {
        self.inner.params.set("ending_at", at.to_string());
        self
    }

    /// `bucket_width` (server default `1d`).
    pub fn bucket_width(mut self, width: BucketWidth) -> Self {
        self.inner.max_limit = width.max_limit().unwrap_or(u32::MAX);
        self.inner.params.set("bucket_width", width.as_str());
        self
    }

    /// Adds an `account_ids[]` filter.
    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.inner.params.push("account_ids[]", account_id);
        self
    }

    /// Adds an `api_key_ids[]` filter.
    pub fn api_key_id(mut self, api_key_id: impl Into<String>) -> Self {
        self.inner.params.push("api_key_ids[]", api_key_id);
        self
    }

    /// Adds a `context_window[]` filter.
    pub fn context_window(mut self, window: ContextWindow) -> Self {
        self.inner.params.push("context_window[]", window.as_str());
        self
    }

    /// Adds a `group_by[]` dimension. Grouping by [`UsageGroupBy::Speed`] also sends the
    /// `fast-mode-2026-02-01` beta header, which the API requires for it.
    pub fn group_by(mut self, dimension: UsageGroupBy) -> Self {
        if dimension == UsageGroupBy::Speed {
            self = self.fast_mode();
        }
        self.inner.params.push("group_by[]", dimension.as_str());
        self
    }

    /// Adds an `inference_geos[]` filter.
    pub fn inference_geo(mut self, geo: UsageInferenceGeo) -> Self {
        self.inner.params.push("inference_geos[]", geo.as_str());
        self
    }

    /// Adds a `models[]` filter.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.inner.params.push("models[]", model);
        self
    }

    /// Adds a `service_account_ids[]` filter.
    pub fn service_account_id(mut self, service_account_id: impl Into<String>) -> Self {
        self.inner.params.push("service_account_ids[]", service_account_id);
        self
    }

    /// Adds a `service_tiers[]` filter.
    pub fn service_tier(mut self, tier: ServiceTier) -> Self {
        self.inner.params.push("service_tiers[]", tier.as_str());
        self
    }

    /// Adds a `speeds[]` filter (Claude Code research preview). Also sends the
    /// `fast-mode-2026-02-01` beta header, which the API requires for it.
    pub fn speed(mut self, speed: Speed) -> Self {
        self = self.fast_mode();
        self.inner.params.push("speeds[]", speed.as_str());
        self
    }

    fn fast_mode(mut self) -> Self {
        self.inner.options = std::mem::take(&mut self.inner.options).beta(FAST_MODE_BETA);
        self
    }

    /// Adds a `workspace_ids[]` filter.
    pub fn workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.inner.params.push("workspace_ids[]", workspace_id);
        self
    }
}

string_enum! {
    /// Claude Code customer type.
    pub enum CustomerType {
        /// `api`: pay-as-you-go API.
        Api = "api",
        /// `subscription`: Pro/Team plans.
        Subscription = "subscription",
    }
}

string_enum! {
    /// Subscription tier.
    pub enum SubscriptionType {
        /// `enterprise`.
        Enterprise = "enterprise",
        /// `team`.
        Team = "team",
    }
}

/// One actor's Claude Code activity for one UTC day. Remote and local usage are separate rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaudeCodeUsage {
    /// The user or API key that acted.
    pub actor: ClaudeCodeActor,
    /// Productivity metrics.
    pub core_metrics: CoreMetrics,
    /// Customer type.
    pub customer_type: CustomerType,
    /// Midnight UTC of the covered day.
    pub date: Timestamp,
    /// Whether the usage came from remote sessions, such as Claude Code on the web.
    pub is_remote: bool,
    /// Tokens and estimated cost per model.
    pub model_breakdown: Vec<ModelUsage>,
    /// Organization ID.
    pub organization_id: String,
    /// Terminal or environment, for example `vscode` or `iTerm.app`.
    pub terminal_type: String,
    /// Accepted and rejected proposals by tool, for example `edit_tool`.
    pub tool_actions: BTreeMap<String, ToolActionCounts>,
    /// Subscription tier; `None` for API customers.
    #[serde(default)]
    pub subscription_type: Option<SubscriptionType>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// The Claude Code actor, discriminated by `type`.
    pub enum ClaudeCodeActor {
        /// `user_actor`: a user authenticated through OAuth.
        User(ClaudeCodeUserActor) = "user_actor",
        /// `api_actor`: an API key.
        Api(ClaudeCodeApiActor) = "api_actor",
    }
}

/// `user_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeUserActor {
    /// Email address.
    pub email_address: String,
}

/// `api_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeApiActor {
    /// API key name.
    pub api_key_name: String,
}

/// Claude Code productivity metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreMetrics {
    /// Commits created through Claude Code.
    pub commits_by_claude_code: u64,
    /// Lines changed.
    pub lines_of_code: LinesOfCode,
    /// Distinct sessions.
    pub num_sessions: u64,
    /// Pull requests created through Claude Code.
    pub pull_requests_by_claude_code: u64,
}

/// Lines added and removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinesOfCode {
    /// Lines added.
    pub added: u64,
    /// Lines removed.
    pub removed: u64,
}

/// Tool proposal outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActionCounts {
    /// Accepted proposals.
    pub accepted: u64,
    /// Rejected proposals.
    pub rejected: u64,
}

/// Per-model tokens and estimated cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Estimated cost.
    pub estimated_cost: EstimatedCost,
    /// Model name.
    pub model: String,
    /// Token counts.
    pub tokens: ModelTokens,
}

/// An estimated cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimatedCost {
    /// Amount in minor currency units (cents for USD). Documented as a number.
    pub amount: f64,
    /// Currency code, for example `USD`.
    pub currency: String,
}

/// Token counts for one model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTokens {
    /// Cache creation tokens.
    pub cache_creation: u64,
    /// Cache read tokens.
    pub cache_read: u64,
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
}

/// Request builder for `GET /v1/organizations/usage_report/claude_code`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ClaudeCodeUsageReport {
    inner: TokenList,
}

impl ClaudeCodeUsageReport {
    token_list_methods!(ClaudeCodeUsage, "Records per page, 1 to 1000 (server default 20).");
}

impl AdminClient {
    /// `GET /v1/organizations/usage_report/messages`, for buckets starting at or after `starting_at`
    /// (snapped to the start of the minute, hour or day in UTC).
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/usage_report/retrieve_messages>
    pub fn messages_usage_report(&self, starting_at: Timestamp) -> MessagesUsageReport {
        let path = Ok(ApiPath::new("v1/organizations/usage_report/messages"));
        let mut inner = TokenList::new(self.api.clone(), path, 31);
        inner.params.set("starting_at", starting_at.to_string());
        MessagesUsageReport { inner }
    }

    /// `GET /v1/organizations/usage_report/claude_code`, for the single UTC day `starting_at`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/usage_report/retrieve_claude_code>
    pub fn claude_code_usage_report(&self, starting_at: Date) -> ClaudeCodeUsageReport {
        let path = Ok(ApiPath::new("v1/organizations/usage_report/claude_code"));
        let mut inner = TokenList::new(self.api.clone(), path, 1000);
        inner.params.set("starting_at", starting_at.to_string());
        ClaudeCodeUsageReport { inner }
    }
}
