//! Groups and their members.
//!
//! - `GET /v1/compliance/groups`
//! - `GET /v1/compliance/groups/{group_id}`
//! - `GET /v1/compliance/groups/{group_id}/members`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::pin::Pin;

use claude_api_core::{ApiClient, ApiPath, ApiResponse, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ComplianceClient;
use crate::organizations::{Paging, TokenList, token_list_methods};

const GROUPS: &str = "v1/compliance/groups";

/// A page of groups.
pub type GroupPage = ApiResponse<TokenPage<Group>>;
/// The stream returned by [`ListGroups::stream`].
pub type GroupStream = Pin<Box<dyn Stream<Item = Result<Group>> + Send + 'static>>;
/// A page of group members.
pub type GroupMemberPage = ApiResponse<TokenPage<GroupMember>>;
/// The stream returned by [`ListGroupMembers::stream`].
pub type GroupMemberStream = Pin<Box<dyn Stream<Item = Result<GroupMember>> + Send + 'static>>;

claude_api_core::string_enum! {
    /// How a group was created.
    pub enum GroupSourceType {
        /// `direct`
        Direct = "direct",
        /// `scim`: provisioned by directory sync.
        Scim = "scim",
    }
}

/// An RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// `rbac_group_…` ID.
    pub id: String,
    /// When the group was created.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Group description.
    pub description: String,
    /// Group name.
    pub name: String,
    /// `rbac_role_…` IDs assigned to the group.
    #[serde(default)]
    pub roles: Option<Vec<String>>,
    /// How the group was created.
    pub source_type: GroupSourceType,
    /// When the group was last updated.
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A member of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMember {
    /// When the membership was created.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Member email address.
    pub email: String,
    /// When the membership was last updated.
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    /// `user_…` ID.
    pub user_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ComplianceClient {
    /// `GET /v1/compliance/groups`.
    pub fn groups(&self) -> ListGroups {
        ListGroups { api: self.api.clone(), name_prefix: None, paging: Paging::default() }
    }

    /// `GET /v1/compliance/groups/{group_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/groups/retrieve).
    pub async fn group(&self, group_id: &str) -> Result<ApiResponse<Group>> {
        self.api.get_json(&ApiPath::new(GROUPS).id(group_id)?, &[]).await
    }

    /// `GET /v1/compliance/groups/{group_id}/members`.
    pub fn group_members(&self, group_id: impl Into<String>) -> ListGroupMembers {
        ListGroupMembers { api: self.api.clone(), group_id: group_id.into(), paging: Paging::default() }
    }
}

/// Request builder for `GET /v1/compliance/groups`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListGroups {
    api: ApiClient,
    name_prefix: Option<String>,
    paging: Paging,
}

impl ListGroups {
    /// `name_prefix`: only groups whose name starts with this.
    pub fn name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.name_prefix = Some(prefix.into());
        self
    }
}

impl TokenList for ListGroups {
    type Record = Group;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(GROUPS))
    }
    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        self.paging.push_limit(&mut query)?;
        if let Some(prefix) = &self.name_prefix {
            query.push(("name_prefix", prefix.clone()));
        }
        self.paging.push_page(&mut query);
        Ok(query)
    }
    fn paging(&mut self) -> &mut Paging {
        &mut self.paging
    }
}

token_list_methods!(
    ListGroups,
    GroupPage,
    GroupStream,
    "500",
    "https://platform.claude.com/docs/en/api/compliance/groups/list"
);

/// Request builder for `GET /v1/compliance/groups/{group_id}/members`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListGroupMembers {
    api: ApiClient,
    group_id: String,
    paging: Paging,
}

impl TokenList for ListGroupMembers {
    type Record = GroupMember;
    fn api(&self) -> &ApiClient {
        &self.api
    }
    fn path(&self) -> Result<ApiPath> {
        Ok(ApiPath::new(GROUPS).id(&self.group_id)?.then("members"))
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
    ListGroupMembers,
    GroupMemberPage,
    GroupMemberStream,
    "500",
    "https://platform.claude.com/docs/en/api/compliance/groups/members/list"
);
