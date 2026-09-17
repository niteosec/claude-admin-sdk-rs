//! Organization members and invites.
//!
//! - `GET /v1/organizations/users`
//! - `GET /v1/organizations/users/{user_id}`
//! - `GET /v1/organizations/invites`
//! - `GET /v1/organizations/invites/{invite_id}`
//!
//! Claude Enterprise keys need `read:members` (or `read:org_audit`).
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Result, string_enum};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{CursorList, cursor_list_methods};

string_enum! {
    /// An organization role. Console organizations use `user`, `developer`, `billing`, `admin` and
    /// `claude_code_user`; Claude Enterprise organizations use `user`, `owner`, `primary_owner`,
    /// `membership_admin` and `managed`.
    pub enum OrganizationRole {
        /// `admin`.
        Admin = "admin",
        /// `billing`.
        Billing = "billing",
        /// `claude_code_user`.
        ClaudeCodeUser = "claude_code_user",
        /// `developer`.
        Developer = "developer",
        /// `managed`.
        Managed = "managed",
        /// `membership_admin`.
        MembershipAdmin = "membership_admin",
        /// `owner`.
        Owner = "owner",
        /// `primary_owner`.
        PrimaryOwner = "primary_owner",
        /// `user`.
        User = "user",
    }
}

string_enum! {
    /// Invite status.
    pub enum InviteStatus {
        /// `accepted`.
        Accepted = "accepted",
        /// `deleted`. Not accepted by the `statuses[]` filter.
        Deleted = "deleted",
        /// `expired`.
        Expired = "expired",
        /// `pending`.
        Pending = "pending",
    }
}

/// An organization member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Always `user`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `user_…` ID.
    pub id: String,
    /// When the user joined the organization.
    pub added_at: Timestamp,
    /// Email address.
    pub email: String,
    /// Display name.
    pub name: String,
    /// Organization role.
    pub role: OrganizationRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An organization invite. Invites expire after 21 days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    /// Always `invite`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `invite_…` ID.
    pub id: String,
    /// When the invite was accepted.
    #[serde(default)]
    pub accepted_at: Option<Timestamp>,
    /// Invited email address.
    pub email: String,
    /// When the invite expires.
    pub expires_at: Timestamp,
    /// When the invite was created.
    pub invited_at: Timestamp,
    /// RBAC group IDs assigned on acceptance (Claude Enterprise); empty when none.
    pub rbac_group_ids: Vec<String>,
    /// Role granted on acceptance.
    pub role: OrganizationRole,
    /// Invite status.
    pub status: InviteStatus,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/organizations/users`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListUsers {
    inner: CursorList,
}

impl ListUsers {
    cursor_list_methods!(User, 1000);

    /// `email`: filter by email address.
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.inner.params.set("email", email);
        self
    }

    /// Adds a `roles[]` filter; values are OR'ed.
    pub fn role(mut self, role: OrganizationRole) -> Self {
        self.inner.params.push("roles[]", role.as_str());
        self
    }
}

/// Request builder for `GET /v1/organizations/invites`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListInvites {
    inner: CursorList,
}

impl ListInvites {
    cursor_list_methods!(Invite, 1000);

    /// `email`: filter by invited address (normalized, case-insensitive).
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.inner.params.set("email", email);
        self
    }

    /// Adds a `roles[]` filter; values are OR'ed.
    pub fn role(mut self, role: OrganizationRole) -> Self {
        self.inner.params.push("roles[]", role.as_str());
        self
    }

    /// Adds a `statuses[]` filter (`accepted`, `expired` or `pending`); values are OR'ed. Omit to
    /// return all three.
    pub fn status(mut self, status: InviteStatus) -> Self {
        self.inner.params.push("statuses[]", status.as_str());
        self
    }
}

impl AdminClient {
    /// `GET /v1/organizations/users`: organization members.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/users/list>
    pub fn users(&self) -> ListUsers {
        ListUsers { inner: CursorList::new(self.api.clone(), Ok(ApiPath::new("v1/organizations/users")), 1000) }
    }

    /// `GET /v1/organizations/users/{user_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/users/retrieve>
    pub async fn user(&self, user_id: &str) -> Result<ApiResponse<User>> {
        self.api.get_json(&ApiPath::new("v1/organizations/users").id(user_id)?, &[]).await
    }

    /// `GET /v1/organizations/invites`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/invites/list>
    pub fn invites(&self) -> ListInvites {
        ListInvites { inner: CursorList::new(self.api.clone(), Ok(ApiPath::new("v1/organizations/invites")), 1000) }
    }

    /// `GET /v1/organizations/invites/{invite_id}`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/invites/retrieve>
    pub async fn invite(&self, invite_id: &str) -> Result<ApiResponse<Invite>> {
        self.api.get_json(&ApiPath::new("v1/organizations/invites").id(invite_id)?, &[]).await
    }
}
