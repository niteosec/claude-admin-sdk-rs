//! Workspaces and their members.
//!
//! - `GET /v1/organizations/workspaces`
//! - `GET /v1/organizations/workspaces/{workspace_id}`
//! - `GET /v1/organizations/workspaces/{workspace_id}/members`
//! - `GET /v1/organizations/workspaces/{workspace_id}/members/{user_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, ApiResponse, Result, string_enum};
use jiff::Timestamp;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{CursorList, cursor_list_methods};

string_enum! {
    /// A workspace role.
    pub enum WorkspaceRole {
        /// `workspace_admin`.
        WorkspaceAdmin = "workspace_admin",
        /// `workspace_billing`.
        WorkspaceBilling = "workspace_billing",
        /// `workspace_developer`.
        WorkspaceDeveloper = "workspace_developer",
        /// `workspace_restricted_developer`.
        WorkspaceRestrictedDeveloper = "workspace_restricted_developer",
        /// `workspace_user`.
        WorkspaceUser = "workspace_user",
    }
}

string_enum! {
    /// An inference geo a workspace may allow or default to.
    pub enum InferenceGeo {
        /// `global`.
        Global = "global",
        /// `us`.
        Us = "us",
    }
}

string_enum! {
    /// Where workspace data is stored.
    pub enum WorkspaceGeo {
        /// `us`.
        Us = "us",
    }
}

/// A workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// Always `workspace`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `wrkspc_…` ID.
    pub id: String,
    /// When the workspace was archived; `None` when it is not.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// The workspace's encryption compartment, referenced by AWS CMEK key policies.
    pub compartment_id: String,
    /// When the workspace was created.
    pub created_at: Timestamp,
    /// Data residency configuration.
    pub data_residency: DataResidency,
    /// Hex color shown in the Console.
    pub display_color: String,
    /// The customer-managed encryption key configuration attached to the workspace (write-once).
    #[serde(default)]
    pub external_key_id: Option<String>,
    /// Workspace name.
    pub name: String,
    /// User-defined tags.
    pub tags: BTreeMap<String, String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A workspace's data residency configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataResidency {
    /// Permitted inference geos.
    pub allowed_inference_geos: AllowedInferenceGeos,
    /// Inference geo applied when a request omits it.
    pub default_inference_geo: InferenceGeo,
    /// Storage region; immutable after creation.
    pub workspace_geo: WorkspaceGeo,
}

/// `allowed_inference_geos`: a list of geos, or the string `unrestricted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedInferenceGeos {
    /// `"unrestricted"`: every geo is allowed.
    Unrestricted,
    /// The allowed geos.
    Geos(Vec<InferenceGeo>),
    /// Any other shape, kept as received.
    Other(Value),
}

impl<'de> Deserialize<'de> for AllowedInferenceGeos {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        match raw {
            Value::String(ref value) if value == "unrestricted" => Ok(Self::Unrestricted),
            Value::Array(_) => serde_json::from_value(raw).map(Self::Geos).map_err(D::Error::custom),
            other => Ok(Self::Other(other)),
        }
    }
}

impl Serialize for AllowedInferenceGeos {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unrestricted => serializer.serialize_str("unrestricted"),
            Self::Geos(geos) => geos.serialize(serializer),
            Self::Other(raw) => raw.serialize(serializer),
        }
    }
}

/// A workspace member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMember {
    /// Always `workspace_member`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `user_…` ID.
    pub user_id: String,
    /// `wrkspc_…` ID.
    pub workspace_id: String,
    /// Role in the workspace.
    pub workspace_role: WorkspaceRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/organizations/workspaces`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListWorkspaces {
    inner: CursorList,
}

impl ListWorkspaces {
    cursor_list_methods!(Workspace, 1000);

    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.params.set("include_archived", include.to_string());
        self
    }
}

/// Request builder for `GET /v1/organizations/workspaces/{workspace_id}/members`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListWorkspaceMembers {
    inner: CursorList,
}

impl ListWorkspaceMembers {
    cursor_list_methods!(WorkspaceMember, 1000);
}

const WORKSPACES: &str = "v1/organizations/workspaces";

impl AdminClient {
    /// `GET /v1/organizations/workspaces`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/workspaces/list>
    pub fn workspaces(&self) -> ListWorkspaces {
        ListWorkspaces { inner: CursorList::new(self.api.clone(), Ok(ApiPath::new(WORKSPACES)), 1000) }
    }

    /// `GET /v1/organizations/workspaces/{workspace_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/workspaces/retrieve>
    pub async fn workspace(&self, workspace_id: &str) -> Result<ApiResponse<Workspace>> {
        self.api.get_json(&ApiPath::new(WORKSPACES).id(workspace_id)?, &[]).await
    }

    /// `GET /v1/organizations/workspaces/{workspace_id}/members`. An invalid `workspace_id` is
    /// reported when the request is sent.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/workspaces/members/list>
    pub fn workspace_members(&self, workspace_id: &str) -> ListWorkspaceMembers {
        let path = ApiPath::new(WORKSPACES).id(workspace_id).map(|path| path.then("members"));
        ListWorkspaceMembers { inner: CursorList::new(self.api.clone(), path, 1000) }
    }

    /// `GET /v1/organizations/workspaces/{workspace_id}/members/{user_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/workspaces/members/retrieve>
    pub async fn workspace_member(&self, workspace_id: &str, user_id: &str) -> Result<ApiResponse<WorkspaceMember>> {
        let path = ApiPath::new(WORKSPACES).id(workspace_id)?.then("members").id(user_id)?;
        self.api.get_json(&path, &[]).await
    }
}
