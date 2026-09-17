//! The request builder shared by the engagement and adoption lists.
//!
//! - `GET /v1/organizations/analytics/users` ([`ListUserActivity`])
//! - `GET /v1/organizations/analytics/skills` ([`ListSkillUsage`])
//! - `GET /v1/organizations/analytics/connectors` ([`ListConnectorUsage`])
//! - `GET /v1/organizations/analytics/apps/chat/projects` ([`ListChatProjectUsage`])
//! - `GET /v1/organizations/analytics/plugins` ([`ListPluginUsage`])
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::marker::PhantomData;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, PageToken, Result, TokenPage};
use jiff::civil::Date;
use serde::de::DeserializeOwned;

use crate::chat_projects::ChatProjectUsage;
use crate::common::{AnalyticsStream, SortOrder, check_date, check_date_span, check_items, check_limit, invalid};
use crate::connectors::ConnectorUsage;
use crate::plugins::PluginUsage;
use crate::skills::SkillUsage;
use crate::users::UserActivity;

const MAX_LIMIT: u32 = 1000;
const MAX_RANGE_DAYS: i64 = 366;

claude_api_core::string_enum! {
    /// A `group_by[]` dimension on the engagement lists. Support varies by endpoint (see
    /// [`ListEngagement::group_by`]); an unsupported one is a 400.
    pub enum EngagementDimension {
        /// `product`
        Product = "product",
        /// `rbac_group_id`
        RbacGroupId = "rbac_group_id",
        /// `user_id`
        UserId = "user_id",
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::users::UserActivity {}
    impl Sealed for crate::skills::SkillUsage {}
    impl Sealed for crate::connectors::ConnectorUsage {}
    impl Sealed for crate::chat_projects::ChatProjectUsage {}
    impl Sealed for crate::plugins::PluginUsage {}
}

/// A row type of an engagement list.
pub trait EngagementReport: sealed::Sealed + DeserializeOwned + Send + 'static {
    /// The request path.
    const PATH: &'static str;
}

impl EngagementReport for UserActivity {
    const PATH: &'static str = "v1/organizations/analytics/users";
}

impl EngagementReport for SkillUsage {
    const PATH: &'static str = "v1/organizations/analytics/skills";
}

impl EngagementReport for ConnectorUsage {
    const PATH: &'static str = "v1/organizations/analytics/connectors";
}

impl EngagementReport for ChatProjectUsage {
    const PATH: &'static str = "v1/organizations/analytics/apps/chat/projects";
}

impl EngagementReport for PluginUsage {
    const PATH: &'static str = "v1/organizations/analytics/plugins";
}

/// Request builder for the engagement lists:
/// [users](https://platform.claude.com/docs/en/api/beta/organization/analytics/users/list),
/// [skills](https://platform.claude.com/docs/en/api/beta/organization/analytics/skills/list),
/// [connectors](https://platform.claude.com/docs/en/api/beta/organization/analytics/connectors/list),
/// [chat projects](https://platform.claude.com/docs/en/api/beta/organization/analytics/chat_projects/list)
/// and [plugins](https://platform.claude.com/docs/en/api/beta/organization/analytics/plugins/list).
///
/// Select one day with [`Self::date`] or a rollup range with [`Self::starting_date`] and
/// [`Self::ending_date`]. Data typically lags one day (available from about 17:00 UTC the next
/// day) and may be revised by a few percent; a date that is not available yet is a 400 naming the
/// latest available day. Rows are derived from usage: a skill, connector, project or plugin nobody
/// used in the window has no row.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListEngagement<T> {
    api: ApiClient,
    date: Option<Date>,
    starting_date: Option<Date>,
    ending_date: Option<Date>,
    filters: Vec<String>,
    group_by: Vec<EngagementDimension>,
    limit: Option<u32>,
    order: Option<SortOrder>,
    order_by: Option<String>,
    page: Option<PageToken>,
    row: PhantomData<fn() -> T>,
}

/// `GET /v1/organizations/analytics/users`.
pub type ListUserActivity = ListEngagement<UserActivity>;
/// `GET /v1/organizations/analytics/skills`.
pub type ListSkillUsage = ListEngagement<SkillUsage>;
/// `GET /v1/organizations/analytics/connectors`.
pub type ListConnectorUsage = ListEngagement<ConnectorUsage>;
/// `GET /v1/organizations/analytics/apps/chat/projects`.
pub type ListChatProjectUsage = ListEngagement<ChatProjectUsage>;
/// `GET /v1/organizations/analytics/plugins`.
pub type ListPluginUsage = ListEngagement<PluginUsage>;

impl<T: EngagementReport> ListEngagement<T> {
    pub(crate) fn new(api: ApiClient) -> Self {
        Self {
            api,
            date: None,
            starting_date: None,
            ending_date: None,
            filters: Vec::new(),
            group_by: Vec::new(),
            limit: None,
            order: None,
            order_by: None,
            page: None,
            row: PhantomData,
        }
    }

    /// `date`: the UTC day to report. No earlier than 2026-01-01; not combinable with
    /// [`Self::starting_date`] (both checked before sending).
    pub fn date(mut self, date: Date) -> Self {
        self.date = Some(date);
        self
    }

    /// `starting_date`: start of a rollup range (inclusive), one row per entity over the whole
    /// range. No earlier than 2026-01-01; not combinable with [`Self::date`].
    pub fn starting_date(mut self, date: Date) -> Self {
        self.starting_date = Some(date);
        self
    }

    /// `ending_date`: end of the rollup range (exclusive), at most today (the default) and at most
    /// 366 days after `starting_date`. Only valid with [`Self::starting_date`].
    pub fn ending_date(mut self, date: Date) -> Self {
        self.ending_date = Some(date);
        self
    }

    /// Adds a `filter[]` entry, sent as `dimension:value`. Repeat for OR within a dimension and AND
    /// across dimensions; at most 100 entries. Supported dimensions:
    ///
    /// - users: `project_id` (not combinable with `group_by[]` or an `rbac_group_id` filter),
    ///   `rbac_group_id`, `user_id`;
    /// - skills: `product`, `rbac_group_id`, `share_status`, `skill_name`, `user_id`;
    /// - connectors: `connector_name`, `product`, `rbac_group_id`, `user_id`;
    /// - chat projects: `project_id`, `rbac_group_id`, `user_id`;
    /// - plugins: `plugin_name`, `product` (`claude_code` or `cowork`), `rbac_group_id`, `user_id`.
    pub fn filter(mut self, dimension: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push(format!("{}:{}", dimension.into(), value.into()));
        self
    }

    /// Adds a `group_by[]` dimension; at most 100 entries. Supported: users `rbac_group_id`; chat
    /// projects `rbac_group_id`, `user_id`; skills, connectors and plugins `product`,
    /// `rbac_group_id`, `user_id`. RBAC groups overlap, so grouped rows can sum above org totals.
    pub fn group_by(mut self, dimension: EngagementDimension) -> Self {
        self.group_by.push(dimension);
        self
    }

    /// `limit`: results per page, 1 to 1000 (default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `order`: sort direction. Defaults to `asc` on the endpoint's sort column and `desc` when
    /// `order_by` names a metric.
    pub fn order(mut self, order: SortOrder) -> Self {
        self.order = Some(order);
        self
    }

    /// `order_by`: the endpoint's sort column or one of its rankable metrics.
    pub fn order_by(mut self, field: impl Into<String>) -> Self {
        self.order_by = Some(field.into());
        self
    }

    /// `page`: resume from a previous response's `next_page`.
    pub fn page(mut self, token: PageToken) -> Self {
        self.page = Some(token);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ApiResponse<TokenPage<T>>> {
        self.api.get_json(&ApiPath::new(T::PATH), &self.query()?).await
    }

    /// Streams every row, following `next_page` until it is `null`. Starts from [`Self::page`]
    /// when set.
    pub fn stream(self) -> AnalyticsStream<T> {
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
        if self.date.is_some() && self.starting_date.is_some() {
            return Err(invalid("use either date or starting_date, not both".into()));
        }
        if let Some(date) = self.date {
            check_date("date", date)?;
        }
        if let Some(starting) = self.starting_date {
            check_date("starting_date", starting)?;
        }
        match (self.starting_date, self.ending_date) {
            (None, Some(_)) => return Err(invalid("ending_date is only valid with starting_date".into())),
            (Some(starting), Some(ending)) => check_date_span(starting, ending, MAX_RANGE_DAYS)?,
            _ => {}
        }
        check_items("filter[]", self.filters.len())?;
        check_items("group_by[]", self.group_by.len())?;
        check_limit(self.limit, MAX_LIMIT)?;

        let mut query = Vec::new();
        if let Some(date) = self.date {
            query.push(("date", date.to_string()));
        }
        if let Some(starting) = self.starting_date {
            query.push(("starting_date", starting.to_string()));
        }
        if let Some(ending) = self.ending_date {
            query.push(("ending_date", ending.to_string()));
        }
        query.extend(self.filters.iter().map(|filter| ("filter[]", filter.clone())));
        query.extend(self.group_by.iter().map(|dimension| ("group_by[]", dimension.to_string())));
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(order) = &self.order {
            query.push(("order", order.to_string()));
        }
        if let Some(order_by) = &self.order_by {
            query.push(("order_by", order_by.clone()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        Ok(query)
    }
}
