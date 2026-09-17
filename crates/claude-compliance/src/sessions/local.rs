//! Local sessions: list, retrieve, messages.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, PageToken, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::content::{SessionContentBlock, split_tag, tagged, untagged};
use super::{LIST_MAX_LIMIT, MessageQuery, SessionMessageOrder, SessionMessageRole, SessionUser, push_limit};

pub(super) const PATH: &str = "v1/compliance/apps/sessions/local";

/// A page of local sessions. The list signals its end only with `next_page: null`.
pub type LocalSessionPage = ApiResponse<TokenPage<LocalSession>>;

/// The stream returned by [`ListLocalSessions::stream`].
pub type LocalSessionStream = Pin<Box<dyn Stream<Item = Result<LocalSession>> + Send + 'static>>;

/// The stream returned by [`ListLocalSessionMessages::stream`].
pub type LocalSessionMessageStream = Pin<Box<dyn Stream<Item = Result<LocalSessionMessage>> + Send + 'static>>;

claude_api_core::string_enum! {
    /// The product a local session ran in. Anthropic adds values as coverage expands.
    pub enum LocalProductSurface {
        /// `cowork`: Cowork in Claude Desktop on the user's machine.
        Cowork = "cowork",
        /// `claude_code`: Claude Code.
        ClaudeCode = "claude_code",
        /// `claude_science`: Claude Science.
        ClaudeScience = "claude_science",
        /// `office_agents`: Claude for Microsoft 365, app not identified.
        OfficeAgents = "office_agents",
        /// `office_agents/excel`
        OfficeAgentsExcel = "office_agents/excel",
        /// `office_agents/powerpoint`
        OfficeAgentsPowerpoint = "office_agents/powerpoint",
        /// `office_agents/word`
        OfficeAgentsWord = "office_agents/word",
        /// `office_agents/outlook`
        OfficeAgentsOutlook = "office_agents/outlook",
    }
}

/// A session a user ran on their own computer in a Claude app while signed in with their
/// organization account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSession {
    /// Always `compliance_local_session`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `clls_…` ID, unique within the parent organization.
    pub id: String,
    /// The session's first retained inference call.
    pub created_at: Timestamp,
    /// UUID of the child organization.
    pub organization_uuid: String,
    /// The product the session ran in; `null` when not recorded.
    #[serde(default)]
    pub product_surface: Option<LocalProductSurface>,
    /// Whether the session exceeds the 100,000 inference calls the messages endpoint can return.
    #[serde(default)]
    pub truncated: bool,
    /// The session's last retained inference call. A lower bound on the list endpoint.
    pub updated_at: Timestamp,
    /// The authenticated user. `email_address` is always `null` inside a messages response.
    pub user: SessionUser,
    /// `wrkspc_…` ID; `null` when not attributed to a workspace.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One local session transcript turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSessionMessage {
    /// Always `compliance_local_session_message`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `clsm_…` ID, stable while the turn is retained.
    pub id: String,
    /// Content blocks; empty when `provenance` is `content_unavailable`. Thinking and the request's
    /// `system` field are never included.
    pub content: Vec<SessionContentBlock>,
    /// When the message was recorded (the call's timestamp).
    pub created_at: Timestamp,
    /// The model that served an assistant turn; `null` on user turns and whenever `provenance` is set.
    #[serde(default)]
    pub model: Option<String>,
    /// Where the content came from; `null` means verified content.
    #[serde(default)]
    pub provenance: Option<Provenance>,
    /// Sender.
    pub role: SessionMessageRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The origin of a local session turn whose content is not verified, discriminated by `type`.
#[derive(Debug, Clone, PartialEq)]
pub enum Provenance {
    /// `content_unavailable`: the content cannot be returned and `content` is empty.
    ContentUnavailable(ContentUnavailable),
    /// `client_asserted`: assistant history supplied by the client; authorship not verified.
    ClientAsserted,
    /// `synthetic_marker`: a marker the endpoint generated, not content either party sent.
    SyntheticMarker,
    /// Any other `type`, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole object.
        raw: Value,
    },
}

impl Provenance {
    /// The wire `type`.
    pub fn kind(&self) -> &str {
        match self {
            Self::ContentUnavailable(_) => "content_unavailable",
            Self::ClientAsserted => "client_asserted",
            Self::SyntheticMarker => "synthetic_marker",
            Self::Other { kind, .. } => kind,
        }
    }
}

/// `content_unavailable` provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentUnavailable {
    /// Why the content cannot be returned.
    pub reason: ContentUnavailableReason,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Why a local session turn's content cannot be returned.
    pub enum ContentUnavailableReason {
        /// `not_captured`: not captured for compliance retrieval, or withheld by storage access
        /// policies (deliberately indistinguishable).
        NotCaptured = "not_captured",
        /// `client_aborted`: the client cancelled before the response completed (assistant turns).
        ClientAborted = "client_aborted",
        /// `cmek_key_revoked`: the customer-managed encryption key is unavailable.
        CmekKeyRevoked = "cmek_key_revoked",
        /// `retention_elapsed`: the placeholder for every turn past the retention boundary.
        RetentionElapsed = "retention_elapsed",
        /// `oversize`: the message exceeds the per-message size bound after truncation.
        Oversize = "oversize",
    }
}

impl<'de> Deserialize<'de> for Provenance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let kind = split_tag::<D::Error>(&raw)?;
        match kind.as_str() {
            "content_unavailable" => untagged(raw).map(Self::ContentUnavailable).map_err(D::Error::custom),
            "client_asserted" => Ok(Self::ClientAsserted),
            "synthetic_marker" => Ok(Self::SyntheticMarker),
            _ => Ok(Self::Other { kind, raw }),
        }
    }
}

impl Serialize for Provenance {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::ContentUnavailable(inner) => tagged(self.kind(), inner).map_err(S::Error::custom)?,
            Self::ClientAsserted | Self::SyntheticMarker => serde_json::json!({ "type": self.kind() }),
            Self::Other { raw, .. } => raw.clone(),
        };
        value.serialize(serializer)
    }
}

/// A page of local session messages with the session they belong to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSessionMessagesPage {
    /// Turns in call order.
    pub data: Vec<LocalSessionMessage>,
    /// Token for the next page, `null` at the end. Valid for 24 hours from the walk's first page.
    #[serde(default)]
    pub next_page: Option<PageToken>,
    /// The session. `user.email_address` is always `null` here.
    pub session: LocalSession,
}

impl LocalSessionMessagesPage {
    /// The token to request next, or `None` when the walk is complete.
    pub fn next(&self) -> Option<&PageToken> {
        self.next_page.as_ref()
    }
}

/// Request builder for `GET /v1/compliance/apps/sessions/local`. Results are newest first by
/// `created_at`; pagination is forward-only.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListLocalSessions {
    api: ApiClient,
    limit: Option<u32>,
    page: Option<PageToken>,
    created_at_gte: Option<Timestamp>,
    created_at_lt: Option<Timestamp>,
    updated_at_gte: Option<Timestamp>,
}

impl ListLocalSessions {
    pub(super) fn new(api: ApiClient) -> Self {
        Self { api, limit: None, page: None, created_at_gte: None, created_at_lt: None, updated_at_gte: None }
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

    /// `created_at.gte`: first inference call at or after this time.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.created_at_gte = Some(at);
        self
    }

    /// `created_at.lt`: first inference call strictly before this time.
    pub fn created_at_lt(mut self, at: Timestamp) -> Self {
        self.created_at_lt = Some(at);
        self
    }

    /// `updated_at.gte`: last inference call at or after this time. Filters without changing order
    /// or pagination; use it to poll for sessions active since a previous pass.
    pub fn updated_at_gte(mut self, at: Timestamp) -> Self {
        self.updated_at_gte = Some(at);
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<LocalSessionPage> {
        self.api.get_json(&ApiPath::new(PATH), &self.query()?).await
    }

    /// Streams every matching session, following `next_page` until it is `null`.
    pub fn stream(self) -> LocalSessionStream {
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
        let mut query = Vec::new();
        push_limit(&mut query, self.limit, LIST_MAX_LIMIT)?;
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        for (name, at) in [
            ("created_at.gte", self.created_at_gte),
            ("created_at.lt", self.created_at_lt),
            ("updated_at.gte", self.updated_at_gte),
        ] {
            if let Some(at) = at {
                query.push((name, at.to_string()));
            }
        }
        Ok(query)
    }
}

/// Request builder for `GET /v1/compliance/apps/sessions/local/{local_session_id}/messages`.
///
/// Turns at or before the organization's retention boundary are replaced by one leading
/// `content_unavailable` placeholder (`retention_elapsed`). The boundary is pinned on the first page
/// for 24 hours; an older `page` token is a 400, so restart the walk.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListLocalSessionMessages {
    api: ApiClient,
    local_session_id: String,
    query: MessageQuery,
}

impl ListLocalSessionMessages {
    pub(super) fn new(api: ApiClient, local_session_id: String) -> Self {
        Self { api, local_session_id, query: MessageQuery::default() }
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

    /// `tool_result_max_bytes`: cap on each tool-result text item. `-1` requests the server maximum
    /// (about 1 MiB); `0` is invalid. Server default 10000.
    pub fn tool_result_max_bytes(mut self, bytes: i64) -> Self {
        self.query.tool_result_max_bytes = Some(bytes);
        self
    }

    /// `tool_use_input_max_bytes`: cap on each tool-use input. `-1` requests the server maximum
    /// (about 1 MiB); `0` is invalid. Server default 10000.
    pub fn tool_use_input_max_bytes(mut self, bytes: i64) -> Self {
        self.query.tool_use_input_max_bytes = Some(bytes);
        self
    }

    /// Fetches one page, with the session envelope.
    pub async fn send(&self) -> Result<ApiResponse<LocalSessionMessagesPage>> {
        let path = ApiPath::new(PATH).id(&self.local_session_id)?.then("messages");
        self.api.get_json(&path, &self.query.build()?).await
    }

    /// Streams every message, following `next_page` until it is `null`. The session envelope is
    /// dropped; use [`Self::send`] to read it.
    pub fn stream(self) -> LocalSessionMessageStream {
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
