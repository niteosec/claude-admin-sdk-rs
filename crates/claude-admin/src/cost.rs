//! The cost report.
//!
//! - `GET /v1/organizations/cost_report`
//!
//! Costs are in USD minor units as decimal strings, daily buckets only. Priority Tier costs are not
//! included; read them from the usage report.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, string_enum};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};
use crate::usage::{BucketWidth, ContextWindow, ServiceTier, UsageInferenceGeo};

string_enum! {
    /// A cost report grouping dimension.
    pub enum CostGroupBy {
        /// `description`: adds parsed fields such as `model` and `inference_geo`.
        Description = "description",
        /// `workspace_id`.
        WorkspaceId = "workspace_id",
    }
}

string_enum! {
    /// Type of cost.
    pub enum CostType {
        /// `code_execution`.
        CodeExecution = "code_execution",
        /// `session_usage`.
        SessionUsage = "session_usage",
        /// `tokens`.
        Tokens = "tokens",
        /// `web_search`.
        WebSearch = "web_search",
    }
}

string_enum! {
    /// Type of token a cost row covers.
    pub enum TokenType {
        /// `cache_creation.ephemeral_1h_input_tokens`.
        CacheCreationEphemeral1hInputTokens = "cache_creation.ephemeral_1h_input_tokens",
        /// `cache_creation.ephemeral_5m_input_tokens`.
        CacheCreationEphemeral5mInputTokens = "cache_creation.ephemeral_5m_input_tokens",
        /// `cache_read_input_tokens`.
        CacheReadInputTokens = "cache_read_input_tokens",
        /// `output_tokens`.
        OutputTokens = "output_tokens",
        /// `uncached_input_tokens`.
        UncachedInputTokens = "uncached_input_tokens",
    }
}

/// One daily bucket of the cost report. Buckets with no costs have empty `results`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostBucket {
    /// Bucket start (inclusive).
    pub starting_at: Timestamp,
    /// Bucket end (exclusive).
    pub ending_at: Timestamp,
    /// One row per group; several when grouping.
    pub results: Vec<Cost>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One cost row. Description-derived fields are `None` unless grouped by `description`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    /// Amount in minor units as a decimal string: `"123.45"` USD is $1.23.
    pub amount: String,
    /// Context window band; also `None` for non-token costs.
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    /// Type of cost.
    #[serde(default)]
    pub cost_type: Option<CostType>,
    /// Currency code; currently always `USD`.
    pub currency: String,
    /// Cost description, for example `Claude Opus 5 Usage - Input Tokens`.
    #[serde(default)]
    pub description: Option<String>,
    /// Inference geo.
    #[serde(default)]
    pub inference_geo: Option<UsageInferenceGeo>,
    /// Model; also `None` for non-token costs.
    #[serde(default)]
    pub model: Option<String>,
    /// Service tier (`batch` or `standard`); also `None` for non-token costs.
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    /// Token type; also `None` for non-token costs.
    #[serde(default)]
    pub token_type: Option<TokenType>,
    /// Workspace ID; also `None` for the default workspace.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/organizations/cost_report`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct CostReport {
    inner: TokenList,
}

impl CostReport {
    token_list_methods!(CostBucket, "Maximum buckets per page, 1 to 31 (server default 7).");

    /// `ending_at`: only buckets that end before this instant.
    pub fn ending_at(mut self, at: Timestamp) -> Self {
        self.inner.params.set("ending_at", at.to_string());
        self
    }

    /// `bucket_width`. Only `1d` is documented; any other width is rejected before sending.
    pub fn bucket_width(mut self, width: BucketWidth) -> Self {
        if width != BucketWidth::Day {
            self.inner.params.reject(format!("cost_report only supports bucket_width 1d, got {width}"));
        }
        self.inner.params.set("bucket_width", width.as_str());
        self
    }

    /// Adds a `group_by[]` dimension.
    pub fn group_by(mut self, dimension: CostGroupBy) -> Self {
        self.inner.params.push("group_by[]", dimension.as_str());
        self
    }
}

impl AdminClient {
    /// `GET /v1/organizations/cost_report`, for buckets starting at or after `starting_at`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/cost_report/retrieve>
    pub fn cost_report(&self, starting_at: Timestamp) -> CostReport {
        let path = Ok(ApiPath::new("v1/organizations/cost_report"));
        let mut inner = TokenList::new(self.api.clone(), path, 31);
        inner.params.set("starting_at", starting_at.to_string());
        CostReport { inner }
    }
}
