//! Deployments (an agent bound to an environment, credentials, initial events and an optional cron
//! schedule) and their runs.
//!
//! | Endpoint | Method |
//! |---|---|
//! | `GET /v1/deployments` | [`ManagedAgentsClient::deployments`](crate::ManagedAgentsClient::deployments) |
//! | `GET /v1/deployments/{deployment_id}` | [`ManagedAgentsClient::deployment`](crate::ManagedAgentsClient::deployment) |
//! | `GET /v1/deployment_runs` | [`ManagedAgentsClient::deployment_runs`](crate::ManagedAgentsClient::deployment_runs) |
//! | `GET /v1/deployment_runs/{deployment_run_id}` | [`ManagedAgentsClient::deployment_run`](crate::ManagedAgentsClient::deployment_run) |
//!
//! A deployment's `initial_events` are the prompts sent to every session it creates; they can
//! carry instructions and documents, so treat them as sensitive.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, ApiResponse, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ManagedAgentsClient;
use crate::runtime::{ListRequest, Records, get, list_methods, tagged_union};
use crate::sessions::{
    SessionBudget, SessionCheckout, SessionContentBlock, SessionMemoryStoreAccess, SessionRubric, SessionTextContent,
};

/// The stream returned by [`ListDeployments::stream`].
pub type DeploymentStream = Records<Deployment>;
/// The stream returned by [`ListDeploymentRuns::stream`].
pub type DeploymentRunStream = Records<DeploymentRun>;

/// A deployment: a configured instance of an agent that can run autonomously.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    /// Always `deployment`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Deployment ID (`depl_…` in the reference examples).
    pub id: String,
    /// The agent, pinned to a version.
    pub agent: DeploymentAgentRef,
    /// When the deployment was archived.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the deployment was created.
    pub created_at: Timestamp,
    /// What the deployment does.
    #[serde(default)]
    pub description: Option<String>,
    /// The environment sessions run in.
    pub environment_id: String,
    /// Events sent to each session right after creation.
    pub initial_events: Vec<DeploymentInitialEvent>,
    /// Caller-defined metadata (up to 16 pairs).
    pub metadata: BTreeMap<String, String>,
    /// Name.
    pub name: String,
    /// Why the deployment is paused; documented as non-null exactly when `status` is `paused`.
    #[serde(default)]
    pub paused_reason: Option<DeploymentPausedReason>,
    /// Resources attached to each session (write-only credentials omitted).
    pub resources: Vec<DeploymentResourceConfig>,
    /// Cron schedule.
    #[serde(default)]
    pub schedule: Option<DeploymentSchedule>,
    /// Lifecycle status.
    pub status: DeploymentStatus,
    /// When the deployment was last updated.
    pub updated_at: Timestamp,
    /// Vaults supplying credentials to sessions.
    pub vault_ids: Vec<String>,
    /// Spend ceiling applied to each session.
    #[serde(default)]
    pub budget: Option<SessionBudget>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An agent reference resolved to a concrete version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentAgentRef {
    /// Always `agent`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `agent_…` ID.
    pub id: String,
    /// Agent version.
    pub version: u64,
}

claude_api_core::string_enum! {
    /// Deployment status. Archived deployments are selected with `include_archived`, not a status.
    pub enum DeploymentStatus {
        /// `active`.
        Active = "active",
        /// `paused`.
        Paused = "paused",
    }
}

tagged_union! {
    /// An event sent to each session a deployment creates.
    #[derive(Eq)]
    pub enum DeploymentInitialEvent {
        /// `user.message`.
        UserMessage(DeploymentUserMessage) = "user.message",
        /// `user.define_outcome`: an outcome the agent works toward.
        UserDefineOutcome(DeploymentDefineOutcome) = "user.define_outcome",
        /// `system.message`: system context appended as a `role: "system"` turn.
        SystemMessage(DeploymentSystemMessage) = "system.message",
    }
}

/// A `user.message` initial event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentUserMessage {
    /// Text, image, document or redacted blocks.
    pub content: Vec<SessionContentBlock>,
}

/// A `user.define_outcome` initial event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentDefineOutcome {
    /// What the agent should produce.
    pub description: String,
    /// The grading rubric.
    pub rubric: SessionRubric,
    /// Evaluation-revision cycles before giving up (default 3, max 20).
    #[serde(default)]
    pub max_iterations: Option<u64>,
}

/// A `system.message` initial event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentSystemMessage {
    /// Text-only content.
    pub content: Vec<SessionTextContent>,
}

tagged_union! {
    /// Why a deployment is paused.
    #[derive(Eq)]
    pub enum DeploymentPausedReason {
        /// `manual`: the pause endpoint was called.
        Manual = "manual",
        /// `error`: a scheduled run failed with an error that auto-pauses.
        Error(DeploymentPausedByError) = "error",
    }
}

/// The error behind an automatic pause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPausedByError {
    /// The error; its type matches the failed run's `error.type`.
    pub error: DeploymentPausedReasonError,
}

/// The error of an automatic pause. Every documented variant carries only `type`, so it is modelled
/// as an open enum of that value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPausedReasonError {
    /// The error type.
    #[serde(rename = "type")]
    pub kind: DeploymentErrorType,
}

claude_api_core::string_enum! {
    /// Error types of deployment runs and automatic pauses.
    pub enum DeploymentErrorType {
        /// The environment was archived.
        EnvironmentArchived = "environment_archived_error",
        /// The agent was archived.
        AgentArchived = "agent_archived_error",
        /// The environment no longer exists.
        EnvironmentNotFound = "environment_not_found_error",
        /// A referenced vault no longer exists.
        VaultNotFound = "vault_not_found_error",
        /// A referenced vault is archived.
        VaultArchived = "vault_archived_error",
        /// A referenced file no longer exists.
        FileNotFound = "file_not_found_error",
        /// A referenced memory store is archived.
        MemoryStoreArchived = "memory_store_archived_error",
        /// A skill of the agent no longer exists.
        SkillNotFound = "skill_not_found_error",
        /// A referenced resource of unreported kind no longer exists.
        SessionResourceNotFound = "session_resource_not_found_error",
        /// The workspace was archived.
        WorkspaceArchived = "workspace_archived_error",
        /// The organization is disabled.
        OrganizationDisabled = "organization_disabled_error",
        /// Session creation was rate limited (runs only; the schedule keeps firing).
        SessionRateLimited = "session_rate_limited_error",
        /// Session creation failed validation (runs only).
        SessionCreationRejected = "session_creation_rejected_error",
        /// The documented fallback.
        Unknown = "unknown_error",
        /// Resources are configured but the environment is self-hosted.
        SelfHostedResourcesUnsupported = "self_hosted_resources_unsupported_error",
        /// An MCP server host is blocked by the environment's network policy.
        McpEgressBlocked = "mcp_egress_blocked_error",
    }
}

tagged_union! {
    /// A resource attached to each session a deployment creates.
    #[derive(Eq)]
    pub enum DeploymentResourceConfig {
        /// `github_repository`. The authorization token is write-only and never returned.
        GitHubRepository(DeploymentGitHubRepositoryConfig) = "github_repository",
        /// `file`.
        File(DeploymentFileConfig) = "file",
        /// `memory_store`.
        MemoryStore(DeploymentMemoryStoreConfig) = "memory_store",
    }
}

/// A GitHub repository mounted into each session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentGitHubRepositoryConfig {
    /// Repository URL.
    pub url: String,
    /// Branch or commit (default: the repository's default branch).
    #[serde(default)]
    pub checkout: Option<SessionCheckout>,
    /// Mount path (default `/workspace/<repo-name>`).
    #[serde(default)]
    pub mount_path: Option<String>,
}

/// A file mounted into each session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentFileConfig {
    /// The file.
    pub file_id: String,
    /// Mount path (default `/mnt/session/uploads/<file_id>`).
    #[serde(default)]
    pub mount_path: Option<String>,
}

/// A memory store attached to each session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentMemoryStoreConfig {
    /// `memstore_…` ID.
    pub memory_store_id: String,
    /// Access mode.
    #[serde(default)]
    pub access: Option<SessionMemoryStoreAccess>,
    /// Per-attachment guidance for the agent.
    #[serde(default)]
    pub instructions: Option<String>,
}

/// A 5-field POSIX cron schedule with computed run times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentSchedule {
    /// Always `cron`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Cron expression, for example `0 9 * * 1-5`.
    pub expression: String,
    /// IANA timezone.
    pub timezone: String,
    /// When the schedule last fired.
    #[serde(default)]
    pub last_run_at: Option<Timestamp>,
    /// Up to 5 upcoming fire times (empty once archived).
    #[serde(default)]
    pub upcoming_runs_at: Option<Vec<Timestamp>>,
}

/// One deployment execution: whether it created a session, not how the session went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRun {
    /// Always `deployment_run`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `drun_…` ID.
    pub id: String,
    /// The agent, pinned to a version.
    pub agent: DeploymentAgentRef,
    /// When the run was created.
    pub created_at: Timestamp,
    /// The deployment.
    pub deployment_id: String,
    /// Why session creation failed. Documented as exclusive with `session_id`.
    #[serde(default)]
    pub error: Option<DeploymentRunError>,
    /// The session created, on success.
    #[serde(default)]
    pub session_id: Option<String>,
    /// What triggered the run.
    pub trigger_context: DeploymentTriggerContext,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Why a run failed. Every documented variant carries `type` and `message`, so it is modelled as a
/// struct with an open enum for `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRunError {
    /// The error type.
    #[serde(rename = "type")]
    pub kind: DeploymentErrorType,
    /// Human-readable detail.
    pub message: String,
}

tagged_union! {
    /// What triggered a deployment run.
    #[derive(Eq)]
    pub enum DeploymentTriggerContext {
        /// `schedule`: the cron schedule fired.
        Schedule(DeploymentScheduleTrigger) = "schedule",
        /// `manual`: a session was created directly against the deployment.
        Manual = "manual",
    }
}

/// A `schedule` trigger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentScheduleTrigger {
    /// The scheduled fire time.
    pub scheduled_at: Timestamp,
}

claude_api_core::string_enum! {
    /// What triggered a run (list filter).
    pub enum DeploymentTriggerType {
        /// `schedule`.
        Schedule = "schedule",
        /// `manual`.
        Manual = "manual",
    }
}

impl ManagedAgentsClient {
    /// `GET /v1/deployments`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/deployments/list)
    pub fn deployments(&self) -> ListDeployments {
        let path = Ok(ApiPath::new("v1/deployments"));
        ListDeployments { inner: ListRequest::new(self.api.clone(), self.options(), path, Some(100)) }
    }

    /// `GET /v1/deployments/{deployment_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/deployments/retrieve)
    pub async fn deployment(&self, deployment_id: &str) -> Result<ApiResponse<Deployment>> {
        get(&self.api, ApiPath::new("v1/deployments").id(deployment_id), &[], &self.options()).await
    }

    /// `GET /v1/deployment_runs`: runs across the workspace, or of one deployment.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/deployment_runs/list)
    pub fn deployment_runs(&self) -> ListDeploymentRuns {
        let path = Ok(ApiPath::new("v1/deployment_runs"));
        ListDeploymentRuns { inner: ListRequest::new(self.api.clone(), self.options(), path, Some(1000)) }
    }

    /// `GET /v1/deployment_runs/{deployment_run_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/deployment_runs/retrieve)
    pub async fn deployment_run(&self, deployment_run_id: &str) -> Result<ApiResponse<DeploymentRun>> {
        get(&self.api, ApiPath::new("v1/deployment_runs").id(deployment_run_id), &[], &self.options()).await
    }
}

/// Request builder for `GET /v1/deployments`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListDeployments {
    inner: ListRequest,
}

impl ListDeployments {
    list_methods!(Deployment, DeploymentStream, "Page size, 1 to 100 (server default 20).");

    /// `agent_id`.
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.inner.set("agent_id", agent_id);
        self
    }

    /// `created_at[gte]`.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lte]`.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `include_archived` (server default `false`). Cannot be combined with [`Self::status`]
    /// (checked before sending).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    /// `status`: `active` or `paused` (omit for both). Cannot be combined with
    /// [`Self::include_archived`] (checked before sending).
    pub fn status(mut self, status: DeploymentStatus) -> Self {
        self.inner.set("status", status.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        (self.inner.get("status").is_some() && self.inner.get("include_archived").is_some())
            .then(|| "status and include_archived cannot be combined".to_owned())
    }
}

/// Request builder for `GET /v1/deployment_runs`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListDeploymentRuns {
    inner: ListRequest,
}

impl ListDeploymentRuns {
    list_methods!(DeploymentRun, DeploymentRunStream, "Page size, 1 to 1000 (server default 20).");

    /// `created_at[gt]`.
    pub fn created_at_gt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gt]", at.to_string());
        self
    }

    /// `created_at[gte]`.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lt]`.
    pub fn created_at_lt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lt]", at.to_string());
        self
    }

    /// `created_at[lte]`.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `deployment_id`. An unknown ID returns an empty page, not an error.
    pub fn deployment_id(mut self, deployment_id: impl Into<String>) -> Self {
        self.inner.set("deployment_id", deployment_id);
        self
    }

    /// `has_error`: `true` for failed runs (non-null `error`), `false` for runs that created a
    /// session.
    pub fn has_error(mut self, has_error: bool) -> Self {
        self.inner.set("has_error", has_error.to_string());
        self
    }

    /// `trigger_type`.
    pub fn trigger_type(mut self, trigger_type: DeploymentTriggerType) -> Self {
        self.inner.set("trigger_type", trigger_type.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        None
    }
}
