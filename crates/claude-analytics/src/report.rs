//! Request builders, parameters and the page envelope shared by the cost and usage reports.
//!
//! - `GET /v1/organizations/analytics/usage_report` ([`ListReport<UsageResult>`](ListReport))
//! - `GET /v1/organizations/analytics/cost_report` ([`ListReport<CostResult>`](ListReport))
//! - `GET /v1/organizations/analytics/user_usage_report` ([`ListUserReport<UserUsage>`](ListUserReport))
//! - `GET /v1/organizations/analytics/user_cost_report` ([`ListUserReport<UserCost>`](ListUserReport))
//!
//! Cost and usage data typically arrives within four hours (at most 24) and can be revised for 30
//! days. Rows after a response's `data_refreshed_at` are incomplete; for stable results set
//! `ending_at` at or before a previously returned `data_refreshed_at`.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::fmt;
use std::marker::PhantomData;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, PageToken, Result};
use jiff::{SignedDuration, Timestamp};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::{AnalyticsStream, SortOrder, check_items, check_limit, check_time_span, check_timestamp, invalid};
use crate::cost::{CostResult, UserCost};
use crate::usage::{UsageResult, UserUsage};

const MAX_RANGE: SignedDuration = SignedDuration::from_hours(31 * 24);
const MAX_MINUTE_RANGE: SignedDuration = SignedDuration::from_hours(24);
const MAX_USER_LIMIT: u32 = 1000;

claude_api_core::string_enum! {
    /// Time-bucket granularity (`bucket_width`).
    pub enum BucketWidth {
        /// `1d`
        Day = "1d",
        /// `1h`
        Hour = "1h",
        /// `1m`
        Minute = "1m",
    }
}

claude_api_core::string_enum! {
    /// A Claude Tag (Claude in Slack) spend category. New categories may be added.
    pub enum ClaudeTagCategory {
        /// `dm`: direct messages with Claude, reported under the user's own product.
        Dm = "dm",
        /// `engaged`: a person addressed Claude in a channel or thread.
        Engaged = "engaged",
        /// `monitoring`: Claude watching a channel it was asked to monitor.
        Monitoring = "monitoring",
        /// `proactive`: Claude responded without being addressed.
        Proactive = "proactive",
        /// `scheduled`: a scheduled routine ran.
        Scheduled = "scheduled",
    }
}

claude_api_core::string_enum! {
    /// A context-window pricing tier.
    pub enum ContextWindow {
        /// `0-200k`
        UpTo200k = "0-200k",
        /// `200k-1M`
        From200kTo1M = "200k-1M",
    }
}

claude_api_core::string_enum! {
    /// An inference region.
    pub enum InferenceGeo {
        /// `global`
        Global = "global",
        /// `not_available`: filter value matching rows where the region is unset. Rows report
        /// that as `null`.
        NotAvailable = "not_available",
        /// `us`
        Us = "us",
    }
}

claude_api_core::string_enum! {
    /// Inference speed mode.
    pub enum Speed {
        /// `fast`
        Fast = "fast",
        /// `standard`
        Standard = "standard",
    }
}

claude_api_core::string_enum! {
    /// A product surface accepted by `products[]`.
    pub enum Product {
        /// `chat`
        Chat = "chat",
        /// `claude-tag`: Claude Tag, the Claude product in Slack.
        ClaudeTag = "claude-tag",
        /// `claude_code`
        ClaudeCode = "claude_code",
        /// `claude_design`
        ClaudeDesign = "claude_design",
        /// `claude_in_chrome`
        ClaudeInChrome = "claude_in_chrome",
        /// `cowork`
        Cowork = "cowork",
        /// `office_agent`
        OfficeAgent = "office_agent",
    }
}

claude_api_core::string_enum! {
    /// A `group_by[]` dimension on the usage reports.
    pub enum UsageDimension {
        /// `claude_tag_category`
        ClaudeTagCategory = "claude_tag_category",
        /// `claude_tag_user_id`
        ClaudeTagUserId = "claude_tag_user_id",
        /// `context_window`
        ContextWindow = "context_window",
        /// `inference_geo`
        InferenceGeo = "inference_geo",
        /// `model`
        Model = "model",
        /// `product`
        Product = "product",
        /// `rbac_group_id`
        RbacGroupId = "rbac_group_id",
        /// `slack_channel_id`
        SlackChannelId = "slack_channel_id",
        /// `speed`
        Speed = "speed",
    }
}

claude_api_core::string_enum! {
    /// A `group_by[]` dimension on the cost reports.
    pub enum CostDimension {
        /// `claude_tag_category`
        ClaudeTagCategory = "claude_tag_category",
        /// `claude_tag_user_id`
        ClaudeTagUserId = "claude_tag_user_id",
        /// `context_window`
        ContextWindow = "context_window",
        /// `cost_type`
        CostType = "cost_type",
        /// `inference_geo`
        InferenceGeo = "inference_geo",
        /// `model`
        Model = "model",
        /// `product`
        Product = "product",
        /// `rbac_group_id`
        RbacGroupId = "rbac_group_id",
        /// `slack_channel_id`
        SlackChannelId = "slack_channel_id",
        /// `speed`
        Speed = "speed",
        /// `token_type`
        TokenType = "token_type",
    }
}

claude_api_core::string_enum! {
    /// `order_by` on the per-user usage report.
    pub enum UserUsageOrderBy {
        /// `output_tokens`
        OutputTokens = "output_tokens",
        /// `requests`
        Requests = "requests",
        /// `total_tokens` (the default)
        TotalTokens = "total_tokens",
        /// `uncached_input_tokens`
        UncachedInputTokens = "uncached_input_tokens",
    }
}

claude_api_core::string_enum! {
    /// `order_by` on the per-user cost report.
    pub enum UserCostOrderBy {
        /// `amount` (the default)
        Amount = "amount",
        /// `list_amount`
        ListAmount = "list_amount",
    }
}

/// A cost or usage report page: `{data, data_refreshed_at, has_more, next_page, organization_id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportPage<T> {
    /// The rows or time buckets of this page.
    pub data: Vec<T>,
    /// The export this response was served from. Data after it is incomplete. `null` when no
    /// export covers the range yet, in which case there is no data.
    #[serde(default)]
    pub data_refreshed_at: Option<Timestamp>,
    /// Whether another page is available.
    pub has_more: bool,
    /// Token for the next page, bound to this query. It can expire when the data refreshes (HTTP
    /// 410); restart from the first page then.
    #[serde(default)]
    pub next_page: Option<PageToken>,
    /// ID of the organization.
    pub organization_id: String,
}

impl<T> ReportPage<T> {
    /// The token to request next, or `None` when the walk is complete.
    pub fn next(&self) -> Option<&PageToken> {
        if self.has_more { self.next_page.as_ref() } else { None }
    }
}

/// One time bucket of a bucketed report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeBucket<R> {
    /// Start of the bucket (inclusive).
    pub starting_at: Timestamp,
    /// End of the bucket (exclusive).
    pub ending_at: Timestamp,
    /// Empty when the bucket has no data; one combined row without `group_by[]`, otherwise one
    /// row per group, capped at the top 100 groups per bucket.
    pub results: Vec<R>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::usage::UsageResult {}
    impl Sealed for crate::cost::CostResult {}
    impl Sealed for crate::usage::UserUsage {}
    impl Sealed for crate::cost::UserCost {}
}

/// A row type of a bucketed report: [`UsageResult`] or [`CostResult`].
pub trait BucketedReport: sealed::Sealed + DeserializeOwned + Send + 'static {
    /// The `group_by[]` dimensions.
    type Dimension: fmt::Display;
    /// The request path.
    const PATH: &'static str;
}

impl BucketedReport for UsageResult {
    type Dimension = UsageDimension;
    const PATH: &'static str = "v1/organizations/analytics/usage_report";
}

impl BucketedReport for CostResult {
    type Dimension = CostDimension;
    const PATH: &'static str = "v1/organizations/analytics/cost_report";
}

/// A row type of a per-user report: [`UserUsage`] or [`UserCost`].
pub trait UserReport: sealed::Sealed + DeserializeOwned + Send + 'static {
    /// The `group_by[]` dimensions.
    type Dimension: fmt::Display;
    /// The `order_by` metrics.
    type OrderBy: fmt::Display;
    /// The request path.
    const PATH: &'static str;
}

impl UserReport for UserUsage {
    type Dimension = UsageDimension;
    type OrderBy = UserUsageOrderBy;
    const PATH: &'static str = "v1/organizations/analytics/user_usage_report";
}

impl UserReport for UserCost {
    type Dimension = CostDimension;
    type OrderBy = UserCostOrderBy;
    const PATH: &'static str = "v1/organizations/analytics/user_cost_report";
}

/// Array parameters, in the order they are sent.
const ARRAY_KEYS: [&str; 11] = [
    "claude_tag_categories[]",
    "claude_tag_user_ids[]",
    "context_windows[]",
    "group_by[]",
    "inference_geos[]",
    "models[]",
    "products[]",
    "rbac_group_ids[]",
    "slack_channel_ids[]",
    "speeds[]",
    "user_ids[]",
];

/// Parameters common to all four reports.
#[derive(Debug, Clone)]
struct ReportQuery {
    starting_at: Timestamp,
    ending_at: Option<Timestamp>,
    bucket_width: Option<BucketWidth>,
    arrays: Vec<(&'static str, String)>,
    limit: Option<u32>,
    page: Option<PageToken>,
}

impl ReportQuery {
    fn new(starting_at: Timestamp) -> Self {
        Self { starting_at, ending_at: None, bucket_width: None, arrays: Vec::new(), limit: None, page: None }
    }

    fn validate(&self) -> Result<()> {
        check_timestamp("starting_at", self.starting_at)?;
        if let Some(ending_at) = self.ending_at {
            check_time_span(self.starting_at, ending_at, MAX_RANGE, "the range may span at most 31 days")?;
        }
        for key in ARRAY_KEYS {
            check_items(key, self.arrays.iter().filter(|(existing, _)| *existing == key).count())?;
        }
        Ok(())
    }

    /// The common parameters; `extra` goes between the arrays and `page`.
    fn build(&self, extra: Vec<(&'static str, String)>) -> Vec<(&'static str, String)> {
        let mut query = vec![("starting_at", self.starting_at.to_string())];
        if let Some(ending_at) = self.ending_at {
            query.push(("ending_at", ending_at.to_string()));
        }
        if let Some(width) = &self.bucket_width {
            query.push(("bucket_width", width.to_string()));
        }
        for key in ARRAY_KEYS {
            query.extend(self.arrays.iter().filter(|(existing, _)| *existing == key).cloned());
        }
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        query.extend(extra);
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        query
    }
}

/// Setters shared by both report builders.
macro_rules! report_setters {
    () => {
        /// `ending_at`: end of range, exclusive. Defaults to the earlier of now and `starting_at` +
        /// 31 days. The range may span at most 31 days (checked before sending).
        pub fn ending_at(mut self, at: Timestamp) -> Self {
            self.query.ending_at = Some(at);
            self
        }

        /// Adds a `claude_tag_categories[]` filter. Usage with no category never matches.
        pub fn claude_tag_category(self, category: ClaudeTagCategory) -> Self {
            self.push("claude_tag_categories[]", category.to_string())
        }

        /// Adds a `claude_tag_user_ids[]` filter: a Slack user ID such as `U0123ABCDEF`, not a
        /// claude.ai user ID.
        pub fn claude_tag_user_id(self, slack_user_id: impl Into<String>) -> Self {
            self.push("claude_tag_user_ids[]", slack_user_id.into())
        }

        /// Adds a `context_windows[]` filter.
        pub fn context_window(self, window: ContextWindow) -> Self {
            self.push("context_windows[]", window.to_string())
        }

        /// Adds a `group_by[]` dimension. Without one, each bucket or user has a single total row.
        pub fn group_by(self, dimension: R::Dimension) -> Self {
            self.push("group_by[]", dimension.to_string())
        }

        /// Adds an `inference_geos[]` filter. [`InferenceGeo::NotAvailable`] matches rows with no
        /// region.
        pub fn inference_geo(self, geo: InferenceGeo) -> Self {
            self.push("inference_geos[]", geo.to_string())
        }

        /// Adds a `models[]` filter, for example `claude-opus-5`. Defaults to all models.
        pub fn model(self, model: impl Into<String>) -> Self {
            self.push("models[]", model.into())
        }

        /// Adds a `products[]` filter. Defaults to all products.
        pub fn product(self, product: Product) -> Self {
            self.push("products[]", product.to_string())
        }

        /// Adds an `rbac_group_ids[]` filter: a tagged `rbac_group_…` ID or a bare group UUID.
        /// Matches usage by users in the group on the UTC day it occurred.
        pub fn rbac_group_id(self, id: impl Into<String>) -> Self {
            self.push("rbac_group_ids[]", id.into())
        }

        /// Adds a `slack_channel_ids[]` filter.
        pub fn slack_channel_id(self, id: impl Into<String>) -> Self {
            self.push("slack_channel_ids[]", id.into())
        }

        /// Adds a `speeds[]` filter.
        pub fn speed(self, speed: Speed) -> Self {
            self.push("speeds[]", speed.to_string())
        }

        /// Adds a `user_ids[]` filter (tagged `user_…` IDs).
        pub fn user_id(self, id: impl Into<String>) -> Self {
            self.push("user_ids[]", id.into())
        }

        /// `page`: resume from a previous response's `next_page`, with every other parameter
        /// unchanged (a token used with a different query is a 400).
        pub fn page(mut self, token: PageToken) -> Self {
            self.query.page = Some(token);
            self
        }

        fn push(mut self, key: &'static str, value: String) -> Self {
            self.query.arrays.push((key, value));
            self
        }
    };
}

/// Request builder for the bucketed reports: token usage
/// ([`GET /v1/organizations/analytics/usage_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/usage/list))
/// and cost in USD
/// ([`GET /v1/organizations/analytics/cost_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/cost/list))
/// over time, including direct API-key and automation traffic.
///
/// `starting_at` must be within the last 365 days and no earlier than 2026-01-01T00:00:00Z; the
/// earliest bound and the 31-day maximum range are checked before sending. Every array parameter
/// takes at most 100 entries. Buckets come oldest first, including empty ones.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListReport<R> {
    api: ApiClient,
    query: ReportQuery,
    row: PhantomData<fn() -> R>,
}

/// `GET /v1/organizations/analytics/usage_report`.
pub type ListUsageReport = ListReport<UsageResult>;
/// `GET /v1/organizations/analytics/cost_report`.
pub type ListCostReport = ListReport<CostResult>;

impl<R: BucketedReport> ListReport<R> {
    pub(crate) fn new(api: ApiClient, starting_at: Timestamp) -> Self {
        Self { api, query: ReportQuery::new(starting_at), row: PhantomData }
    }

    report_setters!();

    /// `bucket_width`: time-bucket granularity, default `1d`.
    pub fn bucket_width(mut self, width: BucketWidth) -> Self {
        self.query.bucket_width = Some(width);
        self
    }

    /// `limit`: time buckets per page, at least 1. The default and maximum depend on
    /// `bucket_width`: `1d` 7 / 31, `1h` 24 / 168, `1m` 60 / 256 (checked before sending).
    pub fn limit(mut self, limit: u32) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ApiResponse<ReportPage<TimeBucket<R>>>> {
        self.api.get_json(&ApiPath::new(R::PATH), &self.build_query()?).await
    }

    /// Streams every time bucket, following `next_page` until `has_more` is false. Starts from
    /// [`Self::page`] when set.
    pub fn stream(self) -> AnalyticsStream<TimeBucket<R>> {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for bucket in page.data {
                    yield bucket;
                }
                match next {
                    Some(token) => request.query.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn build_query(&self) -> Result<Vec<(&'static str, String)>> {
        self.query.validate()?;
        let max = match self.query.bucket_width.as_ref().unwrap_or(&BucketWidth::Day) {
            BucketWidth::Day => 31,
            BucketWidth::Hour => 168,
            BucketWidth::Minute => 256,
            BucketWidth::Other(_) => u32::MAX,
        };
        check_limit(self.query.limit, max)?;
        Ok(self.query.build(Vec::new()))
    }
}

/// Request builder for the per-user reports: token usage
/// ([`GET /v1/organizations/analytics/user_usage_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/usage/list_by_user))
/// and cost in USD
/// ([`GET /v1/organizations/analytics/user_cost_report`](https://platform.claude.com/docs/en/api/beta/organization/analytics/cost/list_by_user))
/// ranked by a metric. Only usage attributable to a seat user is included.
///
/// `starting_at` must be within the last 365 days and no earlier than 2026-01-01T00:00:00Z; the
/// earliest bound, the 31-day maximum range, the `bucket_width` rules and `limit` are checked
/// before sending. Every array parameter takes at most 100 entries.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListUserReport<R> {
    api: ApiClient,
    query: ReportQuery,
    exclude_deleted_users: Option<bool>,
    order: Option<SortOrder>,
    order_by: Option<String>,
    row: PhantomData<fn() -> R>,
}

/// `GET /v1/organizations/analytics/user_usage_report`.
pub type ListUserUsageReport = ListUserReport<UserUsage>;
/// `GET /v1/organizations/analytics/user_cost_report`.
pub type ListUserCostReport = ListUserReport<UserCost>;

impl<R: UserReport> ListUserReport<R> {
    pub(crate) fn new(api: ApiClient, starting_at: Timestamp) -> Self {
        Self {
            api,
            query: ReportQuery::new(starting_at),
            exclude_deleted_users: None,
            order: None,
            order_by: None,
            row: PhantomData,
        }
    }

    report_setters!();

    /// `bucket_width`: split each user's row per time bucket. Requires [`Self::ending_at`]; with
    /// `1m` the range may span at most 24 hours (both checked before sending). Without it each row
    /// covers the whole range.
    pub fn bucket_width(mut self, width: BucketWidth) -> Self {
        self.query.bucket_width = Some(width);
        self
    }

    /// `limit`: rows per page, 1 to 1000 (default 20). `cost_type` / `token_type` fan-out rows on
    /// the cost report do not count, so a page can hold more.
    pub fn limit(mut self, limit: u32) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// `exclude_deleted_users`: omit rows for deleted users (default false). Pages may then hold
    /// fewer than `limit` rows.
    pub fn exclude_deleted_users(mut self, exclude: bool) -> Self {
        self.exclude_deleted_users = Some(exclude);
        self
    }

    /// `order`: sort direction, default `desc`.
    pub fn order(mut self, order: SortOrder) -> Self {
        self.order = Some(order);
        self
    }

    /// `order_by`: the metric to rank users by.
    pub fn order_by(mut self, metric: R::OrderBy) -> Self {
        self.order_by = Some(metric.to_string());
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ApiResponse<ReportPage<R>>> {
        self.api.get_json(&ApiPath::new(R::PATH), &self.build_query()?).await
    }

    /// Streams every row, following `next_page` until `has_more` is false. Starts from
    /// [`Self::page`] when set.
    pub fn stream(self) -> AnalyticsStream<R> {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for row in page.data {
                    yield row;
                }
                match next {
                    Some(token) => request.query.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn build_query(&self) -> Result<Vec<(&'static str, String)>> {
        self.query.validate()?;
        check_limit(self.query.limit, MAX_USER_LIMIT)?;
        if let Some(width) = &self.query.bucket_width {
            let Some(ending_at) = self.query.ending_at else {
                return Err(invalid("ending_at is required when bucket_width is set".into()));
            };
            if *width == BucketWidth::Minute {
                check_time_span(
                    self.query.starting_at,
                    ending_at,
                    MAX_MINUTE_RANGE,
                    "with bucket_width=1m the range may span at most 24 hours",
                )?;
            }
        }

        let mut extra = Vec::new();
        if let Some(exclude) = self.exclude_deleted_users {
            extra.push(("exclude_deleted_users", exclude.to_string()));
        }
        if let Some(order) = &self.order {
            extra.push(("order", order.to_string()));
        }
        if let Some(order_by) = &self.order_by {
            extra.push(("order_by", order_by.clone()));
        }
        Ok(self.query.build(extra))
    }
}
