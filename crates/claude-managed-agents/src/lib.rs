//! Unofficial, read-only Rust client for Anthropic's
//! [Claude Managed Agents API](https://platform.claude.com/docs/en/managed-agents/overview) (beta).
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! Managed Agents are agents defined and run through the Claude API: an agent's model, system prompt,
//! tools, MCP servers and skills; the vaults holding the credentials those tools use; the
//! environments and deployments that run it; and the sessions, memory and runs it produces. This
//! crate reads that estate. It never creates, updates, archives or deletes anything.
//!
//! Requests carry the `managed-agents-2026-04-01` beta header, plus the extra beta an endpoint family
//! documents (tunnels, user profiles, dreams). Memory-store requests are the exception: they send
//! `agent-memory-2026-07-22` *instead*, because the API rejects the two together with a 400.
//! Resources belong to a workspace: a key bound to one workspace needs nothing more, and a key that
//! spans several selects one with [`ManagedAgentsClient::in_workspace`].
//!
//! # Sensitive data
//!
//! Session events, thread events, memories and memory versions carry conversation content, tool
//! inputs and outputs, and whatever the agent wrote to memory. Nothing is masked by the API. Treat
//! them like transcripts. Vault credential reads never return secret values, and this crate masks
//! token-like fields in `Debug` output regardless.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

mod agents;
mod config_support;
mod deployments;
mod dreams;
mod environments;
mod memory;
mod runtime;
mod sessions;
mod skills;
mod tunnels;
mod user_profiles;
mod vaults;

pub use agents::{
    Agent, AgentAdvisor, AgentBuiltinToolConfig, AgentCoordinator, AgentCustomTool, AgentCustomToolInputSchema,
    AgentEffort, AgentMcpServer, AgentMcpServerUrl, AgentMcpToolConfig, AgentMcpToolset, AgentModelConfig,
    AgentMultiagent, AgentPermissionPolicy, AgentReference, AgentRosterEntry, AgentSkill, AgentSkillReference,
    AgentSpeed, AgentStream, AgentTool, AgentToolConfig, AgentToolset, AgentToolsetDefaultConfig, AgentUserLocation,
    AgentWebFetchToolConfig, AgentWebSearchToolConfig, ListAgentVersions, ListAgents,
};
pub use environments::{
    Environment, EnvironmentCloudConfig, EnvironmentConfig, EnvironmentLimitedNetwork, EnvironmentNetworking,
    EnvironmentPackages, EnvironmentScope, EnvironmentStream, ListEnvironments,
};
pub use skills::{
    ListSkillVersions, ListSkills, Skill, SkillSource, SkillSourceType, SkillStream, SkillVersion, SkillVersionStream,
};
pub use tunnels::{
    ListTunnelCertificates, ListTunnels, MCP_TUNNELS_BETA, Tunnel, TunnelCertificate, TunnelCertificateStream,
    TunnelStream,
};
pub use user_profiles::{
    ListUserProfiles, USER_PROFILES_BETA, UserProfile, UserProfileAccessType, UserProfileAccountStatus,
    UserProfileEntityType, UserProfileExternalUserDetails, UserProfileOrder, UserProfileOrderBy, UserProfileStream,
    UserProfileTrustGrant, UserProfileTrustGrantStatus,
};
pub use vaults::{
    ListVaultCredentials, ListVaults, Vault, VaultCredential, VaultCredentialAuth, VaultCredentialEnvironmentVariable,
    VaultCredentialInjectionLocation, VaultCredentialLimitedNetworking, VaultCredentialMcpOAuth,
    VaultCredentialNetworking, VaultCredentialOAuthRefresh, VaultCredentialPage, VaultCredentialStaticBearer,
    VaultCredentialStream, VaultCredentialTokenEndpointAuth, VaultPage, VaultStream,
};

pub use claude_api_core::{
    ApiClient, ApiError, ApiErrorKind, ApiKey, ApiResponse, ByteStream, ClientConfig, Cursor, CursorPage, Download,
    Error, KeyKind, PageToken, RateLimit, RequestOptions, ResponseMeta, Result, RetryPolicy, TokenPage,
};

pub use deployments::{
    Deployment, DeploymentAgentRef, DeploymentDefineOutcome, DeploymentErrorType, DeploymentFileConfig,
    DeploymentGitHubRepositoryConfig, DeploymentInitialEvent, DeploymentMemoryStoreConfig, DeploymentPausedByError,
    DeploymentPausedReason, DeploymentPausedReasonError, DeploymentResourceConfig, DeploymentRun, DeploymentRunError,
    DeploymentRunStream, DeploymentSchedule, DeploymentScheduleTrigger, DeploymentStatus, DeploymentStream,
    DeploymentSystemMessage, DeploymentTriggerContext, DeploymentTriggerType, DeploymentUserMessage,
    ListDeploymentRuns, ListDeployments,
};
pub use dreams::{
    DREAMING_BETA, Dream, DreamError, DreamInput, DreamMemoryStoreRef, DreamModelConfig, DreamOutput,
    DreamOutputBehavior, DreamSessionsInput, DreamStatus, DreamStream, DreamUsage, ListDreams,
};
pub use memory::{
    AGENT_MEMORY_BETA, GetMemory, GetMemoryVersion, ListMemories, ListMemoryStores, ListMemoryVersions, Memory,
    MemoryActor, MemoryApiActor, MemoryListItem, MemoryListItemStream, MemoryPrefix, MemoryServiceAccountActor,
    MemorySessionActor, MemoryStore, MemoryStoreStream, MemoryUserActor, MemoryVersion, MemoryVersionOperation,
    MemoryVersionStream, MemoryView,
};
pub use runtime::RuntimePage;
pub use sessions::{
    ListSessionEvents, ListSessionResources, ListSessionThreadEvents, ListSessionThreads, ListSessions, Session,
    SessionAdvisor, SessionAgent, SessionAgentCustomToolUseEvent, SessionAgentMcpToolResultEvent,
    SessionAgentMcpToolUseEvent, SessionAgentMessageEvent, SessionAgentTool, SessionAgentToolConfig,
    SessionAgentToolResultEvent, SessionAgentToolUseEvent, SessionAgentToolset, SessionAutoEvaluation,
    SessionAutoJudgement, SessionBase64Source, SessionBasicEvent, SessionBranchCheckout, SessionBudget,
    SessionBuiltinToolConfig, SessionCacheCreationUsage, SessionCheckout, SessionCommitCheckout, SessionContentBlock,
    SessionCredentialErrorDetail, SessionCurrency, SessionCustomTool, SessionCustomToolInputSchema,
    SessionDefineOutcomeEvent, SessionDocumentBlock, SessionDocumentSource, SessionEffort, SessionErrorDetail,
    SessionErrorEvent, SessionEvaluatedPermission, SessionEvent, SessionEventError, SessionEventStream,
    SessionFileResource, SessionFileSource, SessionGitHubRepositoryResource, SessionImageBlock, SessionImageSource,
    SessionJudgementReason, SessionMcpErrorDetail, SessionMcpServer, SessionMcpToolConfig, SessionMcpToolset,
    SessionMemoryStoreAccess, SessionMemoryStoreResource, SessionModelConfig, SessionModelRequestEndEvent,
    SessionModelSpeed, SessionModelUsage, SessionMonetaryAmount, SessionMultiagent, SessionOrder,
    SessionOutcomeEvaluation, SessionOutcomeEvaluationEndEvent, SessionOutcomeEvaluationProgressEvent,
    SessionOutcomeResult, SessionPage, SessionPermissionPolicy, SessionRequiresAction, SessionResource,
    SessionResourceStream, SessionRetryStatus, SessionRosterAgent, SessionRubric, SessionSearchResultBlock,
    SessionSearchResultCitations, SessionServerToolUsage, SessionSkill, SessionSkillVersion, SessionStats,
    SessionStatus, SessionStatusIdleEvent, SessionStopReason, SessionStream, SessionSystemMessageEvent,
    SessionTextBlock, SessionTextContent, SessionTextRubric, SessionThread, SessionThreadAgent,
    SessionThreadLifecycleEvent, SessionThreadMessageReceivedEvent, SessionThreadMessageSentEvent, SessionThreadStats,
    SessionThreadStatus, SessionThreadStatusIdleEvent, SessionThreadStream, SessionToolConfirmationResult,
    SessionToolDefaultConfig, SessionToolEvaluation, SessionUpdatedEvent, SessionUrlSource, SessionUsage,
    SessionUsageEvent, SessionUserCustomToolResultEvent, SessionUserInterruptEvent, SessionUserLocation,
    SessionUserMessageEvent, SessionUserToolConfirmationEvent, SessionUserToolResultEvent, SessionWebFetchToolConfig,
    SessionWebSearchToolConfig,
};

/// The beta the Managed Agents endpoints require (memory stores use their own; see the crate docs).
pub const MANAGED_AGENTS_BETA: &str = "managed-agents-2026-04-01";

/// Client for the Managed Agents API.
#[derive(Debug, Clone)]
pub struct ManagedAgentsClient {
    api: ApiClient,
    workspace_id: Option<String>,
}

impl ManagedAgentsClient {
    /// A client with the default configuration, for a regular Claude API key.
    pub fn new(key: impl Into<String>) -> Result<Self> {
        Ok(Self::from_api_client(ApiClient::new(ApiKey::new(key))?))
    }

    /// A client over a configured [`ApiClient`] (custom base URL, retries, HTTP client). The beta
    /// header is added per request, so the configuration does not need it.
    pub fn from_api_client(api: ApiClient) -> Self {
        Self { api, workspace_id: None }
    }

    /// A copy of this client whose requests target `workspace_id` (`anthropic-workspace-id`), for
    /// keys that span several workspaces. Clones share the connection pool and rate-limit state.
    pub fn in_workspace(&self, workspace_id: impl Into<String>) -> Self {
        Self { api: self.api.clone(), workspace_id: Some(workspace_id.into()) }
    }

    /// The workspace requests target, when one was selected.
    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }

    /// The underlying transport.
    pub fn api_client(&self) -> &ApiClient {
        &self.api
    }

    /// Headers for every request: the Managed Agents beta and the selected workspace.
    pub(crate) fn options(&self) -> RequestOptions {
        let options = RequestOptions::default().beta(MANAGED_AGENTS_BETA);
        match &self.workspace_id {
            Some(workspace_id) => options.workspace_id(workspace_id.clone()),
            None => options,
        }
    }
}
