//! Remote sessions: list, messages.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Error, PageToken, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::content::SessionContentBlock;
use super::{LIST_MAX_LIMIT, MessageQuery, SessionMessageOrder, SessionMessageRole, SessionUser, push_limit};

const PATH: &str = "v1/compliance/apps/sessions/remote";
const MAX_ORGANIZATION_IDS: usize = 500;
const MAX_USER_IDS: usize = 10;

/// A page of remote sessions. The list signals its end only with `next_page: null`.
pub type RemoteSessionPage = ApiResponse<TokenPage<RemoteSession>>;

/// The stream returned by [`ListRemoteSessions::stream`].
pub type RemoteSessionStream = Pin<Box<dyn Stream<Item = Result<RemoteSession>> + Send + 'static>>;

/// The stream returned by [`ListRemoteSessionMessages::stream`].
pub type RemoteSessionMessageStream = Pin<Box<dyn Stream<Item = Result<RemoteSessionMessage>> + Send + 'static>>;

claude_api_core::string_enum! {
    /// The Claude product a remote session was created from. Anthropic adds values as surfaces launch.
    pub enum RemoteProductSurface {
        /// `cowork_remote`: Cowork started on claude.ai web or mobile.
        CoworkRemote = "cowork_remote",
    }
}

claude_api_core::string_enum! {
    /// A remote session lifecycle state.
    pub enum RemoteSessionStatus {
        /// `pending`: transient, before any transcript exists; the messages endpoint returns 404.
        Pending = "pending",
        /// `active`
        Active = "active",
        /// `paused`
        Paused = "paused",
        /// `archived`
        Archived = "archived",
        /// `failed`
        Failed = "failed",
    }
}

/// A Cowork session that ran in an Anthropic-managed cloud environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSession {
    /// `cse_…` ID.
    pub id: String,
    /// The automated agent that owns the session; `null` for user-owned sessions. At most one of
    /// `user` and `agent_id` is set.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The bound project; `null` without a binding, and always `null` inside a messages response.
    #[serde(default)]
    pub claude_project_id: Option<String>,
    /// When the session was created.
    pub created_at: Timestamp,
    /// UUID of the organization.
    pub organization_uuid: String,
    /// The product the session was created from; `null` when not recorded or not recognised.
    #[serde(default)]
    pub product_surface: Option<RemoteProductSurface>,
    /// The user who started the session. Always `null` inside a messages response.
    #[serde(default)]
    pub started_by_user: Option<SessionUser>,
    /// Lifecycle state.
    pub status: RemoteSessionStatus,
    /// When the session was last modified.
    pub updated_at: Timestamp,
    /// The owning user; `null` for agent-owned sessions. `email_address` is always `null` inside a
    /// messages response.
    #[serde(default)]
    pub user: Option<SessionUser>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One remote session transcript turn. Thinking blocks and images are not included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSessionMessage {
    /// Message ID, for example `csev_…`.
    pub id: String,
    /// Content blocks; empty when [`Self::content_unavailable`] is set.
    pub content: Vec<SessionContentBlock>,
    /// Whether stored content was withheld (undecryptable or over the per-event size bound).
    #[serde(default)]
    pub content_unavailable: bool,
    /// Commit timestamp; may tie or invert under concurrent writes, so keep the returned order.
    pub created_at: Timestamp,
    /// Sender.
    pub role: SessionMessageRole,
    /// The human account that sent this turn on an agent-owned session; `null` on user-owned ones.
    #[serde(default)]
    pub sent_by_user_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A page of remote session messages with the session they belong to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSessionMessagesPage {
    /// Turns in transcript order.
    pub data: Vec<RemoteSessionMessage>,
    /// Token for the next page, `null` at the end.
    #[serde(default)]
    pub next_page: Option<PageToken>,
    /// The session. `started_by_user`, `user.email_address` and `claude_project_id` are always `null`
    /// here.
    pub session: RemoteSession,
}

impl RemoteSessionMessagesPage {
    /// The token to request next, or `None` when the walk is complete.
    pub fn next(&self) -> Option<&PageToken> {
        self.next_page.as_ref()
    }
}

/// Request builder for `GET /v1/compliance/apps/sessions/remote`. Results are newest first by
/// `created_at`; pagination is forward-only.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRemoteSessions {
    api: ApiClient,
    limit: Option<u32>,
    page: Option<PageToken>,
    organization_ids: Vec<String>,
    user_ids: Vec<String>,
    created_at: Vec<(&'static str, Timestamp)>,
}

impl ListRemoteSessions {
    pub(super) fn new(api: ApiClient) -> Self {
        Self {
            api,
            limit: None,
            page: None,
            organization_ids: Vec::new(),
            user_ids: Vec::new(),
            created_at: Vec::new(),
        }
    }

    /// Page size, 1 to 500 (server default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Continue from a previous page's `next_page`.
    pub fn page(mut self, page: PageToken) -> Self {
        self.page = Some(page);
        self
    }

    /// Adds an `organization_ids[]` filter (at most 500).
    pub fn organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_ids.push(organization_id.into());
        self
    }

    /// Adds a `user_ids[]` filter (at most 10). Matches the owning user, so agent-owned sessions are
    /// excluded whenever it is set.
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_ids.push(user_id.into());
        self
    }

    /// `created_at.gt`.
    pub fn created_at_gt(self, at: Timestamp) -> Self {
        self.created_at("created_at.gt", at)
    }

    /// `created_at.gte`.
    pub fn created_at_gte(self, at: Timestamp) -> Self {
        self.created_at("created_at.gte", at)
    }

    /// `created_at.lt`.
    pub fn created_at_lt(self, at: Timestamp) -> Self {
        self.created_at("created_at.lt", at)
    }

    /// `created_at.lte`.
    pub fn created_at_lte(self, at: Timestamp) -> Self {
        self.created_at("created_at.lte", at)
    }

    fn created_at(mut self, key: &'static str, at: Timestamp) -> Self {
        self.created_at.retain(|(existing, _)| *existing != key);
        self.created_at.push((key, at));
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<RemoteSessionPage> {
        self.api.get_json(&ApiPath::new(PATH), &self.query()?).await
    }

    /// Streams every matching session, following `next_page` until it is `null`.
    pub fn stream(self) -> RemoteSessionStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for session in page.data {
                    yield session;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        if self.organization_ids.len() > MAX_ORGANIZATION_IDS {
            return Err(Error::InvalidArgument(format!(
                "at most {MAX_ORGANIZATION_IDS} organization_ids, got {}",
                self.organization_ids.len()
            )));
        }
        if self.user_ids.len() > MAX_USER_IDS {
            return Err(Error::InvalidArgument(format!(
                "at most {MAX_USER_IDS} user_ids, got {}",
                self.user_ids.len()
            )));
        }
        let mut query = Vec::new();
        push_limit(&mut query, self.limit, LIST_MAX_LIMIT)?;
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        query.extend(self.organization_ids.iter().map(|value| ("organization_ids[]", value.clone())));
        query.extend(self.user_ids.iter().map(|value| ("user_ids[]", value.clone())));
        query.extend(self.created_at.iter().map(|(key, at)| (*key, at.to_string())));
        Ok(query)
    }
}

/// Request builder for `GET /v1/compliance/apps/sessions/remote/{claude_remote_session_id}/messages`.
///
/// A `pending`, deleted or out-of-scope session is a 404; a malformed ID is a 400.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListRemoteSessionMessages {
    api: ApiClient,
    claude_remote_session_id: String,
    query: MessageQuery,
}

impl ListRemoteSessionMessages {
    pub(super) fn new(api: ApiClient, claude_remote_session_id: String) -> Self {
        Self { api, claude_remote_session_id, query: MessageQuery::default() }
    }

    /// Page size, 1 to 1000 (server default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Sort direction (server default oldest first).
    pub fn order(mut self, order: SessionMessageOrder) -> Self {
        self.query.order = Some(order);
        self
    }

    /// Continue from a previous page's `next_page`.
    pub fn page(mut self, page: PageToken) -> Self {
        self.query.page = Some(page);
        self
    }

    /// `tool_result_max_bytes`: cap on each tool-result text item. `-1` requests the server maximum;
    /// `0` is invalid. Server default 10000.
    pub fn tool_result_max_bytes(mut self, bytes: i64) -> Self {
        self.query.tool_result_max_bytes = Some(bytes);
        self
    }

    /// `tool_use_input_max_bytes`: cap on each tool-use input. `-1` requests the server maximum; `0`
    /// is invalid. Server default 10000.
    pub fn tool_use_input_max_bytes(mut self, bytes: i64) -> Self {
        self.query.tool_use_input_max_bytes = Some(bytes);
        self
    }

    /// Fetches one page, with the session envelope.
    pub async fn send(&self) -> Result<ApiResponse<RemoteSessionMessagesPage>> {
        let path = ApiPath::new(PATH).id(&self.claude_remote_session_id)?.then("messages");
        self.api.get_json(&path, &self.query.build()?).await
    }

    /// Streams every message, following `next_page` until it is `null`. The session envelope is
    /// dropped; use [`Self::send`] to read it.
    pub fn stream(self) -> RemoteSessionMessageStream {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.next().cloned();
                for message in page.data {
                    yield message;
                }
                match next {
                    Some(token) => request.query.page = Some(token),
                    None => break,
                }
            }
        })
    }
}
