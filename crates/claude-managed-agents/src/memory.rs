//! Memory stores, the memories in them, and the version history of every memory.
//!
//! | Endpoint | Method |
//! |---|---|
//! | `GET /v1/memory_stores` | [`ManagedAgentsClient::memory_stores`](crate::ManagedAgentsClient::memory_stores) |
//! | `GET /v1/memory_stores/{memory_store_id}` | [`ManagedAgentsClient::memory_store`](crate::ManagedAgentsClient::memory_store) |
//! | `GET /v1/memory_stores/{memory_store_id}/memories` | [`ManagedAgentsClient::memories`](crate::ManagedAgentsClient::memories) |
//! | `GET /v1/memory_stores/{memory_store_id}/memories/{memory_id}` | [`ManagedAgentsClient::memory`](crate::ManagedAgentsClient::memory) |
//! | `GET /v1/memory_stores/{memory_store_id}/memory_versions` | [`ManagedAgentsClient::memory_versions`](crate::ManagedAgentsClient::memory_versions) |
//! | `GET /v1/memory_stores/{memory_store_id}/memory_versions/{memory_version_id}` | [`ManagedAgentsClient::memory_version`](crate::ManagedAgentsClient::memory_version) |
//!
//! **Memories are sensitive.** Agents write whatever they learned into them (user preferences,
//! project details, sometimes secrets), and memory versions keep every past content. Store and log
//! them accordingly.
//!
//! These endpoints use the `agent-memory-2026-07-22` beta *instead of* `managed-agents-2026-04-01`:
//! the memory guide documents that sending both is a `400`. Do not add the Managed Agents beta to
//! the [`ClientConfig`](claude_api_core::ClientConfig) of the client you use for memory calls.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, ApiResponse, Error, RequestOptions, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ManagedAgentsClient;
use crate::runtime::{ListRequest, Records, deferred, get, list_methods, memory_type, tagged_union};

/// The beta the memory store endpoints require, in place of the Managed Agents beta.
pub const AGENT_MEMORY_BETA: &str = "agent-memory-2026-07-22";

const STORES: &str = "v1/memory_stores";
const FULL_VIEW_MAX_LIMIT: u32 = 20;

/// The stream returned by [`ListMemoryStores::stream`].
pub type MemoryStoreStream = Records<MemoryStore>;
/// The stream returned by [`ListMemories::stream`].
pub type MemoryListItemStream = Records<MemoryListItem>;
/// The stream returned by [`ListMemoryVersions::stream`].
pub type MemoryVersionStream = Records<MemoryVersion>;

/// A memory store: a named, workspace-scoped container of memories that sessions mount as a
/// directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStore {
    /// Always `memory_store`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `memstore_…` ID.
    pub id: String,
    /// When the store was created.
    pub created_at: Timestamp,
    /// Name (1–255 characters); the mount-path slug derives from it.
    pub name: String,
    /// When the store was last updated.
    pub updated_at: Timestamp,
    /// When the store was archived.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// What the store contains (included in the agent's system prompt). Empty when unset.
    #[serde(default)]
    pub description: Option<String>,
    /// Caller-defined tags (up to 16 pairs).
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A memory: one text document at a path inside a store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// Always `memory`.
    #[serde(rename = "type", default = "memory_type")]
    pub object_type: String,
    /// `mem_…` ID, stable across renames.
    pub id: String,
    /// Lowercase hex SHA-256 of the UTF-8 content. Always populated.
    pub content_sha256: String,
    /// Content size in bytes. Always populated.
    pub content_size_bytes: u64,
    /// When the memory was created.
    pub created_at: Timestamp,
    /// The store (`memstore_…`).
    pub memory_store_id: String,
    /// The current version (`memver_…`), the authoritative head pointer.
    pub memory_version_id: String,
    /// Path inside the store, starting with `/`.
    pub path: String,
    /// When the memory was last updated.
    pub updated_at: Timestamp,
    /// The content: populated with [`MemoryView::Full`], `null` with [`MemoryView::Basic`].
    #[serde(default)]
    pub content: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A rolled-up directory returned by the memory list when `depth` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPrefix {
    /// The prefix, with a trailing `/`; pass it as `path_prefix` to drill in.
    pub path: String,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// An item of the memory list.
    #[derive(Eq)]
    pub enum MemoryListItem {
        /// `memory`.
        Memory(Memory) = "memory",
        /// `memory_prefix`: a directory with memories deeper than the requested `depth`.
        Prefix(MemoryPrefix) = "memory_prefix",
    }
}

claude_api_core::string_enum! {
    /// Which projection of a memory to return.
    pub enum MemoryView {
        /// `basic`: content omitted (the default).
        Basic = "basic",
        /// `full`: content included; caps list pages at 20 items.
        Full = "full",
    }
}

/// One immutable, attributed row of a memory's history.
///
/// A redacted version has `content`, `path`, `content_size_bytes` and `content_sha256` set to
/// `null`; branch on `redacted_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryVersion {
    /// Always `memory_version`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `memver_…` ID.
    pub id: String,
    /// When the version was written.
    pub created_at: Timestamp,
    /// The memory (`mem_…`); valid after the memory is deleted.
    pub memory_id: String,
    /// The store (`memstore_…`).
    pub memory_store_id: String,
    /// The mutation recorded.
    pub operation: MemoryVersionOperation,
    /// Content as of this version (`null` with the basic view, for deletions and when redacted).
    #[serde(default)]
    pub content: Option<String>,
    /// SHA-256 of the content (`null` for deletions and when redacted).
    #[serde(default)]
    pub content_sha256: Option<String>,
    /// Content size in bytes (`null` for deletions and when redacted).
    #[serde(default)]
    pub content_size_bytes: Option<u64>,
    /// Who made the write.
    #[serde(default)]
    pub created_by: Option<MemoryActor>,
    /// The memory's path at the time (`null` exactly when redacted).
    #[serde(default)]
    pub path: Option<String>,
    /// When the version was redacted.
    #[serde(default)]
    pub redacted_at: Option<Timestamp>,
    /// Who redacted it.
    #[serde(default)]
    pub redacted_by: Option<MemoryActor>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// The mutation a memory version records.
    pub enum MemoryVersionOperation {
        /// `created`.
        Created = "created",
        /// `modified`.
        Modified = "modified",
        /// `deleted`.
        Deleted = "deleted",
    }
}

tagged_union! {
    /// Who performed a memory write or redaction.
    #[derive(Eq)]
    pub enum MemoryActor {
        /// `session_actor`: an agent, through the mounted filesystem.
        Session(MemorySessionActor) = "session_actor",
        /// `api_actor`: a direct API call.
        Api(MemoryApiActor) = "api_actor",
        /// `user_actor`: a user in the Anthropic Console.
        User(MemoryUserActor) = "user_actor",
        /// `service_account_actor`: a workload authenticated as a service account.
        ServiceAccount(MemoryServiceAccountActor) = "service_account_actor",
    }
}

/// A `session_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySessionActor {
    /// The session (`sesn_…`).
    pub session_id: String,
}

/// An `api_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryApiActor {
    /// The API key (its ID, not the secret).
    pub api_key_id: String,
}

/// A `user_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryUserActor {
    /// The user (`user_…`).
    pub user_id: String,
}

/// A `service_account_actor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryServiceAccountActor {
    /// The service account (`svac_…`).
    pub service_account_id: String,
}

impl ManagedAgentsClient {
    /// Headers for memory store calls: the agent-memory beta (replacing the Managed Agents beta)
    /// and the selected workspace.
    fn memory_options(&self) -> RequestOptions {
        let options = RequestOptions::default().beta(AGENT_MEMORY_BETA);
        match self.workspace_id() {
            Some(workspace_id) => options.workspace_id(workspace_id),
            None => options,
        }
    }

    /// `GET /v1/memory_stores`: newest first. Sends `anthropic-beta: agent-memory-2026-07-22`
    /// instead of the Managed Agents beta.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/list)
    pub fn memory_stores(&self) -> ListMemoryStores {
        let path = Ok(ApiPath::new(STORES));
        ListMemoryStores { inner: ListRequest::new(self.api.clone(), self.memory_options(), path, Some(100)) }
    }

    /// `GET /v1/memory_stores/{memory_store_id}`. Sends `anthropic-beta: agent-memory-2026-07-22`
    /// instead of the Managed Agents beta.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/retrieve)
    pub async fn memory_store(&self, memory_store_id: &str) -> Result<ApiResponse<MemoryStore>> {
        get(&self.api, ApiPath::new(STORES).id(memory_store_id), &[], &self.memory_options()).await
    }

    /// `GET /v1/memory_stores/{memory_store_id}/memories`, in a stable server-defined order.
    /// Sends `anthropic-beta: agent-memory-2026-07-22` instead of the Managed Agents beta.
    /// Memories are sensitive (see the [crate docs](crate#sensitive-data)).
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/memories/list)
    pub fn memories(&self, memory_store_id: &str) -> ListMemories {
        let path = ApiPath::new(STORES).id(memory_store_id).map(|path| path.then("memories"));
        ListMemories { inner: ListRequest::new(self.api.clone(), self.memory_options(), path, Some(100)) }
    }

    /// `GET /v1/memory_stores/{memory_store_id}/memories/{memory_id}`. Sends
    /// `anthropic-beta: agent-memory-2026-07-22` instead of the Managed Agents beta.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/memories/retrieve)
    pub fn memory(&self, memory_store_id: &str, memory_id: &str) -> GetMemory {
        let path = ApiPath::new(STORES).id(memory_store_id).and_then(|path| path.then("memories").id(memory_id));
        GetMemory { client: self.clone(), path: deferred(path), view: None }
    }

    /// `GET /v1/memory_stores/{memory_store_id}/memory_versions`: newest first. Sends
    /// `anthropic-beta: agent-memory-2026-07-22` instead of the Managed Agents beta.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/memory_versions/list)
    pub fn memory_versions(&self, memory_store_id: &str) -> ListMemoryVersions {
        let path = ApiPath::new(STORES).id(memory_store_id).map(|path| path.then("memory_versions"));
        ListMemoryVersions { inner: ListRequest::new(self.api.clone(), self.memory_options(), path, None) }
    }

    /// `GET /v1/memory_stores/{memory_store_id}/memory_versions/{memory_version_id}`. A redacted
    /// version is a `200` with its content fields `null`. Sends
    /// `anthropic-beta: agent-memory-2026-07-22` instead of the Managed Agents beta.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/memory_stores/memory_versions/retrieve)
    pub fn memory_version(&self, memory_store_id: &str, memory_version_id: &str) -> GetMemoryVersion {
        let path = ApiPath::new(STORES)
            .id(memory_store_id)
            .and_then(|path| path.then("memory_versions").id(memory_version_id));
        GetMemoryVersion { client: self.clone(), path: deferred(path), view: None }
    }
}

/// Request builder for `GET /v1/memory_stores`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListMemoryStores {
    inner: ListRequest,
}

impl ListMemoryStores {
    list_methods!(MemoryStore, MemoryStoreStream, "Page size, 1 to 100 (server default 20).");

    /// `created_at[gte]`.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lte]`.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    fn check(&self) -> Option<String> {
        None
    }
}

/// Request builder for `GET /v1/memory_stores/{memory_store_id}/memories`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListMemories {
    inner: ListRequest,
}

impl ListMemories {
    list_methods!(
        MemoryListItem,
        MemoryListItemStream,
        "Page size, 1 to 100 (server default 20); at most 20 with [`MemoryView::Full`]. Memories and prefixes both count."
    );

    /// `depth`: `0` (the default) lists every descendant, `1` only immediate children, with deeper
    /// entries rolled up as [`MemoryListItem::Prefix`].
    pub fn depth(mut self, depth: u32) -> Self {
        self.inner.set("depth", depth.to_string());
        self
    }

    /// `path_prefix`: must end with `/` (checked before sending). It appears in request URLs, so
    /// keep secrets and personal data out of it.
    pub fn path_prefix(mut self, path_prefix: impl Into<String>) -> Self {
        self.inner.set("path_prefix", path_prefix);
        self
    }

    /// `view` (server default `basic`). `full` includes content and caps the page size at 20.
    pub fn view(mut self, view: MemoryView) -> Self {
        self.inner.set("view", view.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        if let Some(prefix) = self.inner.get("path_prefix") {
            if !prefix.ends_with('/') {
                return Some(format!("path_prefix must end with '/', got {prefix:?}"));
            }
        }
        match (self.inner.get("view"), self.inner.limit_value()) {
            (Some("full"), Some(limit)) if limit > FULL_VIEW_MAX_LIMIT => {
                Some(format!("limit is capped at {FULL_VIEW_MAX_LIMIT} with view=full, got {limit}"))
            }
            _ => None,
        }
    }
}

/// Request builder for `GET /v1/memory_stores/{memory_store_id}/memories/{memory_id}`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent"]
pub struct GetMemory {
    client: ManagedAgentsClient,
    path: std::result::Result<ApiPath, String>,
    view: Option<MemoryView>,
}

impl GetMemory {
    /// `view` (`full` includes the content).
    pub fn view(mut self, view: MemoryView) -> Self {
        self.view = Some(view);
        self
    }

    /// Fetches the memory.
    pub async fn send(&self) -> Result<ApiResponse<Memory>> {
        let query: Vec<(&str, String)> = self.view.iter().map(|view| ("view", view.as_str().to_owned())).collect();
        get(&self.client.api, self.path.clone().map_err(Error::InvalidArgument), &query, &self.client.memory_options())
            .await
    }
}

/// Request builder for `GET /v1/memory_stores/{memory_store_id}/memory_versions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListMemoryVersions {
    inner: ListRequest,
}

impl ListMemoryVersions {
    list_methods!(MemoryVersion, MemoryVersionStream, "Page size. The reference documents no bounds.");

    /// `api_key_id`: versions written by this API key.
    pub fn api_key_id(mut self, api_key_id: impl Into<String>) -> Self {
        self.inner.set("api_key_id", api_key_id);
        self
    }

    /// `created_at[gte]`.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lte]`.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `memory_id`: one memory's history (still valid after the memory is deleted).
    pub fn memory_id(mut self, memory_id: impl Into<String>) -> Self {
        self.inner.set("memory_id", memory_id);
        self
    }

    /// `operation`.
    pub fn operation(mut self, operation: MemoryVersionOperation) -> Self {
        self.inner.set("operation", operation.as_str());
        self
    }

    /// `service_account_id`: versions written by this service account.
    pub fn service_account_id(mut self, service_account_id: impl Into<String>) -> Self {
        self.inner.set("service_account_id", service_account_id);
        self
    }

    /// `session_id`: versions written by this session.
    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.inner.set("session_id", session_id);
        self
    }

    /// `view` (`full` includes each version's content).
    pub fn view(mut self, view: MemoryView) -> Self {
        self.inner.set("view", view.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        None
    }
}

/// Request builder for
/// `GET /v1/memory_stores/{memory_store_id}/memory_versions/{memory_version_id}`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent"]
pub struct GetMemoryVersion {
    client: ManagedAgentsClient,
    path: std::result::Result<ApiPath, String>,
    view: Option<MemoryView>,
}

impl GetMemoryVersion {
    /// `view` (`full` includes the content).
    pub fn view(mut self, view: MemoryView) -> Self {
        self.view = Some(view);
        self
    }

    /// Fetches the version.
    pub async fn send(&self) -> Result<ApiResponse<MemoryVersion>> {
        let query: Vec<(&str, String)> = self.view.iter().map(|view| ("view", view.as_str().to_owned())).collect();
        get(&self.client.api, self.path.clone().map_err(Error::InvalidArgument), &query, &self.client.memory_options())
            .await
    }
}
