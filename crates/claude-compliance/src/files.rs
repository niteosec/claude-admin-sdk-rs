//! Chat files, Claude-generated files and artifacts.
//!
//! - `GET /v1/compliance/apps/chats/files/{claude_file_id}`
//! - `GET /v1/compliance/apps/chats/files/{claude_file_id}/content`
//! - `GET /v1/compliance/apps/chats/generated-files/{claude_gen_file_id}`
//! - `GET /v1/compliance/apps/chats/generated-files/{claude_gen_file_id}/content`
//! - `GET /v1/compliance/apps/artifacts/{artifact_version_id}`
//! - `GET /v1/compliance/apps/artifacts/{artifact_version_id}/content`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*
//!
//! The content endpoints return what users uploaded and what Claude produced for them. Treat the
//! bytes as sensitive and the file names as untrusted input.

use claude_api_core::{ApiPath, ApiResponse, Download, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const FILES_PATH: &str = "v1/compliance/apps/chats/files";
const GENERATED_FILES_PATH: &str = "v1/compliance/apps/chats/generated-files";
const ARTIFACTS_PATH: &str = "v1/compliance/apps/artifacts";

/// Metadata of a file uploaded to chats or projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFile {
    /// `claude_file_…` ID.
    pub id: String,
    /// Chats the file is attached to.
    pub claude_chat_ids: Vec<String>,
    /// Upload time.
    pub created_at: Timestamp,
    /// Display name, if set. Untrusted user input.
    #[serde(default)]
    pub filename: Option<String>,
    /// Lowercase hex MD5 of the preferred downloadable variant, as recorded at upload. When it
    /// disagrees with the download's `Content-MD5`, the header is authoritative.
    #[serde(default)]
    pub md5: Option<String>,
    /// Chat message IDs the file is attached to.
    pub message_ids: Vec<String>,
    /// MIME type of the preferred downloadable variant; `null` when there is nothing to download.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Size in bytes of the preferred downloadable variant, if known.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Metadata of a file the assistant created through tool use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedFile {
    /// Opaque `claude_gen_file_…` ID.
    pub id: String,
    /// The chat the file belongs to.
    pub claude_chat_id: String,
    /// Creation time, when available.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Display name.
    pub filename: String,
    /// Lowercase hex MD5 of the stored file, when available.
    #[serde(default)]
    pub md5: Option<String>,
    /// MIME type, when available.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Size in bytes, when available.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Metadata of one artifact version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// `claude_artifact_…` ID.
    pub id: String,
    /// MIME-like type, for example `application/vnd.ant.code`.
    #[serde(default)]
    pub artifact_type: Option<String>,
    /// The chat the artifact belongs to.
    pub claude_chat_id: String,
    /// Version creation time.
    pub created_at: Timestamp,
    /// Lowercase hex MD5 of the UTF-8 content.
    pub md5: String,
    /// Size in bytes of the UTF-8 content.
    pub size_bytes: u64,
    /// Title.
    #[serde(default)]
    pub title: Option<String>,
    /// `claude_artifact_version_…` ID.
    pub version_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl crate::ComplianceClient {
    /// `GET /v1/compliance/apps/chats/files/{claude_file_id}`: file metadata without the bytes.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/files/retrieve)
    pub async fn chat_file(&self, claude_file_id: &str) -> Result<ApiResponse<ChatFile>> {
        self.api.get_json(&ApiPath::new(FILES_PATH).id(claude_file_id)?, &[]).await
    }

    /// `GET /v1/compliance/apps/chats/files/{claude_file_id}/content`: the uploaded bytes. Also
    /// downloads project files listed by
    /// [`ComplianceClient::project_attachments`](crate::ComplianceClient::project_attachments).
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/files/download)
    pub async fn chat_file_content(&self, claude_file_id: &str) -> Result<Download> {
        self.api.get_download(&ApiPath::new(FILES_PATH).id(claude_file_id)?.then("content"), &[]).await
    }

    /// `GET /v1/compliance/apps/chats/generated-files/{claude_gen_file_id}`: generated-file metadata.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/generated_files/retrieve)
    pub async fn generated_file(&self, claude_gen_file_id: &str) -> Result<ApiResponse<GeneratedFile>> {
        self.api.get_json(&ApiPath::new(GENERATED_FILES_PATH).id(claude_gen_file_id)?, &[]).await
    }

    /// `GET /v1/compliance/apps/chats/generated-files/{claude_gen_file_id}/content`: the generated
    /// bytes.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/chats/generated_files/download)
    pub async fn generated_file_content(&self, claude_gen_file_id: &str) -> Result<Download> {
        let path = ApiPath::new(GENERATED_FILES_PATH).id(claude_gen_file_id)?.then("content");
        self.api.get_download(&path, &[]).await
    }

    /// `GET /v1/compliance/apps/artifacts/{artifact_version_id}`: artifact version metadata.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/artifacts/retrieve)
    pub async fn artifact(&self, artifact_version_id: &str) -> Result<ApiResponse<Artifact>> {
        self.api.get_json(&ApiPath::new(ARTIFACTS_PATH).id(artifact_version_id)?, &[]).await
    }

    /// `GET /v1/compliance/apps/artifacts/{artifact_version_id}/content`: the artifact version's text.
    ///
    /// The reference documents no response schema for this endpoint, so the body is returned
    /// unparsed; [`Artifact::md5`] and [`Artifact::size_bytes`] describe the UTF-8 text.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/apps/artifacts/download)
    pub async fn artifact_content(&self, artifact_version_id: &str) -> Result<Download> {
        self.api.get_download(&ApiPath::new(ARTIFACTS_PATH).id(artifact_version_id)?.then("content"), &[]).await
    }
}
