//! Chats and their messages.
//!
//! - `GET /v1/compliance/apps/chats`
//! - `GET /v1/compliance/apps/chats/{claude_chat_id}/messages`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! Message content, tool inputs and tool results are verbatim user and assistant transcript data,
//! including whatever the user pasted and whatever connected tools returned. Treat it as sensitive.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Cursor, CursorPage, Error, Result, string_enum};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::content_common::{
    ContentUser, TimeFilters, check_limit, check_max_items, tagged_union, time_filter_setters,
};

const CHATS_PATH: &str = "v1/compliance/apps/chats";
const MAX_CHAT_LIMIT: u32 = 1000;
const MAX_CHAT_USER_IDS: usize = 10;
const MAX_MESSAGE_LIMIT: u32 = 1000;

/// A page of chats.
pub type ChatPage = ApiResponse<CursorPage<Chat>>;

/// The stream returned by [`ListChats::stream`].
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<Chat>> + Send + 'static>>;

/// The stream returned by [`ListChatMessages::stream`].
pub type ChatMessageStream = Pin<Box<dyn Stream<Item = Result<ChatMessage>> + Send + 'static>>;

string_enum! {
    /// Sort key for [`ListChats::order_by`].
    pub enum ChatOrderBy {
        /// Chat creation time (the server default).
        CreatedAt = "created_at",
        /// Last update time. Org-wide queries only.
        UpdatedAt = "updated_at",
    }
}

string_enum! {
    /// Sort direction for [`ListChatMessages::order`].
    pub enum MessageOrder {
        /// Oldest first (the server default).
        Asc = "asc",
        /// Newest first.
        Desc = "desc",
    }
}

string_enum! {
    /// Who sent a chat message.
    pub enum MessageRole {
        /// The assistant.
        Assistant = "assistant",
        /// The user.
        User = "user",
    }
}

/// Chat metadata, as listed by `GET /v1/compliance/apps/chats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chat {
    /// `claude_chat_…` ID.
    pub id: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Deletion time, if deleted.
    #[serde(default)]
    pub deleted_at: Option<Timestamp>,
    /// URL of the chat in claude.ai.
    pub href: String,
    /// Model selected for the chat; `null` for legacy chats that never recorded one.
    #[serde(default)]
    pub model: Option<String>,
    /// Chat title.
    pub name: String,
    /// Organization UUID.
    pub organization_uuid: String,
    /// `claude_proj_…` ID of the containing project.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Last update time.
    pub updated_at: Timestamp,
    /// The creator with their *current* email address; `null` when a single-organization key can no
    /// longer resolve them. The address at creation time is on the `claude_chat_created` activity.
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// `org_…` ID. Deprecated by Anthropic in favour of `organization_uuid`.
    #[serde(default)]
    pub organization_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The body of `GET /v1/compliance/apps/chats/{claude_chat_id}/messages`: chat metadata, one page of
/// messages and the cursors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessages {
    /// `claude_chat_…` ID.
    pub id: String,
    /// The messages, ordered by `created_at` in the requested direction.
    pub chat_messages: Vec<ChatMessage>,
    /// Creation time.
    pub created_at: Timestamp,
    /// Deletion time, if deleted.
    #[serde(default)]
    pub deleted_at: Option<Timestamp>,
    /// Cursor to the first message; pass as `before_id` to go back.
    #[serde(default)]
    pub first_id: Option<Cursor>,
    /// Whether more messages exist; continue with `last_id` as `after_id`.
    #[serde(default)]
    pub has_more: bool,
    /// URL of the chat in claude.ai.
    pub href: String,
    /// Cursor to the last message; pass as `after_id` to go forward.
    #[serde(default)]
    pub last_id: Option<Cursor>,
    /// Model selected for the chat.
    #[serde(default)]
    pub model: Option<String>,
    /// Chat title.
    pub name: String,
    /// Organization UUID.
    pub organization_uuid: String,
    /// `claude_proj_…` ID of the containing project.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Last update time.
    pub updated_at: Timestamp,
    /// The creator, as on [`Chat::user`].
    #[serde(default)]
    pub user: Option<ContentUser>,
    /// `org_…` ID. Deprecated by Anthropic.
    #[serde(default)]
    pub organization_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// `claude_chat_msg_…` ID.
    pub id: String,
    /// Artifact versions the assistant created or updated in this message.
    #[serde(default)]
    pub artifacts: Option<Vec<MessageArtifact>>,
    /// Content blocks.
    pub content: Vec<ContentBlock>,
    /// For the user: when sent. For the assistant: when the last content block completed.
    pub created_at: Timestamp,
    /// Files the user uploaded with this message.
    #[serde(default)]
    pub files: Option<Vec<MessageFile>>,
    /// Files the assistant created through tool use.
    #[serde(default)]
    pub generated_files: Option<Vec<MessageGeneratedFile>>,
    /// Sender.
    pub role: MessageRole,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A message content block, discriminated by `type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentBlock {
    /// `text`.
    Text(TextBlock),
    /// `tool_use`: a tool invocation requested by the assistant.
    ToolUse(ToolUseBlock),
    /// `tool_result`: what a tool returned.
    ToolResult(ToolResultBlock),
    /// Any other block type, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole block.
        raw: Value,
    },
}

tagged_union!(ContentBlock { Text = "text", ToolUse = "tool_use", ToolResult = "tool_result" });

/// A `text` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBlock {
    /// The text.
    pub text: String,
    /// Whether internal-reasoning content was removed from `text` during export. Always false on
    /// user messages.
    #[serde(default)]
    pub thinking_redacted: bool,
    /// Whether `text` hit the server's fixed 1 MiB bound. Documented as always false on chat text.
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A `tool_use` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// `toolu_…` ID.
    #[serde(default)]
    pub id: Option<String>,
    /// The tool arguments as a JSON-encoded string, possibly shortened (see `truncated`).
    pub input: String,
    /// The integration providing the tool, when applicable.
    #[serde(default)]
    pub integration_name: Option<String>,
    /// Base URL (scheme, host, path) of the MCP server providing the tool, when applicable.
    #[serde(default)]
    pub mcp_server_url: Option<String>,
    /// Tool name.
    pub name: String,
    /// Whether `input` was shortened; see [`ListChatMessages::tool_use_input_max_chars`].
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A `tool_result` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// Text the tool returned. Non-text items are omitted by the server; generated files are listed
    /// on [`ChatMessage::generated_files`].
    pub content: Vec<ToolResultItem>,
    /// The integration providing the tool, when applicable.
    #[serde(default)]
    pub integration_name: Option<String>,
    /// Whether the tool reported an error.
    pub is_error: bool,
    /// Base URL (scheme, host, path) of the MCP server providing the tool, when applicable.
    #[serde(default)]
    pub mcp_server_url: Option<String>,
    /// Tool name.
    pub name: String,
    /// ID of the `tool_use` block this answers.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// Whether any item in `content` was shortened; see [`ListChatMessages::tool_result_max_chars`].
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An item inside [`ToolResultBlock::content`], discriminated by `type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResultItem {
    /// `text`.
    Text(ToolResultText),
    /// Any other item type, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole item.
        raw: Value,
    },
}

tagged_union!(ToolResultItem { Text = "text" });

/// A `text` item of a tool result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultText {
    /// The text.
    pub text: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An artifact version referenced by a message. Fetch it with
/// [`ComplianceClient::artifact`](crate::ComplianceClient::artifact).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageArtifact {
    /// `claude_artifact_…` ID.
    pub id: String,
    /// MIME-like type, for example `application/vnd.ant.code`.
    #[serde(default)]
    pub artifact_type: Option<String>,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
    /// `claude_artifact_version_…` ID.
    pub version_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A file the user uploaded with a message. Download it with
/// [`ComplianceClient::chat_file_content`](crate::ComplianceClient::chat_file_content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageFile {
    /// `claude_file_…` ID.
    pub id: String,
    /// Upload time.
    pub created_at: Timestamp,
    /// Display name. Untrusted user input.
    pub filename: String,
    /// Lowercase hex MD5 of the preferred downloadable variant, as recorded at upload.
    #[serde(default)]
    pub md5: Option<String>,
    /// MIME type of the preferred downloadable variant.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Size in bytes; `null` for older files.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A file the assistant created through tool use. Download it with
/// [`ComplianceClient::generated_file_content`](crate::ComplianceClient::generated_file_content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageGeneratedFile {
    /// Opaque `claude_gen_file_…` ID.
    pub id: String,
    /// Display name.
    pub filename: String,
    /// Lowercase hex MD5, when stored.
    #[serde(default)]
    pub md5: Option<String>,
    /// MIME type reported by the producing tool.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Size in bytes; `null` when expired or unrecorded.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone)]
enum Position {
    After(Cursor),
    Before(Cursor),
}

impl Position {
    fn push_to(&self, query: &mut Vec<(&'static str, String)>) {
        match self {
            Position::After(cursor) => query.push(("after_id", cursor.as_str().to_owned())),
            Position::Before(cursor) => query.push(("before_id", cursor.as_str().to_owned())),
        }
    }
}

/// Request builder for `GET /v1/compliance/apps/chats`. Results are oldest first by the `order_by`
/// key.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/list)
///
/// Checked before sending: `limit` range, at most 10 `user_ids[]`, `order_by=updated_at` and
/// `project_ids[]` against the presence of `user_ids[]`, `before_id` on org-wide queries, and (on
/// org-wide queries) time filters matching the sort key.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListChats {
    api: ApiClient,
    limit: Option<u32>,
    position: Option<Position>,
    order_by: Option<ChatOrderBy>,
    organization_ids: Vec<String>,
    project_ids: Vec<String>,
    user_ids: Vec<String>,
    created_at: TimeFilters,
    updated_at: TimeFilters,
}

impl ListChats {
    pub(crate) fn new(api: ApiClient) -> Self {
        Self {
            api,
            limit: None,
            position: None,
            order_by: None,
            organization_ids: Vec::new(),
            project_ids: Vec::new(),
            user_ids: Vec::new(),
            created_at: TimeFilters::default(),
            updated_at: TimeFilters::default(),
        }
    }

    /// Page size, 1 to 1000 (server default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Continue after a page's `last_id`. Replaces [`Self::before`].
    pub fn after(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::After(cursor));
        self
    }

    /// Go back before a page's `first_id`. Per-user queries only. Replaces [`Self::after`].
    pub fn before(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::Before(cursor));
        self
    }

    /// `order_by`. [`ChatOrderBy::UpdatedAt`] is for org-wide queries (no `user_ids[]`) and is the
    /// documented way to poll incrementally by update time.
    pub fn order_by(mut self, order_by: ChatOrderBy) -> Self {
        self.order_by = Some(order_by);
        self
    }

    /// Adds an `organization_ids[]` filter (UUID or `org_…` form).
    pub fn organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_ids.push(organization_id.into());
        self
    }

    /// Adds a `project_ids[]` filter (`claude_proj_…`). Requires at least one `user_ids[]`.
    pub fn project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_ids.push(project_id.into());
        self
    }

    /// Adds a `user_ids[]` filter (at most 10), making this a per-user query. Anthropic deprecates
    /// combining it with `updated_at.*` filters and rejects that from 2026-09-22.
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
    pub async fn send(&self) -> Result<ChatPage> {
        self.api.get_json(&ApiPath::new(CHATS_PATH), &self.query()?).await
    }

    /// Streams every matching chat, following `last_id` until `has_more` is false. Starts from
    /// [`Self::after`] when set; [`Self::before`] is rejected.
    pub fn stream(self) -> ChatStream {
        Box::pin(try_stream! {
            if matches!(self.position, Some(Position::Before(_))) {
                Err(Error::InvalidArgument("stream() walks forward; use after(), not before()".into()))?;
            }
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.last_id;
                for chat in page.data {
                    yield chat;
                }
                match next {
                    Some(cursor) if page.has_more => request.position = Some(Position::After(cursor)),
                    _ => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        self.validate()?;
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(position) = &self.position {
            position.push_to(&mut query);
        }
        if let Some(order_by) = &self.order_by {
            query.push(("order_by", order_by.as_str().to_owned()));
        }
        query.extend(self.organization_ids.iter().map(|value| ("organization_ids[]", value.clone())));
        query.extend(self.project_ids.iter().map(|value| ("project_ids[]", value.clone())));
        query.extend(self.user_ids.iter().map(|value| ("user_ids[]", value.clone())));
        self.created_at.push_to(&mut query);
        self.updated_at.push_to(&mut query);
        Ok(query)
    }

    fn validate(&self) -> Result<()> {
        if let Some(limit) = self.limit {
            check_limit(limit, MAX_CHAT_LIMIT)?;
        }
        check_max_items("user_ids[]", &self.user_ids, MAX_CHAT_USER_IDS)?;
        let invalid = |message: &str| Err(Error::InvalidArgument(message.to_owned()));
        if !self.user_ids.is_empty() {
            if self.order_by == Some(ChatOrderBy::UpdatedAt) {
                return invalid("order_by=updated_at is only supported for org-wide queries (no user_ids[])");
            }
            return Ok(());
        }
        if !self.project_ids.is_empty() {
            return invalid("project_ids[] requires user_ids[]");
        }
        if matches!(self.position, Some(Position::Before(_))) {
            return invalid("before_id is only supported for per-user queries (user_ids[] set)");
        }
        match self.order_by.as_ref().unwrap_or(&ChatOrderBy::CreatedAt) {
            ChatOrderBy::CreatedAt if !self.updated_at.is_empty() => {
                invalid("org-wide updated_at.* filters require order_by=updated_at")
            }
            ChatOrderBy::UpdatedAt if !self.created_at.is_empty() => {
                invalid("org-wide created_at.* filters require order_by=created_at")
            }
            _ => Ok(()),
        }
    }
}

/// Request builder for `GET /v1/compliance/apps/chats/{claude_chat_id}/messages`.
///
/// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/messages/list)
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListChatMessages {
    api: ApiClient,
    claude_chat_id: String,
    limit: Option<u32>,
    position: Option<Position>,
    order: Option<MessageOrder>,
    tool_result_max_chars: Option<i64>,
    tool_use_input_max_chars: Option<i64>,
    created_at: TimeFilters,
    updated_at: TimeFilters,
}

impl ListChatMessages {
    pub(crate) fn new(api: ApiClient, claude_chat_id: String) -> Self {
        Self {
            api,
            claude_chat_id,
            limit: None,
            position: None,
            order: None,
            tool_result_max_chars: None,
            tool_use_input_max_chars: None,
            created_at: TimeFilters::default(),
            updated_at: TimeFilters::default(),
        }
    }

    /// Page size, 1 to 1000. When unset the server returns every message in one response.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Continue after a response's `last_id`. Replaces [`Self::before`].
    pub fn after(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::After(cursor));
        self
    }

    /// Go back before a response's `first_id`. Replaces [`Self::after`].
    pub fn before(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::Before(cursor));
        self
    }

    /// `order`: [`MessageOrder::Asc`] (server default) or [`MessageOrder::Desc`].
    pub fn order(mut self, order: MessageOrder) -> Self {
        self.order = Some(order);
        self
    }

    /// Characters kept per tool-result text item (server default 10000); `-1` disables the limit.
    pub fn tool_result_max_chars(mut self, max_chars: i64) -> Self {
        self.tool_result_max_chars = Some(max_chars);
        self
    }

    /// Characters of JSON-encoded input kept per `tool_use` block (server default 10000); `-1`
    /// disables the limit.
    pub fn tool_use_input_max_chars(mut self, max_chars: i64) -> Self {
        self.tool_use_input_max_chars = Some(max_chars);
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

    /// Fetches the chat metadata and one page of messages.
    pub async fn send(&self) -> Result<ApiResponse<ChatMessages>> {
        let path = ApiPath::new(CHATS_PATH).id(&self.claude_chat_id)?.then("messages");
        self.api.get_json(&path, &self.query()?).await
    }

    /// Streams every matching message, following `last_id` until `has_more` is false. Starts from
    /// [`Self::after`] when set; [`Self::before`] is rejected.
    pub fn stream(self) -> ChatMessageStream {
        Box::pin(try_stream! {
            if matches!(self.position, Some(Position::Before(_))) {
                Err(Error::InvalidArgument("stream() walks forward; use after(), not before()".into()))?;
            }
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.last_id;
                for message in page.chat_messages {
                    yield message;
                }
                match next {
                    Some(cursor) if page.has_more => request.position = Some(Position::After(cursor)),
                    _ => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            check_limit(limit, MAX_MESSAGE_LIMIT)?;
            query.push(("limit", limit.to_string()));
        }
        if let Some(position) = &self.position {
            position.push_to(&mut query);
        }
        if let Some(order) = &self.order {
            query.push(("order", order.as_str().to_owned()));
        }
        for (key, value) in [
            ("tool_result_max_chars", self.tool_result_max_chars),
            ("tool_use_input_max_chars", self.tool_use_input_max_chars),
        ] {
            if let Some(value) = value {
                if value < -1 {
                    return Err(Error::InvalidArgument(format!("{key} must be -1 or greater, got {value}")));
                }
                query.push((key, value.to_string()));
            }
        }
        self.created_at.push_to(&mut query);
        self.updated_at.push_to(&mut query);
        Ok(query)
    }
}

impl crate::ComplianceClient {
    /// `GET /v1/compliance/apps/chats`: chat metadata across the organization or per user.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/list)
    pub fn chats(&self) -> ListChats {
        ListChats::new(self.api.clone())
    }

    /// `GET /v1/compliance/apps/chats/{claude_chat_id}/messages`: a chat's transcript.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/messages/list)
    pub fn chat_messages(&self, claude_chat_id: impl Into<String>) -> ListChatMessages {
        ListChatMessages::new(self.api.clone(), claude_chat_id.into())
    }
}
