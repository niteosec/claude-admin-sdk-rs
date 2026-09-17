//! Artifact-creation activity by MIME type.
//!
//! - `GET /v1/organizations/analytics/artifacts` → [`ArtifactUsage`]
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, PageToken, Result, TokenPage};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::{AnalyticsStream, check_date, check_items, check_limit, invalid};
use crate::engagement::EngagementDimension;

const PATH: &str = "v1/organizations/analytics/artifacts";
const MAX_LIMIT: u32 = 1000;

/// One cell of the (`artifact_type`, `is_shared`) cube, optionally per product, user or RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUsage {
    /// Canonical MIME type (for example `text/markdown`) or `other`. Claude Code and Cowork
    /// artifacts report as `text/html`.
    pub artifact_type: String,
    /// Artifacts created in this cell.
    pub artifacts_created_count: u64,
    /// Distinct users who created artifacts in this cell.
    pub distinct_user_count: u64,
    /// Whether the artifacts have ever been shared.
    pub is_shared: bool,
    /// Artifacts of this cell that have been published; never above `artifacts_created_count`.
    pub published_artifacts_created_count: u64,
    /// Product surface (`chat`, `claude_code` or `cowork`), when grouped by `product`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`), when grouped by `rbac_group_id`.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// RBAC group display name; `null` if deleted or unresolved.
    #[serde(default)]
    pub rbac_group_name: Option<String>,
    /// Tagged user ID, when grouped by `user_id`.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for
/// [`GET /v1/organizations/analytics/artifacts`](https://platform.claude.com/docs/en/api/beta/organization/analytics/artifacts/list).
///
/// Without `group_by[]` the whole cube comes back in one response (`next_page` is `null`); grouped
/// queries paginate. `date` must be no earlier than 2026-01-01 (checked before sending); data
/// typically lags one day.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListArtifactUsage {
    api: ApiClient,
    date: Date,
    filters: Vec<String>,
    group_by: Vec<EngagementDimension>,
    limit: Option<u32>,
    page: Option<PageToken>,
}

impl ListArtifactUsage {
    pub(crate) fn new(api: ApiClient, date: Date) -> Self {
        Self { api, date, filters: Vec::new(), group_by: Vec::new(), limit: None, page: None }
    }

    /// Adds a `filter[]` entry, sent as `dimension:value`; at most 100. Supported dimensions:
    /// `artifact_type` (MIME type or `other`), `is_shared` (`true` / `false`), `product` (`chat`,
    /// `claude_code`, `cowork`), `rbac_group_id`, `user_id`.
    pub fn filter(mut self, dimension: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push(format!("{}:{}", dimension.into(), value.into()));
        self
    }

    /// Adds a `group_by[]` dimension (`product`, `rbac_group_id`, `user_id`); at most 100.
    pub fn group_by(mut self, dimension: EngagementDimension) -> Self {
        self.group_by.push(dimension);
        self
    }

    /// `limit`: 1 to 1000 (default 100). Only a page size for grouped queries.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `page`: resume from a previous response's `next_page`. Only valid with `group_by[]`
    /// (checked before sending).
    pub fn page(mut self, token: PageToken) -> Self {
        self.page = Some(token);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ApiResponse<TokenPage<ArtifactUsage>>> {
        self.api.get_json(&ApiPath::new(PATH), &self.query()?).await
    }

    /// Streams every row, following `next_page` until it is `null`.
    pub fn stream(self) -> AnalyticsStream<ArtifactUsage> {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for row in page.data {
                    yield row;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        check_date("date", self.date)?;
        check_items("filter[]", self.filters.len())?;
        check_items("group_by[]", self.group_by.len())?;
        check_limit(self.limit, MAX_LIMIT)?;
        if self.page.is_some() && self.group_by.is_empty() {
            return Err(invalid("page is only valid with group_by[]".into()));
        }

        let mut query = vec![("date", self.date.to_string())];
        query.extend(self.filters.iter().map(|filter| ("filter[]", filter.clone())));
        query.extend(self.group_by.iter().map(|dimension| ("group_by[]", dimension.to_string())));
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        Ok(query)
    }
}
