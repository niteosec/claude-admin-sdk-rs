//! Token usage records.
//!
//! - `GET /v1/organizations/analytics/usage_report` → [`TimeBucket<UsageResult>`](crate::TimeBucket)
//! - `GET /v1/organizations/analytics/user_usage_report` → [`UserUsage`]
//!
//! Dimension fields (`model`, `product`, `speed`, …) are `null` unless requested in `group_by[]`,
//! and can also be `null` on grouped rows that have no value for the dimension.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::AnalyticsUserActor;
use crate::report::{ClaudeTagCategory, ContextWindow, InferenceGeo, Speed};

/// Input tokens spent creating cache entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheCreation {
    /// Input tokens used to create 1-hour cache entries.
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
    /// Input tokens used to create 5-minute cache entries.
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
}

/// Server-side tool usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerToolUse {
    /// Web search requests made.
    pub web_search_requests: u64,
}

/// One row of a usage-report time bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageResult {
    /// Cache-creation input tokens.
    pub cache_creation: CacheCreation,
    /// Input tokens read from the cache.
    pub cache_read_input_tokens: u64,
    /// Claude Tag spend category; `null` for usage that is not Claude Tag.
    #[serde(default)]
    pub claude_tag_category: Option<ClaudeTagCategory>,
    /// Slack user ID the Claude Tag usage is attributed to (not a claude.ai user ID).
    #[serde(default)]
    pub claude_tag_user_id: Option<String>,
    /// Context-window pricing tier.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// Inference region; `null` also where the region is unset.
    #[serde(default)]
    pub inference_geo: Option<InferenceGeo>,
    /// Model name, in the form `models[]` accepts.
    #[serde(default)]
    pub model: Option<String>,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Product surface, for example `chat`, `claude_code` or `claude-tag`; some unattributed usage
    /// reports `other`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`). Groups overlap; on grouped rows `null` is the single row for
    /// users in no group.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// API requests in the row's scope (execution spans for code execution).
    #[serde(default)]
    pub requests: Option<u64>,
    /// Server-side tool usage.
    pub server_tool_use: ServerToolUse,
    /// Slack channel the usage originated from.
    #[serde(default)]
    pub slack_channel_id: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<Speed>,
    /// Uncached input tokens processed.
    pub uncached_input_tokens: u64,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One row of the per-user usage report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserUsage {
    /// The user the usage is attributed to.
    pub actor: AnalyticsUserActor,
    /// Cache-creation input tokens.
    pub cache_creation: CacheCreation,
    /// Input tokens read from the cache.
    pub cache_read_input_tokens: u64,
    /// Claude Tag spend category; `null` for usage that is not Claude Tag.
    #[serde(default)]
    pub claude_tag_category: Option<ClaudeTagCategory>,
    /// Slack user ID the Claude Tag usage is attributed to (not a claude.ai user ID).
    #[serde(default)]
    pub claude_tag_user_id: Option<String>,
    /// Context-window pricing tier.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// End of the row's time bucket (exclusive); `null` unless `bucket_width` is set.
    #[serde(default)]
    pub ending_at: Option<Timestamp>,
    /// Inference region; `null` also where the region is unset.
    #[serde(default)]
    pub inference_geo: Option<InferenceGeo>,
    /// Model name, in the form `models[]` accepts.
    #[serde(default)]
    pub model: Option<String>,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Product surface, for example `chat`, `claude_code` or `claude-tag`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`); `null` on grouped rows for users in no group.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// API requests in the row's scope.
    #[serde(default)]
    pub requests: Option<u64>,
    /// Server-side tool usage.
    pub server_tool_use: ServerToolUse,
    /// Slack channel the usage originated from.
    #[serde(default)]
    pub slack_channel_id: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<Speed>,
    /// Start of the row's time bucket (inclusive); `null` unless `bucket_width` is set.
    #[serde(default)]
    pub starting_at: Option<Timestamp>,
    /// Total tokens across all token types; the default ranking metric.
    pub total_tokens: u64,
    /// Uncached input tokens processed.
    pub uncached_input_tokens: u64,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
