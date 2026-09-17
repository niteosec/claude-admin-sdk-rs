//! Unofficial, read-only Rust client for Anthropic's
//! [Claude Compliance API](https://platform.claude.com/docs/en/manage-claude/compliance-api).
//!
//! > Not affiliated with or endorsed by Anthropic.
//!
//! Version 0.1 covers the **Activity Feed** (`GET /v1/compliance/activities`), verified against a
//! live organization on 2026-09-17. The directory, settings, chat and session endpoints need a Claude
//! Enterprise Compliance Access Key and land once they are verified the same way.
//!
//! ```no_run
//! use claude_compliance::{ComplianceClient, activity_types};
//! use futures::TryStreamExt;
//!
//! # async fn run() -> claude_compliance::Result<()> {
//! let client = ComplianceClient::new(std::env::var("ANTHROPIC_COMPLIANCE_KEY").unwrap())?;
//!
//! // One page, newest first.
//! let page = client.activities().limit(100).send().await?;
//! for activity in &page.body.data {
//!     println!("{} {} {}", activity.created_at, activity.activity_type, activity.actor.actor_type());
//! }
//!
//! // Every activity, walking older pages until `has_more` is false. Skip the feed's record of our
//! // own reads.
//! let mut all = client.activities().limit(1000).stream();
//! while let Some(activity) = all.try_next().await? {
//!     if activity.activity_type == activity_types::COMPLIANCE_API_ACCESSED {
//!         continue;
//!     }
//!     println!("{}", activity.id);
//! }
//! # Ok(()) }
//! ```
//!
//! ## Read-only by construction
//!
//! The Compliance API also exposes permanent, immediate DELETE endpoints for chats, files and
//! projects. This crate does not implement them, so a key handed to software built on it cannot
//! destroy customer data through it.

mod activities;
mod actor;
mod chats;
mod code;
mod content_common;
mod files;
mod groups;
mod organizations;
mod projects;
mod sessions;
mod settings;

pub use activities::{Activity, ActivityPage, ActivityStream, ComplianceApiAccess, ListActivities, activity_types};
pub use actor::{
    Actor, AdminApiKeyActor, AnthropicActor, ApiActor, ScimDirectorySyncActor, UnauthenticatedUserActor, UserActor,
};
pub use chats::{
    Chat, ChatMessage, ChatMessageStream, ChatMessages, ChatOrderBy, ChatPage, ChatStream, ContentBlock,
    ListChatMessages, ListChats, MessageArtifact, MessageFile, MessageGeneratedFile, MessageOrder, MessageRole,
    TextBlock, ToolResultBlock, ToolResultItem, ToolResultText, ToolUseBlock,
};
pub use claude_api_core::{
    ApiClient, ApiError, ApiErrorKind, ApiKey, ApiResponse, ByteStream, ClientConfig, Cursor, CursorPage, Download,
    Error, KeyKind, PageToken, RateLimit, ResponseMeta, Result, RetryPolicy, TokenPage,
};
pub use code::{
    CodeArtifact, CodeArtifactPage, CodeArtifactReadMode, CodeArtifactStream, CodeArtifactVersion, ListCodeArtifacts,
};
pub use content_common::ContentUser;
pub use files::{Artifact, ChatFile, GeneratedFile};
pub use groups::{
    Group, GroupMember, GroupMemberPage, GroupMemberStream, GroupPage, GroupSourceType, GroupStream, ListGroupMembers,
    ListGroups,
};
pub use organizations::{
    ListOrganizationUsers, ListOrganizations, ListRolePermissions, ListRoles, Organization, OrganizationPage,
    OrganizationRole, OrganizationStream, OrganizationUser, OrganizationUserPage, OrganizationUserStream, Role,
    RolePage, RolePermission, RolePermissionPage, RolePermissionStream, RoleStream,
};
pub use projects::{
    ListProjectAttachments, ListProjectCollaborators, ListProjects, Project, ProjectAttachment, ProjectAttachmentPage,
    ProjectAttachmentStream, ProjectCollaborator, ProjectCollaboratorPage, ProjectCollaboratorStream, ProjectDetails,
    ProjectDocAttachment, ProjectDocument, ProjectDocumentMetadata, ProjectFileAttachment, ProjectGroupCollaborator,
    ProjectOrganizationCollaborator, ProjectOrganizationRoleCollaborator, ProjectPage, ProjectRole, ProjectStream,
    ProjectUserCollaborator,
};
pub use sessions::{
    ContentUnavailable, ContentUnavailableReason, ListLocalSessionMessages, ListLocalSessions,
    ListRemoteSessionMessages, ListRemoteSessions, LocalProductSurface, LocalSession, LocalSessionMessage,
    LocalSessionMessageStream, LocalSessionMessagesPage, LocalSessionPage, LocalSessionStream, Provenance,
    RemoteProductSurface, RemoteSession, RemoteSessionMessage, RemoteSessionMessageStream, RemoteSessionMessagesPage,
    RemoteSessionPage, RemoteSessionStatus, RemoteSessionStream, SessionContentBlock, SessionMessageOrder,
    SessionMessageRole, SessionTextBlock, SessionToolResultBlock, SessionToolResultItem, SessionToolUseBlock,
    SessionUser,
};
pub use settings::{
    BooleanSetting, BooleanSettingName, ComplianceApiKey, DataRetentionSetting, EffectiveOrganizationSettings,
    FixedRetention, IndefiniteRetention, IntegerSetting, IntegerSettingName, ProvisioningMode, ProvisioningModeSetting,
    RetentionPeriod, RetentionTimescale, Setting, StringListSetting, StringListSettingName, StringSetting,
    StringSettingName,
};

/// Client for the Compliance API.
#[derive(Debug, Clone)]
pub struct ComplianceClient {
    api: ApiClient,
}

impl ComplianceClient {
    /// A client with the default configuration.
    ///
    /// A Compliance Access Key (created in claude.ai, Claude Enterprise) reaches every endpoint its
    /// scopes allow. A Claude Console Admin API key reaches the Activity Feed only.
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

    /// `GET /v1/compliance/activities`. Requires `read:compliance_activities`.
    pub fn activities(&self) -> ListActivities {
        ListActivities::new(self.api.clone())
    }
}
