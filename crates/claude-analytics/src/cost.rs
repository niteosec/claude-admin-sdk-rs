//! Cost records.
//!
//! - `GET /v1/organizations/analytics/cost_report` → [`TimeBucket<CostResult>`](crate::TimeBucket)
//! - `GET /v1/organizations/analytics/user_cost_report` → [`UserCost`]
//!
//! Amounts are decimal strings in cents with fractional digits (`"41280.000000"` is $412.80).
//! They are kept as strings: parse them with a decimal type, not binary floating point. On
//! usage-based Enterprise plans they are spend; on seat-based plans they reflect usage credits only.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::{AnalyticsUserActor, Currency};
use crate::report::{ClaudeTagCategory, ContextWindow, InferenceGeo, Speed};

claude_api_core::string_enum! {
    /// A cost component.
    pub enum CostType {
        /// `code_execution`
        CodeExecution = "code_execution",
        /// `tokens`
        Tokens = "tokens",
        /// `web_search`
        WebSearch = "web_search",
    }
}

claude_api_core::string_enum! {
    /// A token type, on rows with `cost_type: tokens`.
    pub enum TokenType {
        /// `cache_creation.ephemeral_1h_input_tokens`
        CacheCreationEphemeral1hInputTokens = "cache_creation.ephemeral_1h_input_tokens",
        /// `cache_creation.ephemeral_5m_input_tokens`
        CacheCreationEphemeral5mInputTokens = "cache_creation.ephemeral_5m_input_tokens",
        /// `cache_read_input_tokens`
        CacheReadInputTokens = "cache_read_input_tokens",
        /// `output_tokens`
        OutputTokens = "output_tokens",
        /// `uncached_input_tokens`
        UncachedInputTokens = "uncached_input_tokens",
    }
}

/// One row of a cost-report time bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostResult {
    /// Post-discount, pre-credit amount in fractional cents.
    pub amount: String,
    /// Claude Tag spend category; `null` for usage that is not Claude Tag.
    #[serde(default)]
    pub claude_tag_category: Option<ClaudeTagCategory>,
    /// Slack user ID the Claude Tag usage is attributed to (not a claude.ai user ID).
    #[serde(default)]
    pub claude_tag_user_id: Option<String>,
    /// Context-window pricing tier.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// Cost component when grouped by `cost_type`; `null` for the combined total.
    #[serde(default)]
    pub cost_type: Option<CostType>,
    /// Currency of the amounts. Currently always `USD`.
    pub currency: Currency,
    /// Inference region; `null` also where the region is unset.
    #[serde(default)]
    pub inference_geo: Option<InferenceGeo>,
    /// List-price (pre-discount) amount in fractional cents.
    pub list_amount: String,
    /// Model name, in the form `models[]` accepts.
    #[serde(default)]
    pub model: Option<String>,
    /// Product surface, for example `chat`, `claude_code` or `claude-tag`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`); `null` on grouped rows for users in no group.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// API requests in the row's scope; `null` when grouped by `cost_type` or `token_type`.
    #[serde(default)]
    pub requests: Option<u64>,
    /// Slack channel the usage originated from.
    #[serde(default)]
    pub slack_channel_id: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<Speed>,
    /// Token type when grouped by `token_type` and `cost_type` is `tokens`.
    #[serde(default)]
    pub token_type: Option<TokenType>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One row of the per-user cost report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCost {
    /// The user the cost is attributed to.
    pub actor: AnalyticsUserActor,
    /// Post-discount, pre-credit amount in fractional cents.
    pub amount: String,
    /// Claude Tag spend category; `null` for usage that is not Claude Tag.
    #[serde(default)]
    pub claude_tag_category: Option<ClaudeTagCategory>,
    /// Slack user ID the Claude Tag usage is attributed to (not a claude.ai user ID).
    #[serde(default)]
    pub claude_tag_user_id: Option<String>,
    /// Context-window pricing tier.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// Cost component; `null` for the combined total.
    #[serde(default)]
    pub cost_type: Option<CostType>,
    /// Currency of the amounts. Currently always `USD`.
    pub currency: Currency,
    /// End of the row's time bucket (exclusive); `null` unless `bucket_width` is set.
    #[serde(default)]
    pub ending_at: Option<Timestamp>,
    /// Inference region; `null` also where the region is unset.
    #[serde(default)]
    pub inference_geo: Option<InferenceGeo>,
    /// List-price (pre-discount) amount in fractional cents.
    pub list_amount: String,
    /// Model name, in the form `models[]` accepts.
    #[serde(default)]
    pub model: Option<String>,
    /// Product surface, for example `chat`, `claude_code` or `claude-tag`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`); `null` on grouped rows for users in no group.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// API requests in the row's scope; `null` when grouped by `cost_type` or `token_type`.
    #[serde(default)]
    pub requests: Option<u64>,
    /// Slack channel the usage originated from.
    #[serde(default)]
    pub slack_channel_id: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<Speed>,
    /// Start of the row's time bucket (inclusive); `null` unless `bucket_width` is set.
    #[serde(default)]
    pub starting_at: Option<Timestamp>,
    /// Token type when `cost_type` is `tokens`.
    #[serde(default)]
    pub token_type: Option<TokenType>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
