//! Organizations, their users, and their roles.
//!
//! - `GET /v1/compliance/organizations`
//! - `GET /v1/compliance/organizations/{org_uuid}/users`
//! - `GET /v1/compliance/organizations/{org_uuid}/roles`
//! - `GET /v1/compliance/organizations/{org_uuid}/roles/{role_id}`
//! - `GET /v1/compliance/organizations/{org_uuid}/roles/{role_id}/permissions`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Error, PageToken, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ComplianceClient;

const ORGANIZATIONS: &str = "v1/compliance/organizations";
const MAX_LIMIT: u32 = 1000;

/// A page of organizations.
pub type OrganizationPage = ApiResponse<TokenPage<Organization>>;
/// The stream returned by [`ListOrganizations::stream`].
pub type OrganizationStream = Pin<Box<dyn Stream<Item = Result<Organization>> + Send + 'static>>;
/// A page of organization users.
pub type OrganizationUserPage = ApiResponse<TokenPage<OrganizationUser>>;
/// The stream returned by [`ListOrganizationUsers::stream`].
pub type OrganizationUserStream = Pin<Box<dyn Stream<Item = Result<OrganizationUser>> + Send + 'static>>;
/// A page of roles.
pub type RolePage = ApiResponse<TokenPage<Role>>;
/// The stream returned by [`ListRoles::stream`].
pub type RoleStream = Pin<Box<dyn Stream<Item = Result<Role>> + Send + 'static>>;
/// A page of role permissions.
pub type RolePermissionPage = ApiResponse<TokenPage<RolePermission>>;
/// The stream returned by [`ListRolePermissions::stream`].
pub type RolePermissionStream = Pin<Box<dyn Stream<Item = Result<RolePermission>> + Send + 'static>>;

/// An organization under the parent organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Creation time (RFC 3339). Kept as a string: the reference gives no `date-time` format here.
    pub created_at: String,
    /// Organization name.
    pub name: String,
    /// Organization UUID.
    pub uuid: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// A user's built-in organization role, distinct from custom RBAC roles.
    pub enum OrganizationRole {
        /// `admin`
        Admin = "admin",
        /// `billing`
        Billing = "billing",
        /// `claude_code_user`
        ClaudeCodeUser = "claude_code_user",
        /// `developer`
        Developer = "developer",
        /// `managed`
        Managed = "managed",
        /// `membership_admin`
        MembershipAdmin = "membership_admin",
        /// `owner`
        Owner = "owner",
        /// `parent_org_admin`
        ParentOrgAdmin = "parent_org_admin",
        /// `parent_org_owner`
        ParentOrgOwner = "parent_org_owner",
        /// `primary_owner`
        PrimaryOwner = "primary_owner",
        /// `user`
        User = "user",
    }
}

/// A current user member of an organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationUser {
    /// `user_…` ID.
    pub id: String,
    /// When the user account was created.
    pub created_at: Timestamp,
    /// Current email address.
    pub email: String,
    /// Current full name.
    pub full_name: String,
    /// Built-in organization role.
    pub organization_role: OrganizationRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A custom RBAC role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// `rbac_role_…` ID.
    pub id: String,
    /// When the role was created.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Role description.
    pub description: String,
    /// Role name.
    pub name: String,
    /// When the role was last updated.
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One permission granted by a role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePermission {
    /// Action permitted on the resource, for example `claude_code`.
    pub action: String,
    /// Identifier of the resource.
    pub resource_id: String,
    /// Type of the resource, for example `organization`.
    pub resource_type: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ComplianceClient {
    /// `GET /v1/compliance/organizations`. Requires `read:compliance_org_data`.
    pub fn organizations(&self) -> ListOrganizations {
        ListOrganizations { api: self.api.clone(), paging: Paging::default() }
    }

    /// `GET /v1/compliance/organizations/{org_uuid}/users`. Requires `read:compliance_user_data`.
    pub fn organization_users(&self, org_uuid: impl Into<String>) -> ListOrganizationUsers {
        ListOrganizationUsers { api: self.api.clone(), org_uuid: org_uuid.into(), paging: Paging::default() }
    }

    /// `GET /v1/compliance/organizations/{org_uuid}/roles`.
    pub fn organization_roles(&self, org_uuid: impl Into<String>) -> ListRoles {
        ListRoles { api: self.api.clone(), org_uuid: org_uuid.into(), paging: Paging::default() }
    }

    /// `GET /v1/compliance/organizations/{org_uuid}/roles/{role_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/organizations/roles/retrieve).
    pub async fn organization_role(&self, org_uuid: &str, role_id: &str) -> Result<ApiResponse<Role>> {
        let path = ApiPath::new(ORGANIZATIONS).id(org_uuid)?.then("roles").id(role_id)?;
        self.api.get_json(&path, &[]).await
    }

    /// `GET /v1/compliance/organizations/{org_uuid}/roles/{role_id}/permissions`.
    pub fn role_permissions(&self, org_uuid: impl Into<String>, role_id: impl Into<String>) -> ListRolePermissions {
        ListRolePermissions {
            api: self.api.clone(),
            org_uuid: org_uuid.into(),
            role_id: role_id.into(),
            paging: Paging::default(),
        }
    }
}

/// `limit` and `page`, shared by every page-token list in this crate.
#[derive(Debug, Clone, Default)]
pub(crate) struct Paging {
    pub(crate) limit: Option<u32>,
    pub(crate) page: Option<PageToken>,
}

impl Paging {
    pub(crate) fn push_limit(&self, query: &mut Vec<(&'static str, String)>) -> Result<()> {
        if let Some(limit) = self.limit {
            if !(1..=MAX_LIMIT).contains(&limit) {
                return Err(Error::InvalidArgument(format!("limit must be between 1 and {MAX_LIMIT}, got {limit}")));
            }
            query.push(("limit", limit.to_string()));
        }
        Ok(())
    }

    pub(crate) fn push_page(&self, query: &mut Vec<(&'static str, String)>) {
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
    }
}

/// A page-token list request that can be walked page by page.
pub(crate) trait TokenList: Send + Sync + 'static {
    /// The record type.
    type Record: DeserializeOwned + Send + 'static;
    fn api(&self) -> &ApiClient;
    fn path(&self) -> Result<ApiPath>;
    fn query(&self) -> Result<Vec<(&'static str, String)>>;
    fn paging(&mut self) -> &mut Paging;
}

pub(crate) async fn fetch<R: TokenList>(request: &R) -> Result<ApiResponse<TokenPage<R::Record>>> {
    request.api().get_json(&request.path()?, &request.query()?).await
}

/// Walks `next_page` tokens from the request's `page` (or the first page) to the end.
pub(crate) fn walk<R: TokenList>(request: R) -> Pin<Box<dyn Stream<Item = Result<R::Record>> + Send + 'static>> {
    Box::pin(try_stream! {
        let mut request = request;
        loop {
            let page = fetch(&request).await?.body;
            let next = page.next().cloned();
            for record in page.data {
                yield record;
            }
            match next {
                Some(token) => request.paging().page = Some(token),
                None => break,
            }
        }
    })
}

/// Implements the shared `limit` / `page` / `send` / `stream` methods of a page-token list builder.
macro_rules! token_list_methods {
    ($builder:ident, $page:ty, $stream:ty, $default:literal, $reference:literal) => {
        impl $builder {
            #[doc = concat!("Page size, 1 to 1000 (server default ", $default, ").")]
            pub fn limit(mut self, limit: u32) -> Self {
                self.paging.limit = Some(limit);
                self
            }

            /// Start from a page's `next_page` token.
            pub fn page(mut self, token: ::claude_api_core::PageToken) -> Self {
                self.paging.page = Some(token);
                self
            }

            #[doc = concat!("Fetches one page. [Reference](", $reference, ").")]
            pub async fn send(&self) -> ::claude_api_core::Result<$page> {
                $crate::organizations::fetch(self).await
            }

            /// Streams every record, following `next_page` from [`Self::page`] (or the first page)
            /// until the list ends.
            pub fn stream(self) -> $stream {
                $crate::organizations::walk(self)
            }
        }
    };
}
pub(crate) use token_list_methods;

/// Request builder for `GET /v1/compliance/organizations`. Results are oldest first.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListOrganizations {
    api: ApiClient,
    paging: Paging,
}

impl TokenList for ListOrganizations {
    type Record = Organization;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(ORGANIZATIONS))
    }
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        self.paging.push_limit(&mut query)?;
        self.paging.push_page(&mut query);
        Ok(query)
    }
    fn paging(&mut self) -> &mut Paging {
        &mut self.paging
    }
}

token_list_methods!(
    ListOrganizations,
    OrganizationPage,
    OrganizationStream,
    "1000",
    "https://platform.claude.com/docs/en/api/compliance/organizations/list"
);

/// Request builder for `GET /v1/compliance/organizations/{org_uuid}/users`. Results are ordered by
/// organization join date, oldest first.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListOrganizationUsers {
    api: ApiClient,
    org_uuid: String,
    paging: Paging,
}

impl TokenList for ListOrganizationUsers {
    type Record = OrganizationUser;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(ORGANIZATIONS).id(&self.org_uuid)?.then("users"))
    }
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        self.paging.push_limit(&mut query)?;
        self.paging.push_page(&mut query);
        Ok(query)
    }
    fn paging(&mut self) -> &mut Paging {
        &mut self.paging
    }
}

token_list_methods!(
    ListOrganizationUsers,
    OrganizationUserPage,
    OrganizationUserStream,
    "500",
    "https://platform.claude.com/docs/en/api/compliance/organizations/users/list"
);

/// Request builder for `GET /v1/compliance/organizations/{org_uuid}/roles`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRoles {
    api: ApiClient,
    org_uuid: String,
    paging: Paging,
}

impl TokenList for ListRoles {
    type Record = Role;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(ORGANIZATIONS).id(&self.org_uuid)?.then("roles"))
    }
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        self.paging.push_limit(&mut query)?;
        self.paging.push_page(&mut query);
        Ok(query)
    }
    fn paging(&mut self) -> &mut Paging {
        &mut self.paging
    }
}

token_list_methods!(
    ListRoles,
    RolePage,
    RoleStream,
    "500",
    "https://platform.claude.com/docs/en/api/compliance/organizations/roles/list"
);

/// Request builder for `GET /v1/compliance/organizations/{org_uuid}/roles/{role_id}/permissions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRolePermissions {
    api: ApiClient,
    org_uuid: String,
    role_id: String,
    paging: Paging,
}

impl TokenList for ListRolePermissions {
    type Record = RolePermission;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(ORGANIZATIONS).id(&self.org_uuid)?.then("roles").id(&self.role_id)?.then("permissions"))
    }
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        self.paging.push_limit(&mut query)?;
        self.paging.push_page(&mut query);
        Ok(query)
    }
    fn paging(&mut self) -> &mut Paging {
        &mut self.paging
    }
}

token_list_methods!(
    ListRolePermissions,
    RolePermissionPage,
    RolePermissionStream,
    "500",
    "https://platform.claude.com/docs/en/api/compliance/organizations/roles/permissions/list"
);
