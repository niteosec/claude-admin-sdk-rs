//! Projects, their attachments, collaborators and documents.
//!
//! - `GET /v1/compliance/apps/projects`
//! - `GET /v1/compliance/apps/projects/{project_id}`
//! - `GET /v1/compliance/apps/projects/{project_id}/attachments`
//! - `GET /v1/compliance/apps/projects/{project_id}/collaborators`
//! - `GET /v1/compliance/apps/projects/documents/{document_id}/metadata`
//! - `GET /v1/compliance/apps/projects/documents/{document_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! Project instructions and document text are user-authored content. Treat them as sensitive.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, PageToken, Result, TokenPage, string_enum};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::content_common::{ContentUser, TimeFilters, check_limit, tagged_union, time_filter_setters};

const PROJECTS_PATH: &str = "v1/compliance/apps/projects";
const DOCUMENTS_PATH: &str = "v1/compliance/apps/projects/documents";
const MAX_LIMIT: u32 = 100;

/// A page of projects.
pub type ProjectPage = ApiResponse<TokenPage<Project>>;

/// A page of project attachments.
pub type ProjectAttachmentPage = ApiResponse<TokenPage<ProjectAttachment>>;

/// A page of project collaborators.
pub type ProjectCollaboratorPage = ApiResponse<TokenPage<ProjectCollaborator>>;

/// The stream returned by [`ListProjects::stream`].
pub type ProjectStream = Pin<Box<dyn Stream<Item = Result<Project>> + Send + 'static>>;

/// The stream returned by [`ListProjectAttachments::stream`].
pub type ProjectAttachmentStream = Pin<Box<dyn Stream<Item = Result<ProjectAttachment>> + Send + 'static>>;

/// The stream returned by [`ListProjectCollaborators::stream`].
pub type ProjectCollaboratorStream = Pin<Box<dyn Stream<Item = Result<ProjectCollaborator>> + Send + 'static>>;

string_enum! {
    /// A role granted on a project.
    pub enum ProjectRole {
        /// `admin`.
        Admin = "admin",
        /// `editor`.
        Editor = "editor",
        /// `owner`.
        Owner = "owner",
        /// `viewer`.
        Viewer = "viewer",
    }
}

/// Project metadata, as listed by `GET /v1/compliance/apps/projects`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// `claude_proj_…` ID.
    pub id: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// When an end user deleted the project.
    #[serde(default)]
    pub deleted_at: Option<Timestamp>,
    /// `true`: only the creator and named collaborators can access it; `false`: every organization
    /// member can see it.
    pub is_private: bool,
    /// Project name.
    pub name: String,
    /// Organization UUID.
    pub organization_uuid: String,
    /// Last update time.
    pub updated_at: Timestamp,
    /// The creator; `null` when deleted or no longer in an organization the key may read.
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// `org_…` ID. Deprecated by Anthropic in favour of `organization_uuid`.
    #[serde(default)]
    pub organization_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Project details from `GET /v1/compliance/apps/projects/{project_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDetails {
    /// `claude_proj_…` ID.
    pub id: String,
    /// Number of attachments.
    pub attachments_count: u64,
    /// Number of chats.
    pub chats_count: u64,
    /// Creation time.
    pub created_at: Timestamp,
    /// When an end user deleted the project.
    #[serde(default)]
    pub deleted_at: Option<Timestamp>,
    /// Description.
    pub description: String,
    /// Custom instructions (the project prompt).
    pub instructions: String,
    /// Whether access is limited to the creator and named collaborators.
    pub is_private: bool,
    /// Project name.
    pub name: String,
    /// Organization UUID.
    pub organization_uuid: String,
    /// Last update time.
    pub updated_at: Timestamp,
    /// The creator, as on [`Project::user`].
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// `org_…` ID. Deprecated by Anthropic.
    #[serde(default)]
    pub organization_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A project attachment, discriminated by `type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectAttachment {
    /// `project_file`: a binary file. Download with
    /// [`ComplianceClient::chat_file_content`](crate::ComplianceClient::chat_file_content).
    File(ProjectFileAttachment),
    /// `project_doc`: a plain-text document. Fetch with
    /// [`ComplianceClient::project_document`](crate::ComplianceClient::project_document).
    Doc(ProjectDocAttachment),
    /// Any other attachment type, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole attachment.
        raw: Value,
    },
}

tagged_union!(ProjectAttachment { File = "project_file", Doc = "project_doc" });

/// A `project_file` attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFileAttachment {
    /// `claude_file_…` ID.
    pub id: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Display name. Untrusted user input.
    pub filename: String,
    /// Lowercase hex MD5, when recorded. [`ComplianceClient::chat_file`](crate::ComplianceClient::chat_file)
    /// has the authoritative value.
    #[serde(default)]
    pub md5: Option<String>,
    /// MIME type, `application/octet-stream` when none is recorded.
    pub mime_type: String,
    /// Size in bytes, when recorded.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A `project_doc` attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDocAttachment {
    /// `claude_proj_doc_…` ID.
    pub id: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Display name. Untrusted user input.
    pub filename: String,
    /// Always `text/plain`.
    pub mime_type: String,
    /// Reserved; documented as always `null` for now.
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One active role assignment on a project, discriminated by `type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCollaborator {
    /// `user`: an individual user.
    User(ProjectUserCollaborator),
    /// `group`: an RBAC group.
    Group(ProjectGroupCollaborator),
    /// `organization`: every member of an organization.
    Organization(ProjectOrganizationCollaborator),
    /// `organization_role`: every holder of an organization-level role.
    OrganizationRole(ProjectOrganizationRoleCollaborator),
    /// Any other collaborator type, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole entry.
        raw: Value,
    },
}

tagged_union!(ProjectCollaborator {
    User = "user",
    Group = "group",
    Organization = "organization",
    OrganizationRole = "organization_role",
});

/// A `user` collaborator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUserCollaborator {
    /// When access was granted.
    pub granted_at: Timestamp,
    /// Role granted.
    pub role: ProjectRole,
    /// `user_…` ID; `null` when the account has been deleted.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A `group` collaborator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectGroupCollaborator {
    /// When access was granted.
    pub granted_at: Timestamp,
    /// Group ID.
    pub group_id: String,
    /// Role granted.
    pub role: ProjectRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An `organization` collaborator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectOrganizationCollaborator {
    /// When access was granted.
    pub granted_at: Timestamp,
    /// UUID of the organization granted access.
    pub organization_uuid: String,
    /// Role granted.
    pub role: ProjectRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An `organization_role` collaborator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectOrganizationRoleCollaborator {
    /// When access was granted.
    pub granted_at: Timestamp,
    /// The organization-level role whose holders are granted access.
    pub organization_role: String,
    /// Role granted on the project.
    pub role: ProjectRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Project document metadata, without the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDocumentMetadata {
    /// `claude_proj_doc_…` ID.
    pub id: String,
    /// The project the document belongs to.
    pub claude_project_id: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// File name. Untrusted user input.
    pub filename: String,
    /// Lowercase hex MD5 of the UTF-8 text.
    pub md5: String,
    /// Always `text/plain`.
    pub mime_type: String,
    /// Size in bytes of the UTF-8 text.
    pub size_bytes: u64,
    /// The creator, as on [`Project::user`].
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A project document with its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDocument {
    /// `claude_proj_doc_…` ID.
    pub id: String,
    /// The document text.
    pub content: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// File name. Untrusted user input.
    pub filename: String,
    /// The creator, as on [`Project::user`].
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/compliance/apps/projects`. Results are oldest first by
/// `created_at`.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/list)
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListProjects {
    api: ApiClient,
    limit: Option<u32>,
    page: Option<PageToken>,
    organization_ids: Vec<String>,
    user_ids: Vec<String>,
    created_at: TimeFilters,
    updated_at: TimeFilters,
}

impl ListProjects {
    pub(crate) fn new(api: ApiClient) -> Self {
        Self {
            api,
            limit: None,
            page: None,
            organization_ids: Vec::new(),
            user_ids: Vec::new(),
            created_at: TimeFilters::default(),
            updated_at: TimeFilters::default(),
        }
    }

    /// Page size, 1 to 100 (server default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `page`: a previous response's `next_page`.
    pub fn page(mut self, token: PageToken) -> Self {
        self.page = Some(token);
        self
    }

    /// Adds an `organization_ids[]` filter (UUID or `org_…` form).
    pub fn organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_ids.push(organization_id.into());
        self
    }

    /// Adds a `user_ids[]` filter.
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_ids.push(user_id.into());
        self
    }

    time_filter_setters!(created_at:
        created_at_gt => "created_at.gt",
        created_at_gte => "created_at.gte",
        created_at_lt => "created_at.lt",
        created_at_lte => "created_at.lte",
    );

    time_filter_setters!(updated_at:
        updated_at_gt => "updated_at.gt",
        updated_at_gte => "updated_at.gte",
        updated_at_lt => "updated_at.lt",
        updated_at_lte => "updated_at.lte",
    );

    /// Fetches one page.
    pub async fn send(&self) -> Result<ProjectPage> {
        self.api.get_json(&ApiPath::new(PROJECTS_PATH), &self.query()?).await
    }

    /// Streams every matching project, following `next_page`. Starts from [`Self::page`] when set.
    pub fn stream(self) -> ProjectStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for project in page.data {
                    yield project;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        push_page_params(&mut query, self.limit, self.page.as_ref())?;
        query.extend(self.organization_ids.iter().map(|value| ("organization_ids[]", value.clone())));
        query.extend(self.user_ids.iter().map(|value| ("user_ids[]", value.clone())));
        self.created_at.push_to(&mut query);
        self.updated_at.push_to(&mut query);
        Ok(query)
    }
}

/// Request builder for `GET /v1/compliance/apps/projects/{project_id}/attachments`. Results are
/// oldest first by `created_at`.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/attachments/list)
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListProjectAttachments {
    api: ApiClient,
    project_id: String,
    limit: Option<u32>,
    page: Option<PageToken>,
}

impl ListProjectAttachments {
    pub(crate) fn new(api: ApiClient, project_id: String) -> Self {
        Self { api, project_id, limit: None, page: None }
    }

    /// Page size, 1 to 100 (server default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `page`: a previous response's `next_page`.
    pub fn page(mut self, token: PageToken) -> Self {
        self.page = Some(token);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ProjectAttachmentPage> {
        let mut query = Vec::new();
        push_page_params(&mut query, self.limit, self.page.as_ref())?;
        let path = ApiPath::new(PROJECTS_PATH).id(&self.project_id)?.then("attachments");
        self.api.get_json(&path, &query).await
    }

    /// Streams every attachment, following `next_page`. Starts from [`Self::page`] when set.
    pub fn stream(self) -> ProjectAttachmentStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for attachment in page.data {
                    yield attachment;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }
}

/// Request builder for `GET /v1/compliance/apps/projects/{project_id}/collaborators`. Results are
/// oldest first by `granted_at`.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/collaborators/list)
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListProjectCollaborators {
    api: ApiClient,
    project_id: String,
    limit: Option<u32>,
    page: Option<PageToken>,
}

impl ListProjectCollaborators {
    pub(crate) fn new(api: ApiClient, project_id: String) -> Self {
        Self { api, project_id, limit: None, page: None }
    }

    /// Page size, 1 to 100 (server default 20).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// `page`: a previous response's `next_page`.
    pub fn page(mut self, token: PageToken) -> Self {
        self.page = Some(token);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ProjectCollaboratorPage> {
        let mut query = Vec::new();
        push_page_params(&mut query, self.limit, self.page.as_ref())?;
        let path = ApiPath::new(PROJECTS_PATH).id(&self.project_id)?.then("collaborators");
        self.api.get_json(&path, &query).await
    }

    /// Streams every collaborator, following `next_page`. Starts from [`Self::page`] when set.
    pub fn stream(self) -> ProjectCollaboratorStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for collaborator in page.data {
                    yield collaborator;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }
}

fn push_page_params(
    query: &mut Vec<(&'static str, String)>,
    limit: Option<u32>,
    page: Option<&PageToken>,
) -> Result<()> {
    if let Some(limit) = limit {
        check_limit(limit, MAX_LIMIT)?;
        query.push(("limit", limit.to_string()));
    }
    if let Some(page) = page {
        query.push(("page", page.as_str().to_owned()));
    }
    Ok(())
}

impl crate::ComplianceClient {
    /// `GET /v1/compliance/apps/projects`: project metadata.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/list)
    pub fn projects(&self) -> ListProjects {
        ListProjects::new(self.api.clone())
    }

    /// `GET /v1/compliance/apps/projects/{project_id}`: project details, instructions included.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/retrieve)
    pub async fn project(&self, project_id: &str) -> Result<ApiResponse<ProjectDetails>> {
        self.api.get_json(&ApiPath::new(PROJECTS_PATH).id(project_id)?, &[]).await
    }

    /// `GET /v1/compliance/apps/projects/{project_id}/attachments`: files and documents attached to a
    /// project.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/attachments/list)
    pub fn project_attachments(&self, project_id: impl Into<String>) -> ListProjectAttachments {
        ListProjectAttachments::new(self.api.clone(), project_id.into())
    }

    /// `GET /v1/compliance/apps/projects/{project_id}/collaborators`: active role assignments.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/collaborators/list)
    pub fn project_collaborators(&self, project_id: impl Into<String>) -> ListProjectCollaborators {
        ListProjectCollaborators::new(self.api.clone(), project_id.into())
    }

    /// `GET /v1/compliance/apps/projects/documents/{document_id}/metadata`: document metadata without
    /// the text.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/documents/metadata)
    pub async fn project_document_metadata(&self, document_id: &str) -> Result<ApiResponse<ProjectDocumentMetadata>> {
        self.api.get_json(&ApiPath::new(DOCUMENTS_PATH).id(document_id)?.then("metadata"), &[]).await
    }

    /// `GET /v1/compliance/apps/projects/documents/{document_id}`: the document with its text.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/projects/documents/retrieve)
    pub async fn project_document(&self, document_id: &str) -> Result<ApiResponse<ProjectDocument>> {
        self.api.get_json(&ApiPath::new(DOCUMENTS_PATH).id(document_id)?, &[]).await
    }
}
