//! Session endpoints:
//!
//! - `GET /v1/compliance/apps/sessions/local`
//! - `GET /v1/compliance/apps/sessions/local/{local_session_id}`
//! - `GET /v1/compliance/apps/sessions/local/{local_session_id}/messages`
//! - `GET /v1/compliance/apps/sessions/remote`
//! - `GET /v1/compliance/apps/sessions/remote/{claude_remote_session_id}/messages`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! Local sessions ran in a Claude app on the user's own computer (Cowork, Claude Code, Claude for
//! Microsoft 365); remote sessions are Cowork sessions in Anthropic-managed cloud environments. A
//! Compliance Access Key with `read:compliance_user_data` reads both.
//!
//! Transcripts are returned as sent: credentials, secrets and personal data inside prompts, tool
//! inputs and tool results are not masked. Handle them as sensitive data.

use claude_api_core::{ApiPath, ApiResponse, Error, PageToken, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

mod content;
mod local;
mod remote;

pub use content::{
    SessionContentBlock, SessionTextBlock, SessionToolResultBlock, SessionToolResultItem, SessionToolUseBlock,
};
pub use local::{
    ContentUnavailable, ContentUnavailableReason, ListLocalSessionMessages, ListLocalSessions, LocalProductSurface,
    LocalSession, LocalSessionMessage, LocalSessionMessageStream, LocalSessionMessagesPage, LocalSessionPage,
    LocalSessionStream, Provenance,
};
pub use remote::{
    ListRemoteSessionMessages, ListRemoteSessions, RemoteProductSurface, RemoteSession, RemoteSessionMessage,
    RemoteSessionMessageStream, RemoteSessionMessagesPage, RemoteSessionPage, RemoteSessionStatus, RemoteSessionStream,
};

const LIST_MAX_LIMIT: u32 = 500;
const MESSAGES_MAX_LIMIT: u32 = 1000;
const MAX_BYTES: i64 = 2_147_483_647;

/// The user a session is attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUser {
    /// `user_…` ID. Set even after the account is deleted or leaves the readable organizations.
    pub id: String,
    /// Email address; `null` when it cannot be resolved, and always `null` on the messages endpoints.
    #[serde(default)]
    pub email_address: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// A transcript message sender.
    pub enum SessionMessageRole {
        /// `assistant`
        Assistant = "assistant",
        /// `user`
        User = "user",
    }
}

/// Sort direction for the session message endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionMessageOrder {
    /// `asc`: oldest first (server default).
    Asc,
    /// `desc`: newest first.
    Desc,
}

impl SessionMessageOrder {
    /// The wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Validates a documented `limit` range and adds it to the query.
fn push_limit(query: &mut Vec<(&'static str, String)>, limit: Option<u32>, max: u32) -> Result<()> {
    if let Some(limit) = limit {
        if !(1..=max).contains(&limit) {
            return Err(Error::InvalidArgument(format!("limit must be between 1 and {max}, got {limit}")));
        }
        query.push(("limit", limit.to_string()));
    }
    Ok(())
}

/// Query parameters shared by both message endpoints.
#[derive(Debug, Clone, Default)]
struct MessageQuery {
    limit: Option<u32>,
    order: Option<SessionMessageOrder>,
    page: Option<PageToken>,
    tool_result_max_bytes: Option<i64>,
    tool_use_input_max_bytes: Option<i64>,
}

impl MessageQuery {
    fn build(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        push_limit(&mut query, self.limit, MESSAGES_MAX_LIMIT)?;
        if let Some(order) = self.order {
            query.push(("order", order.as_str().to_owned()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        for (name, value) in [
            ("tool_result_max_bytes", self.tool_result_max_bytes),
            ("tool_use_input_max_bytes", self.tool_use_input_max_bytes),
        ] {
            if let Some(bytes) = value {
                if bytes == 0 || !(-1..=MAX_BYTES).contains(&bytes) {
                    return Err(Error::InvalidArgument(format!(
                        "{name} must be -1 (server maximum) or between 1 and {MAX_BYTES}, got {bytes}"
                    )));
                }
                query.push((name, bytes.to_string()));
            }
        }
        Ok(query)
    }
}

impl crate::ComplianceClient {
    /// `GET /v1/compliance/apps/sessions/local`. Requires `read:compliance_user_data`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/sessions/local/list)
    pub fn local_sessions(&self) -> ListLocalSessions {
        ListLocalSessions::new(self.api.clone())
    }

    /// `GET /v1/compliance/apps/sessions/local/{local_session_id}`. Requires `read:compliance_user_data`.
    ///
    /// A session whose every inference call has aged out of retention is a 404.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/sessions/local/retrieve)
    pub async fn local_session(&self, local_session_id: &str) -> Result<ApiResponse<LocalSession>> {
        let path = ApiPath::new(local::PATH).id(local_session_id)?;
        self.api.get_json(&path, &[]).await
    }

    /// `GET /v1/compliance/apps/sessions/local/{local_session_id}/messages`. Requires
    /// `read:compliance_user_data`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/sessions/local/messages/list)
    pub fn local_session_messages(&self, local_session_id: impl Into<String>) -> ListLocalSessionMessages {
        ListLocalSessionMessages::new(self.api.clone(), local_session_id.into())
    }

    /// `GET /v1/compliance/apps/sessions/remote`. Requires `read:compliance_user_data`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/sessions/remote/list)
    pub fn remote_sessions(&self) -> ListRemoteSessions {
        ListRemoteSessions::new(self.api.clone())
    }

    /// `GET /v1/compliance/apps/sessions/remote/{claude_remote_session_id}/messages`. Requires
    /// `read:compliance_user_data`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/sessions/remote/messages/list)
    pub fn remote_session_messages(&self, claude_remote_session_id: impl Into<String>) -> ListRemoteSessionMessages {
        ListRemoteSessionMessages::new(self.api.clone(), claude_remote_session_id.into())
    }
}
