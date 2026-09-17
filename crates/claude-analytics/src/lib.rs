//! Unofficial, read-only Rust client for Anthropic's
//! [Claude Enterprise Analytics API](https://platform.claude.com/docs/en/manage-claude/analytics-api).
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! The API reports organization-wide engagement, adoption, cost and token usage across Claude
//! products (chat, projects, Claude Code, Cowork, Claude in Office, …) under
//! `https://api.anthropic.com/v1/organizations/analytics/`.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! ## Access
//!
//! - **Key:** an Analytics API key with the `read:analytics` scope, created by the organization's
//!   primary owner in claude.ai (Organization settings > API). Admin API keys are rejected.
//! - **Availability:** Claude Enterprise organizations. Engagement and adoption data is on every
//!   Enterprise plan; the cost and usage reports apply to usage-based plans and reflect usage
//!   credits only on seat-based plans. Claude Code usage through Amazon Bedrock is not reported.
//! - **Data start:** 2026-01-01. Earlier dates are rejected before sending.
//! - **Freshness:** engagement endpoints lag about a day (available from about 17:00 UTC the next
//!   day) and a too-recent date is a 400 naming the latest available day. Cost and usage data
//!   arrives within 4 to 24 hours and is revised for up to 30 days.
//! - **Rate limit:** 60 requests per minute per organization by default, shared by every key and
//!   endpoint. Share one client.
//!
//! ## Usage-derived data
//!
//! Rows come from observed usage. A skill, connector, plugin or project nobody used in the
//! requested window has no row, so these lists are not inventories.
//!
//! ```no_run
//! use claude_analytics::{AnalyticsClient, EngagementDimension, UserCostOrderBy};
//! use futures::TryStreamExt;
//! use jiff::civil::date;
//!
//! # async fn run() -> claude_analytics::Result<()> {
//! let client = AnalyticsClient::new(std::env::var("ANTHROPIC_ANALYTICS_KEY").unwrap())?;
//!
//! // Daily active users for the first week of September.
//! let summaries = client.summaries(date(2026, 9, 1)).ending_date(date(2026, 9, 8)).send().await?;
//! for day in &summaries.body.summaries {
//!     println!("{} {}", day.starting_at, day.daily_active_user_count);
//! }
//!
//! // Every connector used on one day, per user.
//! let mut connectors = client.connectors().date(date(2026, 9, 15)).group_by(EngagementDimension::UserId).stream();
//! while let Some(row) = connectors.try_next().await? {
//!     println!("{} {:?} {}", row.connector_name, row.user_id, row.distinct_user_count);
//! }
//!
//! // Top spenders over a week. Amounts are decimal strings in cents.
//! let top = client
//!     .user_cost_report("2026-09-01T00:00:00Z".parse().unwrap())
//!     .ending_at("2026-09-08T00:00:00Z".parse().unwrap())
//!     .order_by(UserCostOrderBy::Amount)
//!     .limit(10)
//!     .send()
//!     .await?;
//! for row in &top.body.data {
//!     println!("{} {}", row.actor.user_id, row.amount);
//! }
//! # Ok(()) }
//! ```

mod artifacts;
mod chat_projects;
mod common;
mod connectors;
mod cost;
mod engagement;
mod plugins;
mod report;
mod skills;
mod summaries;
mod usage;
mod users;

pub use artifacts::{ArtifactUsage, ListArtifactUsage};
pub use chat_projects::ChatProjectUsage;
pub use claude_api_core::{
    ApiClient, ApiError, ApiErrorKind, ApiKey, ApiResponse, ClientConfig, Error, KeyKind, PageToken, RateLimit,
    ResponseMeta, Result, RetryPolicy, TokenPage,
};
pub use common::{
    AnalyticsStream, AnalyticsUser, AnalyticsUserActor, AnalyticsUserActorType, AnalyticsUserType, Currency,
    EARLIEST_DATE, EARLIEST_TIMESTAMP, MAX_ARRAY_ITEMS, SortOrder,
};
pub use connectors::{ConnectorChatMetrics, ConnectorOfficeMetrics, ConnectorSessionMetrics, ConnectorUsage};
pub use cost::{CostResult, CostType, TokenType, UserCost};
pub use engagement::{
    EngagementDimension, EngagementReport, ListChatProjectUsage, ListConnectorUsage, ListEngagement, ListPluginUsage,
    ListSkillUsage, ListUserActivity,
};
pub use plugins::{PluginSessionMetrics, PluginUsage};
pub use report::{
    BucketWidth, BucketedReport, ClaudeTagCategory, ContextWindow, CostDimension, InferenceGeo, ListCostReport,
    ListReport, ListUsageReport, ListUserCostReport, ListUserReport, ListUserUsageReport, Product, ReportPage, Speed,
    TimeBucket, UsageDimension, UserCostOrderBy, UserReport, UserUsageOrderBy,
};
pub use skills::{ShareStatus, SkillChatMetrics, SkillOfficeMetrics, SkillSessionMetrics, SkillUsage};
pub use summaries::{ActivitySummaries, ActivitySummary, GetActivitySummaries};
pub use usage::{CacheCreation, ServerToolUse, UsageResult, UserUsage};
pub use users::{
    ChatMetrics, ClaudeCodeCoreMetrics, ClaudeCodeMetrics, CoworkMetrics, DesignMetrics, LinesOfCode, OfficeMetrics,
    OfficeProductMetrics, ScienceMetrics, ToolActionCounts, ToolActions, UserActivity,
};

use jiff::Timestamp;
use jiff::civil::Date;

/// Client for the Claude Enterprise Analytics API.
#[derive(Debug, Clone)]
pub struct AnalyticsClient {
    api: ApiClient,
}

impl AnalyticsClient {
    /// A client with the default configuration, for an Analytics API key (`read:analytics`).
    pub fn new(key: impl Into<String>) -> Result<Self> {
        Ok(Self { api: ApiClient::new(ApiKey::new(key))? })
    }

    /// A client over a configured [`ApiClient`] (custom base URL, retries, HTTP client).
    pub fn from_api_client(api: ApiClient) -> Self {
        Self { api }
    }

    /// The underlying transport.
    pub fn api_client(&self) -> &ApiClient {
        &self.api
    }

    /// [`GET /v1/organizations/analytics/summaries`](https://platform.claude.com/docs/en/api/beta/organization/analytics/retrieve_summaries):
    /// daily, weekly and monthly active users, seats and invites, one entry per day from
    /// `starting_date` (inclusive, no earlier than 2026-01-01).
    pub fn summaries(&self, starting_date: Date) -> GetActivitySummaries {
        GetActivitySummaries::new(self.api.clone(), starting_date)
    }

    /// [`GET /v1/organizations/analytics/usage_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/usage/list):
    /// token usage in time buckets from `starting_at` (inclusive).
    pub fn usage_report(&self, starting_at: Timestamp) -> ListUsageReport {
        ListReport::new(self.api.clone(), starting_at)
    }

    /// [`GET /v1/organizations/analytics/user_usage_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/usage/list_by_user):
    /// token usage per user from `starting_at` (inclusive).
    pub fn user_usage_report(&self, starting_at: Timestamp) -> ListUserUsageReport {
        ListUserReport::new(self.api.clone(), starting_at)
    }

    /// [`GET /v1/organizations/analytics/cost_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/cost/list):
    /// cost in time buckets from `starting_at` (inclusive).
    pub fn cost_report(&self, starting_at: Timestamp) -> ListCostReport {
        ListReport::new(self.api.clone(), starting_at)
    }

    /// [`GET /v1/organizations/analytics/user_cost_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/cost/list_by_user):
    /// cost per user from `starting_at` (inclusive).
    pub fn user_cost_report(&self, starting_at: Timestamp) -> ListUserCostReport {
        ListUserReport::new(self.api.clone(), starting_at)
    }

    /// [`GET /v1/organizations/analytics/users`](https://platform.claude.com/docs/en/api/beta/organization/analytics/users/list):
    /// per-user activity, sorted by email address.
    pub fn users(&self) -> ListUserActivity {
        ListEngagement::new(self.api.clone())
    }

    /// [`GET /v1/organizations/analytics/skills`](https://platform.claude.com/docs/en/api/beta/organization/analytics/skills/list):
    /// per-skill usage, sorted by skill name.
    pub fn skills(&self) -> ListSkillUsage {
        ListEngagement::new(self.api.clone())
    }

    /// [`GET /v1/organizations/analytics/connectors`](https://platform.claude.com/docs/en/api/beta/organization/analytics/connectors/list):
    /// per-connector usage, sorted by connector name.
    pub fn connectors(&self) -> ListConnectorUsage {
        ListEngagement::new(self.api.clone())
    }

    /// [`GET /v1/organizations/analytics/apps/chat/projects`](https://platform.claude.com/docs/en/api/beta/organization/analytics/chat_projects/list):
    /// per-project chat activity, sorted by project ID.
    pub fn chat_projects(&self) -> ListChatProjectUsage {
        ListEngagement::new(self.api.clone())
    }

    /// [`GET /v1/organizations/analytics/plugins`](https://platform.claude.com/docs/en/api/beta/organization/analytics/plugins/list):
    /// per-plugin install and invocation usage, sorted by plugin name.
    pub fn plugins(&self) -> ListPluginUsage {
        ListEngagement::new(self.api.clone())
    }

    /// [`GET /v1/organizations/analytics/artifacts`](https://platform.claude.com/docs/en/api/beta/organization/analytics/artifacts/list):
    /// artifact creation by MIME type and share state on `date` (no earlier than 2026-01-01).
    pub fn artifacts(&self, date: Date) -> ListArtifactUsage {
        ListArtifactUsage::new(self.api.clone(), date)
    }
}
