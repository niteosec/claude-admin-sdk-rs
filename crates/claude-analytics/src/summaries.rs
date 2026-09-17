//! Organization-wide activity summaries.
//!
//! - `GET /v1/organizations/analytics/summaries`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiClient, ApiPath, ApiResponse, Result};
use jiff::Timestamp;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::{check_date, check_date_span, check_items};

const PATH: &str = "v1/organizations/analytics/summaries";
const MAX_RANGE_DAYS: i64 = 366;

/// Response of `GET /v1/organizations/analytics/summaries`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivitySummaries {
    /// One entry per day of the requested range.
    pub summaries: Vec<ActivitySummary>,
}

/// Activity for one day.
///
/// The per-product counts (`chat_*`, `claude_code_*`, `claude_design_*`, `office_agent_*`,
/// `science_*`) are omitted while the per-product breakdown is not enabled for the organization.
/// Seat, invite and adoption-rate fields are `null` on RBAC-group-scoped responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivitySummary {
    /// Seats currently assigned to members.
    #[serde(default)]
    pub assigned_seat_count: Option<u64>,
    /// Users with Cowork activity on the day.
    pub cowork_daily_active_user_count: u64,
    /// Users with Cowork activity in the 30-day rolling window.
    pub cowork_monthly_active_user_count: u64,
    /// Users with Cowork activity in the 7-day rolling window.
    pub cowork_weekly_active_user_count: u64,
    /// Users with token consumption on the day.
    pub daily_active_user_count: u64,
    /// `DAU / assigned_seat_count * 100`.
    #[serde(default)]
    pub daily_adoption_rate: Option<f64>,
    /// End of the period (exclusive), UTC midnight.
    pub ending_at: Timestamp,
    /// Users with token consumption in the 30-day rolling window.
    pub monthly_active_user_count: u64,
    /// `MAU / assigned_seat_count * 100`.
    #[serde(default)]
    pub monthly_adoption_rate: Option<f64>,
    /// Pending invitations to join the organization.
    #[serde(default)]
    pub pending_invite_count: Option<u64>,
    /// Start of the period (inclusive), UTC midnight.
    pub starting_at: Timestamp,
    /// Users with token consumption in the 7-day rolling window.
    pub weekly_active_user_count: u64,
    /// `WAU / assigned_seat_count * 100`.
    #[serde(default)]
    pub weekly_adoption_rate: Option<f64>,
    /// Users with claude.ai chat activity on the day.
    #[serde(default)]
    pub chat_daily_active_user_count: Option<u64>,
    /// Users with claude.ai chat activity in the 30-day rolling window.
    #[serde(default)]
    pub chat_monthly_active_user_count: Option<u64>,
    /// Users with claude.ai chat activity in the 7-day rolling window.
    #[serde(default)]
    pub chat_weekly_active_user_count: Option<u64>,
    /// Users with Claude Code activity on the day.
    #[serde(default)]
    pub claude_code_daily_active_user_count: Option<u64>,
    /// Users with Claude Code activity in the 30-day rolling window.
    #[serde(default)]
    pub claude_code_monthly_active_user_count: Option<u64>,
    /// Users with Claude Code activity in the 7-day rolling window.
    #[serde(default)]
    pub claude_code_weekly_active_user_count: Option<u64>,
    /// Users with Claude Design activity on the day.
    #[serde(default)]
    pub claude_design_daily_active_user_count: Option<u64>,
    /// Users with Claude Design activity in the 30-day rolling window.
    #[serde(default)]
    pub claude_design_monthly_active_user_count: Option<u64>,
    /// Users with Claude Design activity in the 7-day rolling window.
    #[serde(default)]
    pub claude_design_weekly_active_user_count: Option<u64>,
    /// Users with Claude in Office activity on the day.
    #[serde(default)]
    pub office_agent_daily_active_user_count: Option<u64>,
    /// Users with Claude in Office activity in the 30-day rolling window.
    #[serde(default)]
    pub office_agent_monthly_active_user_count: Option<u64>,
    /// Users with Claude in Office activity in the 7-day rolling window.
    #[serde(default)]
    pub office_agent_weekly_active_user_count: Option<u64>,
    /// Users with Claude Science activity on the day.
    #[serde(default)]
    pub science_daily_active_user_count: Option<u64>,
    /// Users with a Claude Science seat entitlement at the daily snapshot. `null` on
    /// RBAC-group-scoped responses.
    #[serde(default)]
    pub science_entitled_user_count: Option<u64>,
    /// Users with Claude Science activity in the 30-day rolling window.
    #[serde(default)]
    pub science_monthly_active_user_count: Option<u64>,
    /// Users with Claude Science activity in the 7-day rolling window.
    #[serde(default)]
    pub science_weekly_active_user_count: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for
/// [`GET /v1/organizations/analytics/summaries`](https://platform.claude.com/docs/en/api/beta/organization/analytics/retrieve_summaries).
///
/// Returns one entry per day from `starting_date` (inclusive) to `ending_date` (exclusive). Data
/// typically lags one day and may be revised by a few percent over the following days; a date that
/// is not available yet is a 400 naming the latest available day. Not paginated.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent"]
pub struct GetActivitySummaries {
    api: ApiClient,
    starting_date: Date,
    ending_date: Option<Date>,
    filters: Vec<String>,
}

impl GetActivitySummaries {
    pub(crate) fn new(api: ApiClient, starting_date: Date) -> Self {
        Self { api, starting_date, ending_date: None, filters: Vec::new() }
    }

    /// `ending_date`: end of the range (exclusive). At most today, which is the default. The range
    /// may span at most 366 days (checked before sending).
    pub fn ending_date(mut self, date: Date) -> Self {
        self.ending_date = Some(date);
        self
    }

    /// Adds a `filter[]` entry, sent as `dimension:value`. Only `rbac_group_id` is supported (tagged
    /// `rbac_group_…` ID or bare UUID); repeat to OR across groups. Scoped rows have `null`
    /// seat, invite and adoption-rate fields. At most 100 entries.
    pub fn filter(mut self, dimension: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.push(format!("{}:{}", dimension.into(), value.into()));
        self
    }

    /// Fetches the summaries.
    pub async fn send(&self) -> Result<ApiResponse<ActivitySummaries>> {
        self.api.get_json(&ApiPath::new(PATH), &self.query()?).await
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        check_date("starting_date", self.starting_date)?;
        if let Some(ending) = self.ending_date {
            check_date_span(self.starting_date, ending, MAX_RANGE_DAYS)?;
        }
        check_items("filter[]", self.filters.len())?;

        let mut query = vec![("starting_date", self.starting_date.to_string())];
        if let Some(ending) = self.ending_date {
            query.push(("ending_date", ending.to_string()));
        }
        query.extend(self.filters.iter().map(|filter| ("filter[]", filter.clone())));
        Ok(query)
    }
}
