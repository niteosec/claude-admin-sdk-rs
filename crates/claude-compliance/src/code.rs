//! Claude Code Artifacts.
//!
//! - `GET /v1/compliance/apps/code/artifacts`
//! - `GET /v1/compliance/apps/code/artifacts/{artifact_id}/versions/{version_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Download, PageToken, Result, TokenPage, string_enum};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::content_common::{ContentUser, TimeFilters, check_limit, check_max_items, time_filter_setters};

const PATH: &str = "v1/compliance/apps/code/artifacts";
const MAX_LIMIT: u32 = 100;
const MAX_ORGANIZATION_IDS: usize = 500;
const MAX_USER_IDS: usize = 200;

/// A page of Code Artifacts.
pub type CodeArtifactPage = ApiResponse<TokenPage<CodeArtifact>>;

/// The stream returned by [`ListCodeArtifacts::stream`].
pub type CodeArtifactStream = Pin<Box<dyn Stream<Item = Result<CodeArtifact>> + Send + 'static>>;

string_enum! {
    /// Who can view a Code Artifact.
    pub enum CodeArtifactReadMode {
        /// Every member of its organization.
        Org = "org",
        /// Only its owner.
        Owner = "owner",
        /// Anyone on the internet.
        Public = "public",
        /// A named set of users.
        Users = "users",
    }
}

/// A Claude Code Artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeArtifact {
    /// `cart_…` ID.
    pub id: String,
    /// Organization UUID.
    pub organization_uuid: String,
    /// Owner's `user_…` ID; `null` when an agent session published it. Survives the owner's
    /// account deletion.
    #[serde(default)]
    pub owner_user_id: Option<String>,
    /// The version a non-owner viewer would be served, even when `read_mode` is `owner`.
    #[serde(default)]
    pub published_version_id: Option<String>,
    /// Who can view it. `public` means anyone on the internet.
    pub read_mode: CodeArtifactReadMode,
    /// Last update time; `null` for Artifacts published before this was recorded.
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
    /// The owner; `null` for agent-published Artifacts or owners the key can no longer resolve.
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// Roughly the 20 most recently published versions; older ones are not retained.
    pub versions: Vec<CodeArtifactVersion>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A version entry on a [`CodeArtifact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeArtifactVersion {
    /// Opaque version ID.
    pub id: String,
    /// When the version was published.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Title at this version, or the version ID when the title is no longer retained.
    pub name: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/compliance/apps/code/artifacts`. Results are sorted by Artifact ID,
/// not time.
///
/// Pages can be short or empty while `next_page` is still set; [`Self::stream`] continues until it
/// is absent. An Artifact published during a walk can sort before the cursor and be missed, and
/// `updated_at` filters use an eventually consistent index that never matches Artifacts published
/// before the field existed: omit time filters for a complete enumeration.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/code/artifacts/list)
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListCodeArtifacts {
    api: ApiClient,
    limit: Option<u32>,
    page: Option<PageToken>,
    organization_ids: Vec<String>,
    user_ids: Vec<String>,
    updated_at: TimeFilters,
}

impl ListCodeArtifacts {
    pub(crate) fn new(api: ApiClient) -> Self {
        Self {
            api,
            limit: None,
            page: None,
            organization_ids: Vec::new(),
            user_ids: Vec::new(),
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

    /// Adds an `organization_ids[]` filter (UUID or `org_…` form; at most 500).
    pub fn organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_ids.push(organization_id.into());
        self
    }

    /// Adds a `user_ids[]` owner filter (at most 200).
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_ids.push(user_id.into());
        self
    }

    time_filter_setters!(updated_at:
        updated_at_gt => "updated_at.gt",
        updated_at_gte => "updated_at.gte",
        updated_at_lt => "updated_at.lt",
        updated_at_lte => "updated_at.lte",
    );

    /// Fetches one page.
    pub async fn send(&self) -> Result<CodeArtifactPage> {
        self.api.get_json(&ApiPath::new(PATH), &self.query()?).await
    }

    /// Streams every matching Artifact, following `next_page` until it is absent. Starts from
    /// [`Self::page`] when set.
    pub fn stream(self) -> CodeArtifactStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for artifact in page.data {
                    yield artifact;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        check_max_items("organization_ids[]", &self.organization_ids, MAX_ORGANIZATION_IDS)?;
        check_max_items("user_ids[]", &self.user_ids, MAX_USER_IDS)?;
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            check_limit(limit, MAX_LIMIT)?;
            query.push(("limit", limit.to_string()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        query.extend(self.organization_ids.iter().map(|value| ("organization_ids[]", value.clone())));
        query.extend(self.user_ids.iter().map(|value| ("user_ids[]", value.clone())));
        self.updated_at.push_to(&mut query);
        Ok(query)
    }
}

impl crate::ComplianceClient {
    /// `GET /v1/compliance/apps/code/artifacts`: Code Artifacts across the parent organization.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/code/artifacts/list)
    pub fn code_artifacts(&self) -> ListCodeArtifacts {
        ListCodeArtifacts::new(self.api.clone())
    }

    /// `GET /v1/compliance/apps/code/artifacts/{artifact_id}/versions/{version_id}`: one version's
    /// content.
    ///
    /// A 404 can mean the version rotated out of retained history (re-list); a 503 means its upload
    /// is in flight or abandoned (retried per the client's policy). Oversized encoded content ends
    /// the body early with an aborted chunked transfer, surfacing as a body-read error, and
    /// `Content-MD5` is sent only for identity-stored content.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/code/artifacts/retrieve_version)
    pub async fn code_artifact_version_content(&self, artifact_id: &str, version_id: &str) -> Result<Download> {
        let path = ApiPath::new(PATH).id(artifact_id)?.then("versions").id(version_id)?;
        self.api.get_download(&path, &[]).await
    }
}
