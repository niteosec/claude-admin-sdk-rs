//! Dreams: asynchronous jobs that read a memory store and past session transcripts and write a
//! consolidated memory store (research preview).
//!
//! | Endpoint | Method |
//! |---|---|
//! | `GET /v1/dreams` | [`ManagedAgentsClient::dreams`](crate::ManagedAgentsClient::dreams) |
//! | `GET /v1/dreams/{dream_id}` | [`ManagedAgentsClient::dream`](crate::ManagedAgentsClient::dream) |
//!
//! Dream endpoints are gated by the `dreaming-2026-04-21` beta, sent on top of the Managed Agents
//! beta. Anthropic describes the Dreams API shapes as volatile.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*

use claude_api_core::{ApiPath, ApiResponse, RequestOptions, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ManagedAgentsClient;
use crate::runtime::{ListRequest, Records, get, list_methods, tagged_union};
use crate::sessions::SessionModelSpeed;

/// The beta the dream endpoints require, in addition to the Managed Agents beta.
pub const DREAMING_BETA: &str = "dreaming-2026-04-21";

const DREAMS: &str = "v1/dreams";

/// The stream returned by [`ListDreams::stream`].
pub type DreamStream = Records<Dream>;

/// A memory-consolidation job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dream {
    /// Always `dream`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Dream ID.
    pub id: String,
    /// When the dream was archived.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the dream was created.
    pub created_at: Timestamp,
    /// When the dream finished.
    #[serde(default)]
    pub ended_at: Option<Timestamp>,
    /// Failure detail, when `status` is `failed`.
    #[serde(default)]
    pub error: Option<DreamError>,
    /// The memory store and sessions read.
    pub inputs: Vec<DreamInput>,
    /// Caller instructions.
    #[serde(default)]
    pub instructions: Option<String>,
    /// Model used by every pipeline stage.
    pub model: DreamModelConfig,
    /// Where the consolidated memories go.
    pub output_behavior: DreamOutputBehavior,
    /// Output memory stores (may be briefly empty while `running`).
    pub outputs: Vec<DreamOutput>,
    /// Session ID; the reference does not describe it.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Lifecycle status.
    pub status: DreamStatus,
    /// Cumulative token usage.
    pub usage: DreamUsage,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Why a dream failed. `type` is documented as a free string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamError {
    /// Error type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Human-readable detail.
    pub message: String,
}

tagged_union! {
    /// An input of a dream.
    #[derive(Eq)]
    pub enum DreamInput {
        /// `memory_store`: the store to consolidate.
        MemoryStore(DreamMemoryStoreRef) = "memory_store",
        /// `sessions`: transcripts to mine.
        Sessions(DreamSessionsInput) = "sessions",
    }
}

/// A memory store reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamMemoryStoreRef {
    /// `memstore_…` ID.
    pub memory_store_id: String,
}

/// The `sessions` input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamSessionsInput {
    /// Session IDs.
    pub session_ids: Vec<String>,
}

/// Model settings of a dream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamModelConfig {
    /// Model ID, for example `claude-opus-5`.
    pub id: String,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<SessionModelSpeed>,
}

tagged_union! {
    /// Where a dream writes.
    #[derive(Eq)]
    pub enum DreamOutputBehavior {
        /// `create_new`: a new store cloned from the input (the default).
        CreateNew = "create_new",
        /// `update_existing`: consolidate into an existing store in place.
        UpdateExisting(DreamMemoryStoreRef) = "update_existing",
    }
}

/// An output memory store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamOutput {
    /// Always `memory_store`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `memstore_…` ID.
    pub memory_store_id: String,
}

claude_api_core::string_enum! {
    /// Dream lifecycle status.
    pub enum DreamStatus {
        /// `pending`.
        Pending = "pending",
        /// `running`.
        Running = "running",
        /// `completed`.
        Completed = "completed",
        /// `failed`.
        Failed = "failed",
        /// `canceled`.
        Canceled = "canceled",
    }
}

/// Cumulative token usage of a dream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamUsage {
    /// Tokens written to the prompt cache.
    pub cache_creation_input_tokens: u64,
    /// Tokens read from the prompt cache.
    pub cache_read_input_tokens: u64,
    /// Uncached input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
}

impl ManagedAgentsClient {
    fn dream_options(&self) -> RequestOptions {
        self.options().beta(DREAMING_BETA)
    }

    /// `GET /v1/dreams`. Sends `anthropic-beta: managed-agents-2026-04-01,dreaming-2026-04-21`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/dreams/list)
    pub fn dreams(&self) -> ListDreams {
        ListDreams { inner: ListRequest::new(self.api.clone(), self.dream_options(), Ok(ApiPath::new(DREAMS)), None) }
    }

    /// `GET /v1/dreams/{dream_id}`. Sends
    /// `anthropic-beta: managed-agents-2026-04-01,dreaming-2026-04-21`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/dreams/retrieve)
    pub async fn dream(&self, dream_id: &str) -> Result<ApiResponse<Dream>> {
        get(&self.api, ApiPath::new(DREAMS).id(dream_id), &[], &self.dream_options()).await
    }
}

/// Request builder for `GET /v1/dreams`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListDreams {
    inner: ListRequest,
}

impl ListDreams {
    list_methods!(Dream, DreamStream, "Page size. The reference documents no bounds.");

    /// `created_at[gt]`.
    pub fn created_at_gt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gt]", at.to_string());
        self
    }

    /// `created_at[lt]`.
    pub fn created_at_lt(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lt]", at.to_string());
        self
    }

    /// `include_archived`.
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    /// Adds a `statuses` filter; repeated values match any of them.
    pub fn status(mut self, status: DreamStatus) -> Self {
        self.inner.push("statuses", status.as_str());
        self
    }

    fn check(&self) -> Option<String> {
        None
    }
}
