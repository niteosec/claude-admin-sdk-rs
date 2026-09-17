//! Spend limits and increase requests (the Spend Limits API, Claude Enterprise).
//!
//! - `GET /v1/organizations/spend_limits/effective`
//! - `GET /v1/organizations/spend_limits/{spend_limit_id}`
//! - `GET /v1/organizations/spend_limit_increase_requests`
//! - `GET /v1/organizations/spend_limit_increase_requests/{spend_limit_increase_request_id}`
//!
//! Needs `read:spend_limits`. Amounts are decimal strings in the minor unit of `currency`.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Result, string_enum};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};
use crate::union::tagged_union;

const SPEND_LIMITS: &str = "v1/organizations/spend_limits";
const INCREASE_REQUESTS: &str = "v1/organizations/spend_limit_increase_requests";

string_enum! {
    /// The window a limit resets over.
    pub enum SpendPeriod {
        /// `daily`.
        Daily = "daily",
        /// `monthly`.
        Monthly = "monthly",
        /// `weekly`.
        Weekly = "weekly",
    }
}

string_enum! {
    /// Increase request status.
    pub enum IncreaseRequestStatus {
        /// `approved`.
        Approved = "approved",
        /// `denied`.
        Denied = "denied",
        /// `pending`.
        Pending = "pending",
    }
}

tagged_union! {
    /// What a spend limit applies to, discriminated by `type`.
    pub enum SpendLimitScope {
        /// `user`: one member.
        User(UserSpendScope) = "user",
        /// `seat_tier`.
        SeatTier(SeatTierSpendScope) = "seat_tier",
        /// `rbac_group`.
        RbacGroup(RbacGroupSpendScope) = "rbac_group",
        /// `organization_service`.
        OrganizationService(OrganizationServiceSpendScope) = "organization_service",
        /// `organization`.
        Organization = "organization",
    }
}

/// `user` scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSpendScope {
    /// `user_…` ID.
    pub user_id: String,
}

/// `seat_tier` scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatTierSpendScope {
    /// Seat tier.
    pub seat_tier: String,
}

/// `rbac_group` scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacGroupSpendScope {
    /// `rbac_group_…` ID.
    pub rbac_group_id: String,
}

/// `organization_service` scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationServiceSpendScope {
    /// Service.
    pub service: String,
}

/// A member of the organization, as reported on spend records (`type: user_actor`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActor {
    /// Always `user_actor`.
    #[serde(rename = "type")]
    pub actor_type: String,
    /// True only when the account has been deleted.
    #[serde(default)]
    pub deleted: bool,
    /// Email address; `None` when the account is unavailable or deleted.
    #[serde(default)]
    pub email_address: Option<String>,
    /// Display name; `None` when unavailable, deleted or unset.
    #[serde(default)]
    pub name: Option<String>,
    /// `user_…` ID.
    pub user_id: String,
}

/// A configured spend limit: a cap on metered spend for one scope and period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendLimit {
    /// Always `spend_limit`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `spl_…` ID.
    pub id: String,
    /// Integer decimal string in minor units (`"50000"` is $500.00); `None` when no numeric cap is
    /// configured at this scope.
    #[serde(default)]
    pub amount: Option<String>,
    /// When the limit was created.
    pub created_at: Timestamp,
    /// ISO 4217 billing currency.
    pub currency: String,
    /// Reset window.
    pub period: SpendPeriod,
    /// What the limit applies to.
    pub scope: SpendLimitScope,
    /// When the limit was last modified.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One member's effective limit and period-to-date spend for one period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendSummary {
    /// The member.
    pub actor: MemberActor,
    /// Effective limit as an integer decimal string in minor units; `None` when no limit applies for
    /// this period (another period may still cap the member).
    #[serde(default)]
    pub amount: Option<String>,
    /// ISO 4217 billing currency.
    pub currency: String,
    /// The period this row covers.
    pub period: SpendPeriod,
    /// Spend so far this period, a decimal string in minor units with up to three fractional
    /// digits. `"0"` when the reading is temporarily unavailable.
    pub period_to_date_spend: String,
    /// The member scope (documented as always `user`).
    pub scope: SpendLimitScope,
    /// The scope the effective limit was inherited from.
    pub source: SpendLimitScope,
    /// The spend limit that applies.
    pub spend_limit_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A member's request to raise their spend limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendLimitIncreaseRequest {
    /// Always `spend_limit_increase_request`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Request ID.
    pub id: String,
    /// The requester.
    pub actor: MemberActor,
    /// When the request was made.
    pub created_at: Timestamp,
    /// The period the request is for.
    pub period: SpendPeriod,
    /// When the request was resolved.
    #[serde(default)]
    pub resolved_at: Option<Timestamp>,
    /// Who resolved the request.
    #[serde(default)]
    pub resolved_by: Option<IncreaseRequestResolver>,
    /// Live spend summary for the requester, present while pending.
    #[serde(default)]
    pub spend_summary: Option<SpendSummary>,
    /// Request status.
    pub status: IncreaseRequestStatus,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// Who resolved an increase request, discriminated by `type`.
    pub enum IncreaseRequestResolver {
        /// `user_actor`.
        User(MemberActor) = "user_actor",
        /// `scoped_api_key_actor`: a scoped Admin API key.
        ScopedApiKey(ScopedApiKeyActor) = "scoped_api_key_actor",
    }
}

/// `scoped_api_key_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedApiKeyActor {
    /// The key's ID.
    pub scoped_api_key_id: String,
}

/// Request builder for `GET /v1/organizations/spend_limits/effective`. Pages by member, so one
/// member's period rows never split across pages.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListEffectiveSpendLimits {
    inner: TokenList,
}

impl ListEffectiveSpendLimits {
    token_list_methods!(
        SpendSummary,
        "Members per page, 1 to 1000 (server default 20). A page may hold more rows than members."
    );

    /// Adds a `period[]` filter (at most 3 values).
    pub fn period(mut self, period: SpendPeriod) -> Self {
        self.inner.params.push_bounded("period[]", period.as_str(), 3);
        self
    }

    /// Adds a `user_ids[]` filter (at most 100 values).
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.inner.params.push_bounded("user_ids[]", user_id, 100);
        self
    }
}

/// Request builder for `GET /v1/organizations/spend_limit_increase_requests`. Most recent first;
/// requests from former members are excluded.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSpendLimitIncreaseRequests {
    inner: TokenList,
}

impl ListSpendLimitIncreaseRequests {
    token_list_methods!(SpendLimitIncreaseRequest, "Page size, 1 to 1000 (server default 20).");

    /// Adds an `actor_ids[]` filter (requester `user_…` IDs).
    pub fn actor_id(mut self, actor_id: impl Into<String>) -> Self {
        self.inner.params.push("actor_ids[]", actor_id);
        self
    }

    /// Adds a `status[]` filter. Omit to return all.
    pub fn status(mut self, status: IncreaseRequestStatus) -> Self {
        self.inner.params.push("status[]", status.as_str());
        self
    }
}

impl AdminClient {
    /// `GET /v1/organizations/spend_limits/effective`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/spend_limits/list_effective>
    pub fn effective_spend_limits(&self) -> ListEffectiveSpendLimits {
        let path = Ok(ApiPath::new(SPEND_LIMITS).then("effective"));
        ListEffectiveSpendLimits { inner: TokenList::new(self.api.clone(), path, 1000) }
    }

    /// `GET /v1/organizations/spend_limits/{spend_limit_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/spend_limits/retrieve>
    pub async fn spend_limit(&self, spend_limit_id: &str) -> Result<ApiResponse<SpendLimit>> {
        self.api.get_json(&ApiPath::new(SPEND_LIMITS).id(spend_limit_id)?, &[]).await
    }

    /// `GET /v1/organizations/spend_limit_increase_requests`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/spend_limits/increase_requests/list>
    pub fn spend_limit_increase_requests(&self) -> ListSpendLimitIncreaseRequests {
        let path = Ok(ApiPath::new(INCREASE_REQUESTS));
        ListSpendLimitIncreaseRequests { inner: TokenList::new(self.api.clone(), path, 1000) }
    }

    /// `GET /v1/organizations/spend_limit_increase_requests/{spend_limit_increase_request_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/spend_limits/increase_requests/retrieve>
    pub async fn spend_limit_increase_request(
        &self,
        request_id: &str,
    ) -> Result<ApiResponse<SpendLimitIncreaseRequest>> {
        self.api.get_json(&ApiPath::new(INCREASE_REQUESTS).id(request_id)?, &[]).await
    }
}
