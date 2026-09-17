//! Unofficial, read-only Rust client for Anthropic's
//! [Admin API](https://platform.claude.com/docs/en/manage-claude/admin-api).
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! The Admin API exposes an organization's members, invites, workspaces, API keys, rate limits,
//! encryption keys, usage and cost reports, and (for Claude Enterprise) custom roles, groups and
//! spend limits. This crate reads them.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! ```no_run
//! use claude_admin::{AdminClient, ApiKeyStatus, BucketWidth, UsageGroupBy};
//! use futures::TryStreamExt;
//!
//! # async fn run() -> claude_admin::Result<()> {
//! let client = AdminClient::new(std::env::var("ANTHROPIC_ADMIN_KEY").unwrap())?;
//!
//! let organization = client.organization().await?.body;
//! println!("{} ({})", organization.name, organization.id);
//!
//! // Every active API key, across pages.
//! let mut keys = client.api_keys().status(ApiKeyStatus::Active).limit(100).stream();
//! while let Some(key) = keys.try_next().await? {
//!     println!("{} {} expires {:?}", key.id, key.name, key.expires_at);
//! }
//!
//! // Daily token usage by model for the first week of September.
//! let report = client
//!     .messages_usage_report("2026-09-01T00:00:00Z".parse().unwrap())
//!     .ending_at("2026-09-08T00:00:00Z".parse().unwrap())
//!     .bucket_width(BucketWidth::Day)
//!     .group_by(UsageGroupBy::Model)
//!     .send()
//!     .await?;
//! for bucket in &report.body.data {
//!     for row in &bucket.results {
//!         println!("{} {:?} {}", bucket.starting_at, row.model, row.output_tokens);
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! ## Keys and what they reach
//!
//! The transport authenticates with an `x-api-key` header.
//!
//! - **Claude Console Admin API key** (`sk-ant-admin01-…`, created by an organization admin): every
//!   endpoint in this crate except the Claude Enterprise-only ones (RBAC roles and groups, spend
//!   limits). No selectable scopes.
//! - **Personal or service account key not scoped to a workspace**: sent the same way, with the
//!   permissions of the linked account. Workspace keys do not work.
//! - **Claude Enterprise key** (`sk-ant-api01-…`, created in claude.ai): members and invites with
//!   `read:members` (which also reads custom roles and their permissions), groups and their members
//!   with `read:rbac_groups`, spend limits and increase requests with `read:spend_limits`.
//!   `read:org_audit` covers the member, invite, role and group reads but not spend limits.
//! - **Claude Platform on AWS**: only workspaces and external keys are available.
//!
//! Service accounts, federation issuers and federation rules accept only an OAuth bearer token with
//! the `org:admin` scope, and the deprecated MCP tunnel endpoints require an `anthropic-beta` header
//! and tunnel scope. This crate does not cover them.
//!
//! ## Read-only by construction
//!
//! The Admin API can also invite and remove members, change roles, create and archive workspaces,
//! deactivate API keys and set spend limits. This crate implements only GET endpoints, so a key handed
//! to software built on it cannot change the organization through it.

mod api_keys;
mod cost;
mod external_keys;
mod list;
mod organization;
mod rate_limits;
mod rbac;
mod spend_limits;
mod union;
mod usage;
mod users;
mod workspaces;

pub use api_keys::{
    ApiKeyCreatedBy, ApiKeyPrincipal, ApiKeyScope, ApiKeyStatus, CreatorType, ListApiKeys, OrganizationApiKey,
    ServiceAccountPrincipal, UserPrincipal, WorkspaceScope,
};
pub use claude_api_core::{
    ApiClient, ApiError, ApiErrorKind, ApiKey, ApiResponse, ClientConfig, Cursor, CursorPage, Error, KeyKind,
    PageToken, RateLimit, ResponseMeta, Result, RetryPolicy, TokenPage,
};
pub use cost::{Cost, CostBucket, CostGroupBy, CostReport, CostType, TokenType};
pub use external_keys::{
    AwsExternalKeyConfig, AzureExternalKeyConfig, ExternalKey, ExternalKeyAttachment, ExternalKeyProviderConfig,
    GcpExternalKeyConfig, ListExternalKeys,
};
pub use list::RecordStream;
pub use organization::{ComplianceSettings, ComplianceSettingsState, Organization};
pub use rate_limits::{
    ListOrganizationRateLimits, ListWorkspaceRateLimits, OrganizationRateLimit, RateLimitGroupType, RateLimitValue,
    WorkspaceRateLimit, WorkspaceRateLimitValue,
};
pub use rbac::{
    ConnectorResource, ConnectorScopeResource, ConnectorToolResource, GroupSourceType, ListRbacGroupMembers,
    ListRbacGroups, ListRbacRolePermissions, ListRbacRoles, OrganizationResource, PermissionResource, RbacGroup,
    RbacGroupMember, RbacRole, RbacRolePermission,
};
pub use spend_limits::{
    IncreaseRequestResolver, IncreaseRequestStatus, ListEffectiveSpendLimits, ListSpendLimitIncreaseRequests,
    MemberActor, OrganizationServiceSpendScope, RbacGroupSpendScope, ScopedApiKeyActor, SeatTierSpendScope, SpendLimit,
    SpendLimitIncreaseRequest, SpendLimitScope, SpendPeriod, SpendSummary, UserSpendScope,
};
pub use usage::{
    BucketWidth, CacheCreation, ClaudeCodeActor, ClaudeCodeApiActor, ClaudeCodeUsage, ClaudeCodeUsageReport,
    ClaudeCodeUserActor, ContextWindow, CoreMetrics, CustomerType, EstimatedCost, LinesOfCode, MessagesUsage,
    MessagesUsageBucket, MessagesUsageReport, ModelTokens, ModelUsage, ServerToolUse, ServiceTier, Speed,
    SubscriptionType, ToolActionCounts, UsageGroupBy, UsageInferenceGeo,
};
pub use users::{Invite, InviteStatus, ListInvites, ListUsers, OrganizationRole, User};
pub use workspaces::{
    AllowedInferenceGeos, DataResidency, InferenceGeo, ListWorkspaceMembers, ListWorkspaces, Workspace, WorkspaceGeo,
    WorkspaceMember, WorkspaceRole,
};

/// Client for the Admin API.
#[derive(Debug, Clone)]
pub struct AdminClient {
    api: ApiClient,
}

impl AdminClient {
    /// A client with the default configuration.
    ///
    /// Use a Claude Console Admin API key, a personal or service account key not scoped to a
    /// workspace, or a scoped Claude Enterprise key; see the crate documentation for which endpoints
    /// each reaches.
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
}
