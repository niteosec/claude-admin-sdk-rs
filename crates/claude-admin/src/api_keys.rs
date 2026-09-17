//! API keys in the organization. Metadata only: the API never returns a key's secret.
//!
//! - `GET /v1/organizations/api_keys`
//! - `GET /v1/organizations/api_keys/{api_key_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Result, string_enum};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{CursorList, cursor_list_methods};
use crate::union::tagged_union;

string_enum! {
    /// API key status.
    pub enum ApiKeyStatus {
        /// `active`.
        Active = "active",
        /// `archived`.
        Archived = "archived",
        /// `expired`.
        Expired = "expired",
        /// `inactive`.
        Inactive = "inactive",
    }
}

string_enum! {
    /// The kind of actor that created a key.
    pub enum CreatorType {
        /// `service_account`.
        ServiceAccount = "service_account",
        /// `user`.
        User = "user",
    }
}

/// An API key's metadata. Named `OrganizationApiKey` to stay distinct from the credential type
/// [`ApiKey`](claude_api_core::ApiKey).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationApiKey {
    /// Always `api_key`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `apikey_…` ID.
    pub id: String,
    /// When the key was created.
    pub created_at: Timestamp,
    /// Who created the key; `None` when not recorded (legacy, workload-identity-federated or
    /// system-created keys).
    #[serde(default)]
    pub created_by: Option<ApiKeyCreatedBy>,
    /// When the key expires; `None` if it never does.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// Key name.
    pub name: String,
    /// Partially redacted hint, for example `sk-ant-api03-R2D...igAA`.
    #[serde(default)]
    pub partial_key_hint: Option<String>,
    /// The identity the key acts as; `None` for a workspace key not bound to a principal.
    #[serde(default)]
    pub principal: Option<ApiKeyPrincipal>,
    /// Where the key belongs. Prefer this over the deprecated `workspace_id`.
    pub scope: ApiKeyScope,
    /// Key status.
    pub status: ApiKeyStatus,
    /// **Deprecated** upstream in favour of `scope`: `None` both for the default workspace and for
    /// keys without a workspace.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The actor that created a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyCreatedBy {
    /// Actor kind.
    #[serde(rename = "type")]
    pub creator_type: CreatorType,
    /// Actor ID.
    pub id: String,
}

tagged_union! {
    /// The identity a key acts as, discriminated by `type`.
    pub enum ApiKeyPrincipal {
        /// `user_actor`: a personal key.
        User(UserPrincipal) = "user_actor",
        /// `service_account_actor`: a service account key.
        ServiceAccount(ServiceAccountPrincipal) = "service_account_actor",
    }
}

/// `user_actor` principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPrincipal {
    /// `user_…` ID the key acts as.
    pub user_id: String,
}

/// `service_account_actor` principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceAccountPrincipal {
    /// `svac_…` ID the key acts as.
    pub service_account_id: String,
}

tagged_union! {
    /// Where a key belongs, discriminated by `type`.
    pub enum ApiKeyScope {
        /// `organization`: a principal-bound key with no workspace.
        Organization = "organization",
        /// `workspace`: a key bound to one workspace.
        Workspace(WorkspaceScope) = "workspace",
    }
}

/// `workspace` key scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceScope {
    /// The workspace's real ID, including for the default workspace.
    pub workspace_id: String,
}

/// Request builder for `GET /v1/organizations/api_keys`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListApiKeys {
    inner: CursorList,
}

impl ListApiKeys {
    cursor_list_methods!(OrganizationApiKey, 1000);

    /// `created_by_user_id` filter.
    pub fn created_by_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.inner.params.set("created_by_user_id", user_id);
        self
    }

    /// `status` filter.
    pub fn status(mut self, status: ApiKeyStatus) -> Self {
        self.inner.params.set("status", status.as_str());
        self
    }

    /// `workspace_id` filter. The Default Workspace's ID returns only keys bound to it; keys without
    /// a workspace scope match no `workspace_id` filter.
    pub fn workspace_id(mut self, workspace_id: impl Into<String>) -> Self {
        self.inner.params.set("workspace_id", workspace_id);
        self
    }
}

impl AdminClient {
    /// `GET /v1/organizations/api_keys`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/api_keys/list>
    pub fn api_keys(&self) -> ListApiKeys {
        ListApiKeys { inner: CursorList::new(self.api.clone(), Ok(ApiPath::new("v1/organizations/api_keys")), 1000) }
    }

    /// `GET /v1/organizations/api_keys/{api_key_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/api_keys/retrieve>
    pub async fn api_key(&self, api_key_id: &str) -> Result<ApiResponse<OrganizationApiKey>> {
        self.api.get_json(&ApiPath::new("v1/organizations/api_keys").id(api_key_id)?, &[]).await
    }
}
