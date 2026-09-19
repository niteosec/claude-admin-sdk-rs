//! Sessions: runs of an agent, with their resources, threads and event history.
//!
//! | Endpoint | Method |
//! |---|---|
//! | `GET /v1/sessions` | [`ManagedAgentsClient::sessions`](crate::ManagedAgentsClient::sessions) |
//! | `GET /v1/sessions/{session_id}` | [`ManagedAgentsClient::session`](crate::ManagedAgentsClient::session) |
//! | `GET /v1/sessions/{session_id}/events` | [`ManagedAgentsClient::session_events`](crate::ManagedAgentsClient::session_events) |
//! | `GET /v1/sessions/{session_id}/resources` | [`ManagedAgentsClient::session_resources`](crate::ManagedAgentsClient::session_resources) |
//! | `GET /v1/sessions/{session_id}/resources/{resource_id}` | [`ManagedAgentsClient::session_resource`](crate::ManagedAgentsClient::session_resource) |
//! | `GET /v1/sessions/{session_id}/threads` | [`ManagedAgentsClient::session_threads`](crate::ManagedAgentsClient::session_threads) |
//! | `GET /v1/sessions/{session_id}/threads/{thread_id}` | [`ManagedAgentsClient::session_thread`](crate::ManagedAgentsClient::session_thread) |
//! | `GET /v1/sessions/{session_id}/threads/{thread_id}/events` | [`ManagedAgentsClient::session_thread_events`](crate::ManagedAgentsClient::session_thread_events) |
//!
//! **Session events are sensitive.** They hold the conversation itself (user and agent messages,
//! tool inputs and results, documents and images) and may include secrets that passed through
//! tools. Store and log them accordingly.
//!
//! The server-sent-event streams (`…/events/stream`, `…/threads/{thread_id}/stream`) are not
//! implemented; page through the event lists instead.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

mod agent;
mod content;
mod events;

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, ApiResponse, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use agent::{
    SessionAdvisor, SessionAgent, SessionAgentTool, SessionAgentToolConfig, SessionAgentToolset,
    SessionBuiltinToolConfig, SessionCustomTool, SessionCustomToolInputSchema, SessionEffort, SessionMcpServer,
    SessionMcpToolConfig, SessionMcpToolset, SessionModelConfig, SessionModelSpeed, SessionMultiagent,
    SessionPermissionPolicy, SessionRosterAgent, SessionSkill, SessionSkillVersion, SessionThreadAgent,
    SessionToolDefaultConfig, SessionUserLocation, SessionWebFetchToolConfig, SessionWebSearchToolConfig,
};
pub use content::{
    SessionBase64Source, SessionContentBlock, SessionDocumentBlock, SessionDocumentSource, SessionFileSource,
    SessionImageBlock, SessionImageSource, SessionRubric, SessionSearchResultBlock, SessionSearchResultCitations,
    SessionTextBlock, SessionTextContent, SessionTextRubric, SessionUrlSource,
};
pub use events::{
    SessionAgentCustomToolUseEvent, SessionAgentMcpToolResultEvent, SessionAgentMcpToolUseEvent,
    SessionAgentMessageEvent, SessionAgentToolResultEvent, SessionAgentToolUseEvent, SessionAutoEvaluation,
    SessionAutoJudgement, SessionBasicEvent, SessionCredentialErrorDetail, SessionDefineOutcomeEvent,
    SessionErrorDetail, SessionErrorEvent, SessionEvaluatedPermission, SessionEvent, SessionEventError,
    SessionJudgementReason, SessionMcpErrorDetail, SessionModelRequestEndEvent, SessionModelUsage,
    SessionOutcomeEvaluationEndEvent, SessionOutcomeEvaluationProgressEvent, SessionRequiresAction, SessionRetryStatus,
    SessionStatusIdleEvent, SessionStopReason, SessionSystemMessageEvent, SessionThreadLifecycleEvent,
    SessionThreadMessageReceivedEvent, SessionThreadMessageSentEvent, SessionThreadStatusIdleEvent,
    SessionToolConfirmationResult, SessionToolEvaluation, SessionUpdatedEvent, SessionUsageEvent,
    SessionUserCustomToolResultEvent, SessionUserInterruptEvent, SessionUserMessageEvent,
    SessionUserToolConfirmationEvent, SessionUserToolResultEvent,
};

use crate::ManagedAgentsClient;
use crate::runtime::{ListRequest, Records, RuntimePage, get, list_methods, tagged_union};

/// A page of sessions (carries `prev_page` as well as `next_page`).
pub type SessionPage = ApiResponse<RuntimePage<Session>>;

/// The stream returned by [`ListSessions::stream`].
pub type SessionStream = Records<Session>;
/// The stream returned by [`ListSessionEvents::stream`] and [`ListSessionThreadEvents::stream`].
pub type SessionEventStream = Records<SessionEvent>;
/// The stream returned by [`ListSessionResources::stream`].
pub type SessionResourceStream = Records<SessionResource>;
/// The stream returned by [`ListSessionThreads::stream`].
pub type SessionThreadStream = Records<SessionThread>;

/// A Managed Agents session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Always `session`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `sesn_…` ID.
    pub id: String,
    /// The agent definition, as resolved when the session was created.
    pub agent: SessionAgent,
    /// When the session was archived.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// Hard spend ceiling.
    #[serde(default)]
    pub budget: Option<SessionBudget>,
    /// When the session was created.
    pub created_at: Timestamp,
    /// The environment the session runs in.
    pub environment_id: String,
    /// Caller-defined metadata.
    pub metadata: BTreeMap<String, String>,
    /// One entry per `define_outcome` event sent to the session.
    pub outcome_evaluations: Vec<SessionOutcomeEvaluation>,
    /// Files, repositories and memory stores mounted into the session.
    pub resources: Vec<SessionResource>,
    /// Timing statistics.
    pub stats: SessionStats,
    /// Current status.
    pub status: SessionStatus,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
    /// When the session was last updated.
    pub updated_at: Timestamp,
    /// Cumulative usage.
    pub usage: SessionUsage,
    /// Vaults attached at creation.
    pub vault_ids: Vec<String>,
    /// The deployment the session was created from, if any.
    #[serde(default)]
    pub deployment_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Session status.
    pub enum SessionStatus {
        /// `rescheduling`: recovering from an error.
        Rescheduling = "rescheduling",
        /// `running`: the agent is working.
        Running = "running",
        /// `idle`: waiting for input.
        Idle = "idle",
        /// `terminated`: ended.
        Terminated = "terminated",
    }
}

claude_api_core::string_enum! {
    /// Sort direction, by `created_at` (sessions) or `processed_at` (events).
    pub enum SessionOrder {
        /// Oldest first.
        Asc = "asc",
        /// Newest first.
        Desc = "desc",
    }
}

/// A hard spend ceiling: the session stops issuing model requests once its tracked list cost
/// reaches `max_list_cost`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBudget {
    /// Always `limit`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The ceiling.
    pub max_list_cost: SessionMonetaryAmount,
}

/// A monetary amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMonetaryAmount {
    /// Amount in minor units as an integer decimal string: `"2500"` is $25.00.
    pub amount: String,
    /// ISO 4217 currency code.
    pub currency: SessionCurrency,
}

claude_api_core::string_enum! {
    /// Currency code. `USD` is the only documented value.
    pub enum SessionCurrency {
        /// US dollars.
        Usd = "USD",
    }
}

/// Evaluation state of one outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOutcomeEvaluation {
    /// Always `outcome_evaluation`.
    #[serde(rename = "type")]
    pub kind: String,
    /// When evaluation reached a terminal state.
    #[serde(default)]
    pub completed_at: Option<Timestamp>,
    /// What the agent should produce.
    pub description: String,
    /// The grader's verdict text from the most recent evaluation.
    #[serde(default)]
    pub explanation: Option<String>,
    /// 0-indexed revision cycle.
    pub iteration: u64,
    /// `outc_…` ID.
    pub outcome_id: String,
    /// Current evaluation state. Documented as a string.
    pub result: SessionOutcomeResult,
}

claude_api_core::string_enum! {
    /// Outcome evaluation state or verdict. The API documents these as plain strings; the values
    /// are the ones listed in the field descriptions.
    pub enum SessionOutcomeResult {
        /// `pending`: the agent has not started.
        Pending = "pending",
        /// `running`: producing or revising.
        Running = "running",
        /// `evaluating`: the grader is scoring.
        Evaluating = "evaluating",
        /// `needs_revision`: criteria not met; another cycle follows.
        NeedsRevision = "needs_revision",
        /// `satisfied`: criteria met (terminal).
        Satisfied = "satisfied",
        /// `max_iterations_reached`: evaluation budget exhausted (terminal).
        MaxIterationsReached = "max_iterations_reached",
        /// `failed`: the rubric does not apply (terminal).
        Failed = "failed",
        /// `interrupted`: interrupted by the user (terminal).
        Interrupted = "interrupted",
    }
}

/// Session timing statistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionStats {
    /// Seconds spent `running`, summed over threads.
    #[serde(default)]
    pub active_seconds: Option<f64>,
    /// Seconds since creation (frozen once terminated).
    #[serde(default)]
    pub duration_seconds: Option<f64>,
}

/// Cumulative usage of a session or thread (also the `session.usage` snapshot).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionUsage {
    /// Seconds with at least one thread running (for a thread: its own running time). The
    /// duration runtime cost is priced on.
    #[serde(default)]
    pub active_seconds: Option<f64>,
    /// Prompt-cache writes by cache lifetime.
    #[serde(default)]
    pub cache_creation: Option<SessionCacheCreationUsage>,
    /// Tokens read from the prompt cache.
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Tracked list cost.
    #[serde(default)]
    pub list_cost: Option<SessionMonetaryAmount>,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// Server-executed tool calls.
    #[serde(default)]
    pub server_tool_use: Option<SessionServerToolUsage>,
}

/// Prompt-cache writes by cache lifetime.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCacheCreationUsage {
    /// Tokens written to 1-hour cache entries.
    #[serde(default)]
    pub ephemeral_1h_input_tokens: Option<u64>,
    /// Tokens written to 5-minute cache entries.
    #[serde(default)]
    pub ephemeral_5m_input_tokens: Option<u64>,
}

/// Server-executed tool calls.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionServerToolUsage {
    /// Web fetch requests.
    #[serde(default)]
    pub web_fetch_requests: Option<u64>,
    /// Web search requests.
    #[serde(default)]
    pub web_search_requests: Option<u64>,
}

tagged_union! {
    /// A resource mounted into a session, discriminated by `type`.
    #[derive(Eq)]
    pub enum SessionResource {
        /// `github_repository`.
        GitHubRepository(SessionGitHubRepositoryResource) = "github_repository",
        /// `file`.
        File(SessionFileResource) = "file",
        /// `memory_store`.
        MemoryStore(SessionMemoryStoreResource) = "memory_store",
    }
}

/// A GitHub repository mounted into the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGitHubRepositoryResource {
    /// `sesrsc_…` ID.
    pub id: String,
    /// When the resource was added.
    pub created_at: Timestamp,
    /// Mount path in the container.
    pub mount_path: String,
    /// When the resource was last updated.
    pub updated_at: Timestamp,
    /// Repository URL.
    pub url: String,
    /// Branch or commit checked out.
    #[serde(default)]
    pub checkout: Option<SessionCheckout>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A file mounted into the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFileResource {
    /// `sesrsc_…` ID.
    pub id: String,
    /// When the resource was added.
    pub created_at: Timestamp,
    /// The file.
    pub file_id: String,
    /// Mount path in the container.
    pub mount_path: String,
    /// When the resource was last updated.
    pub updated_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A memory store attached to the session. The reference documents no `id` or timestamps for this
/// resource type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMemoryStoreResource {
    /// `memstore_…` ID.
    pub memory_store_id: String,
    /// Access mode.
    #[serde(default)]
    pub access: Option<SessionMemoryStoreAccess>,
    /// Store description, snapshotted at attach time (empty when the store has none).
    #[serde(default)]
    pub description: Option<String>,
    /// Per-attachment guidance for the agent.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Mount path in the container, for example `/mnt/memory/user-preferences`.
    #[serde(default)]
    pub mount_path: Option<String>,
    /// Store name, snapshotted at attach time.
    #[serde(default)]
    pub name: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Access mode of an attached memory store.
    pub enum SessionMemoryStoreAccess {
        /// `read_write`.
        ReadWrite = "read_write",
        /// `read_only`.
        ReadOnly = "read_only",
    }
}

tagged_union! {
    /// What a repository resource checks out.
    #[derive(Eq)]
    pub enum SessionCheckout {
        /// `branch`.
        Branch(SessionBranchCheckout) = "branch",
        /// `commit`.
        Commit(SessionCommitCheckout) = "commit",
    }
}

/// A branch checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBranchCheckout {
    /// Branch name.
    pub name: String,
}

/// A commit checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCommitCheckout {
    /// Full commit SHA.
    pub sha: String,
}

/// An execution thread of a session: the primary thread, or a child spawned by the coordinator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionThread {
    /// Always `session_thread`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `sthr_…` ID.
    pub id: String,
    /// What the thread runs.
    pub agent: SessionRosterAgent,
    /// When the thread was archived.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the thread was created.
    pub created_at: Timestamp,
    /// The thread that spawned this one; `null` for the primary thread.
    #[serde(default)]
    pub parent_thread_id: Option<String>,
    /// The session.
    pub session_id: String,
    /// Timing statistics.
    #[serde(default)]
    pub stats: Option<SessionThreadStats>,
    /// Current status.
    pub status: SessionThreadStatus,
    /// When the thread was last updated.
    pub updated_at: Timestamp,
    /// Cumulative usage of this thread.
    #[serde(default)]
    pub usage: Option<SessionUsage>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Session thread status.
    pub enum SessionThreadStatus {
        /// `running`.
        Running = "running",
        /// `idle`.
        Idle = "idle",
        /// `rescheduling`.
        Rescheduling = "rescheduling",
        /// `terminated`.
        Terminated = "terminated",
    }
}

/// Thread timing statistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionThreadStats {
    /// Seconds spent running.
    #[serde(default)]
    pub active_seconds: Option<f64>,
    /// Seconds since creation (frozen once archived).
    #[serde(default)]
    pub duration_seconds: Option<f64>,
    /// Seconds until the thread began running (zero for child threads).
    #[serde(default)]
    pub startup_seconds: Option<f64>,
}

const SESSIONS: &str = "v1/sessions";

impl ManagedAgentsClient {
    /// `GET /v1/sessions`: lists sessions, newest first by default.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/list)
    pub fn sessions(&self) -> ListSessions {
        ListSessions { inner: ListRequest::new(self.api.clone(), self.options(), Ok(ApiPath::new(SESSIONS)), None) }
    }

    /// `GET /v1/sessions/{session_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/retrieve)
    pub async fn session(&self, session_id: &str) -> Result<ApiResponse<Session>> {
        get(&self.api, ApiPath::new(SESSIONS).id(session_id), &[], &self.options()).await
    }

    /// `GET /v1/sessions/{session_id}/events`: the session's event history, oldest first by
    /// default. Events are sensitive (see the [crate docs](crate#sensitive-data)).
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/events/list)
    pub fn session_events(&self, session_id: &str) -> ListSessionEvents {
        let path = ApiPath::new(SESSIONS).id(session_id).map(|path| path.then("events"));
        ListSessionEvents { inner: ListRequest::new(self.api.clone(), self.options(), path, None) }
    }

    /// `GET /v1/sessions/{session_id}/resources`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/resources/list)
    pub fn session_resources(&self, session_id: &str) -> ListSessionResources {
        let path = ApiPath::new(SESSIONS).id(session_id).map(|path| path.then("resources"));
        ListSessionResources { inner: ListRequest::new(self.api.clone(), self.options(), path, Some(1000)) }
    }

    /// `GET /v1/sessions/{session_id}/resources/{resource_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/resources/retrieve)
    pub async fn session_resource(&self, session_id: &str, resource_id: &str) -> Result<ApiResponse<SessionResource>> {
        let path = ApiPath::new(SESSIONS).id(session_id).and_then(|path| path.then("resources").id(resource_id));
        get(&self.api, path, &[], &self.options()).await
    }

    /// `GET /v1/sessions/{session_id}/threads`: the primary thread first, then children in spawn
    /// order.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/threads/list)
    pub fn session_threads(&self, session_id: &str) -> ListSessionThreads {
        let path = ApiPath::new(SESSIONS).id(session_id).map(|path| path.then("threads"));
        ListSessionThreads { inner: ListRequest::new(self.api.clone(), self.options(), path, None) }
    }

    /// `GET /v1/sessions/{session_id}/threads/{thread_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/threads/retrieve)
    pub async fn session_thread(&self, session_id: &str, thread_id: &str) -> Result<ApiResponse<SessionThread>> {
        let path = ApiPath::new(SESSIONS).id(session_id).and_then(|path| path.then("threads").id(thread_id));
        get(&self.api, path, &[], &self.options()).await
    }

    /// `GET /v1/sessions/{session_id}/threads/{thread_id}/events`: one thread's events. Events are
    /// sensitive (see the [crate docs](crate#sensitive-data)).
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/sessions/threads/events/list)
    pub fn session_thread_events(&self, session_id: &str, thread_id: &str) -> ListSessionThreadEvents {
        let path = ApiPath::new(SESSIONS)
            .id(session_id)
            .and_then(|path| path.then("threads").id(thread_id))
            .map(|path| path.then("events"));
        ListSessionThreadEvents { inner: ListRequest::new(self.api.clone(), self.options(), path, None) }
    }
}

/// Request builder for `GET /v1/sessions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSessions {
    inner: ListRequest,
}

impl ListSessions {
    list_methods!(Session, SessionStream, "Page size. The reference documents no bounds.");

    /// `agent_id`: sessions created with this agent.
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.inner.set("agent_id", agent_id);
        self
    }

    /// `agent_version`: only with [`Self::agent_id`] (checked before sending).
    pub fn agent_version(mut self, version: u32) -> Self {
        self.inner.set("agent_version", version.to_string());
        self
    }

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

    /// `deployment_id`: sessions created by this deployment.
    pub fn deployment_id(mut self, deployment_id: impl Into<String>) -> Self {
        self.inner.set("deployment_id", deployment_id);
        self
    }

    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    /// `memory_store_id`: sessions with this memory store among their resources.
    pub fn memory_store_id(mut self, memory_store_id: impl Into<String>) -> Self {
        self.inner.set("memory_store_id", memory_store_id);
        self
    }

    /// `order` by `created_at` (server default `desc`).
    pub fn order(mut self, order: SessionOrder) -> Self {
        self.inner.set("order", order.as_str());
        self
    }

    /// Adds a `statuses` filter; repeated values match any of them.
    pub fn status(mut self, status: SessionStatus) -> Self {
        self.inner.push("statuses", status.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        (self.inner.get("agent_version").is_some() && self.inner.get("agent_id").is_none())
            .then(|| "agent_version only applies when agent_id is also set".to_owned())
    }
}

/// Request builder for `GET /v1/sessions/{session_id}/events`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSessionEvents {
    inner: ListRequest,
}

impl ListSessionEvents {
    list_methods!(SessionEvent, SessionEventStream, "Page size. The reference documents no bounds.");

    /// `created_at[gt]`, compared against `processed_at`.
    pub fn created_at_gt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gt]", at.to_string());
        self
    }

    /// `created_at[gte]`, compared against `processed_at`.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lt]`, compared against `processed_at`.
    pub fn created_at_lt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lt]", at.to_string());
        self
    }

    /// `created_at[lte]`, compared against `processed_at`.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `order` by `processed_at` (server default `asc`).
    pub fn order(mut self, order: SessionOrder) -> Self {
        self.inner.set("order", order.as_str());
        self
    }

    /// Adds a `types` filter (an event `type`, for example `agent.tool_use`); repeated values match
    /// any of them.
    pub fn event_type(mut self, event_type: impl Into<String>) -> Self {
        self.inner.push("types", event_type);
        self
    }

    fn check(&self) -> Option<String> {
        None
    }
}

/// Request builder for `GET /v1/sessions/{session_id}/resources`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSessionResources {
    inner: ListRequest,
}

impl ListSessionResources {
    list_methods!(SessionResource, SessionResourceStream, "Page size, 1 to 1000. Omitted: every resource in one page.");

    fn check(&self) -> Option<String> {
        None
    }
}

/// Request builder for `GET /v1/sessions/{session_id}/threads`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSessionThreads {
    inner: ListRequest,
}

impl ListSessionThreads {
    list_methods!(
        SessionThread,
        SessionThreadStream,
        "Page size (server default 1000). The reference documents no bounds."
    );

    fn check(&self) -> Option<String> {
        None
    }
}

/// Request builder for `GET /v1/sessions/{session_id}/threads/{thread_id}/events`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSessionThreadEvents {
    inner: ListRequest,
}

impl ListSessionThreadEvents {
    list_methods!(SessionEvent, SessionEventStream, "Page size. The reference documents no bounds.");

    fn check(&self) -> Option<String> {
        None
    }
}
