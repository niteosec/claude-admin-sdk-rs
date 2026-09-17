//! Custom roles and groups (Claude Enterprise only).
//!
//! - `GET /v1/organizations/rbac_roles`
//! - `GET /v1/organizations/rbac_roles/{role_id}`
//! - `GET /v1/organizations/rbac_roles/{role_id}/permissions`
//! - `GET /v1/organizations/rbac_groups`
//! - `GET /v1/organizations/rbac_groups/{group_id}`
//! - `GET /v1/organizations/rbac_groups/{group_id}/members`
//!
//! Roles need `read:members`; groups need `read:rbac_groups` (either works with `read:org_audit`).
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Result, string_enum};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};
use crate::union::tagged_union;

const ROLES: &str = "v1/organizations/rbac_roles";
const GROUPS: &str = "v1/organizations/rbac_groups";

/// A custom role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRole {
    /// Always `rbac_role`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `rbac_role_…` ID.
    pub id: String,
    /// When the role was created.
    pub created_at: Timestamp,
    /// Role name.
    pub name: String,
    /// When the role was last updated.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One permission a role grants.
///
/// A blanket `organization` grant (`capability_access_all`, `capability_access_all_ga`) covers every
/// product-feature entitlement in its set; count it as such or a role's access is under-reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRolePermission {
    /// Always `rbac_role_permission`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// The granted action; its vocabulary depends on the resource kind (for example `chat`,
    /// `permission_*`, `use`, `always_allow`, `grant`, `interactive`, `managed`).
    pub action: String,
    /// What the permission applies to.
    pub resource: PermissionResource,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// A permission's resource, discriminated by `type`.
    pub enum PermissionResource {
        /// `organization`.
        Organization(OrganizationResource) = "organization",
        /// `connector_tool`.
        ConnectorTool(ConnectorToolResource) = "connector_tool",
        /// `connector_scope`.
        ConnectorScope(ConnectorScopeResource) = "connector_scope",
        /// `connector`.
        Connector(ConnectorResource) = "connector",
        /// `all_connectors`.
        AllConnectors = "all_connectors",
    }
}

/// `organization` resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationResource {
    /// Organization UUID.
    pub organization_id: String,
}

/// `connector_tool` resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorToolResource {
    /// Connector ID.
    pub connector_id: String,
    /// Published tool name, or a server-encoded `{prefix}_{32-hex}` form from which the name is not
    /// recoverable.
    pub tool_name: String,
}

/// `connector_scope` resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorScopeResource {
    /// Connector ID.
    pub connector_id: String,
    /// OAuth scope, usually in the server-encoded `{prefix}_{32-hex}` form.
    pub scope: String,
}

/// `connector` resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorResource {
    /// Connector ID.
    pub connector_id: String,
}

string_enum! {
    /// How a group was created.
    pub enum GroupSourceType {
        /// `direct`: created in the organization's admin settings.
        Direct = "direct",
        /// `scim`: provisioned by the identity provider.
        Scim = "scim",
    }
}

/// A group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacGroup {
    /// Always `rbac_group`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `rbac_group_…` ID.
    pub id: String,
    /// When the group was created.
    pub created_at: Timestamp,
    /// Group name; not unique.
    pub name: String,
    /// Attached role IDs. `None` means role data was temporarily unavailable; retry to tell it apart
    /// from an empty list.
    #[serde(default)]
    pub roles: Option<Vec<String>>,
    /// How the group was created.
    pub source_type: GroupSourceType,
    /// When the group was last updated.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A group member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacGroupMember {
    /// Always `rbac_group_member`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// When the user was added to the group.
    pub created_at: Timestamp,
    /// Email address.
    pub email: String,
    /// `rbac_group_…` ID.
    pub group_id: String,
    /// `user_…` ID.
    pub user_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/organizations/rbac_roles`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRbacRoles {
    inner: TokenList,
}

impl ListRbacRoles {
    token_list_methods!(RbacRole, "Page size, 1 to 1000 (server default 20).");
}

/// Request builder for `GET /v1/organizations/rbac_roles/{role_id}/permissions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRbacRolePermissions {
    inner: TokenList,
}

impl ListRbacRolePermissions {
    token_list_methods!(RbacRolePermission, "Page size, 1 to 1000 (server default 20).");
}

/// Request builder for `GET /v1/organizations/rbac_groups`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRbacGroups {
    inner: TokenList,
}

impl ListRbacGroups {
    token_list_methods!(RbacGroup, "Page size, 1 to 1000 (server default 20).");
}

/// Request builder for `GET /v1/organizations/rbac_groups/{group_id}/members`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRbacGroupMembers {
    inner: TokenList,
}

impl ListRbacGroupMembers {
    token_list_methods!(RbacGroupMember, "Page size, 1 to 1000 (server default 20).");
}

impl AdminClient {
    /// `GET /v1/organizations/rbac_roles`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_roles/list>
    pub fn rbac_roles(&self) -> ListRbacRoles {
        ListRbacRoles { inner: TokenList::new(self.api.clone(), Ok(ApiPath::new(ROLES)), 1000) }
    }

    /// `GET /v1/organizations/rbac_roles/{role_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_roles/retrieve>
    pub async fn rbac_role(&self, role_id: &str) -> Result<ApiResponse<RbacRole>> {
        self.api.get_json(&ApiPath::new(ROLES).id(role_id)?, &[]).await
    }

    /// `GET /v1/organizations/rbac_roles/{role_id}/permissions`. An invalid `role_id` is reported
    /// when the request is sent.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_roles/permissions/list>
    pub fn rbac_role_permissions(&self, role_id: &str) -> ListRbacRolePermissions {
        let path = ApiPath::new(ROLES).id(role_id).map(|path| path.then("permissions"));
        ListRbacRolePermissions { inner: TokenList::new(self.api.clone(), path, 1000) }
    }

    /// `GET /v1/organizations/rbac_groups`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_groups/list>
    pub fn rbac_groups(&self) -> ListRbacGroups {
        ListRbacGroups { inner: TokenList::new(self.api.clone(), Ok(ApiPath::new(GROUPS)), 1000) }
    }

    /// `GET /v1/organizations/rbac_groups/{group_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_groups/retrieve>
    pub async fn rbac_group(&self, group_id: &str) -> Result<ApiResponse<RbacGroup>> {
        self.api.get_json(&ApiPath::new(GROUPS).id(group_id)?, &[]).await
    }

    /// `GET /v1/organizations/rbac_groups/{group_id}/members`. An invalid `group_id` is reported when
    /// the request is sent.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/rbac_groups/members/list>
    pub fn rbac_group_members(&self, group_id: &str) -> ListRbacGroupMembers {
        let path = ApiPath::new(GROUPS).id(group_id).map(|path| path.then("members"));
        ListRbacGroupMembers { inner: TokenList::new(self.api.clone(), path, 1000) }
    }
}
