//! Skills:
//!
//! - `GET /v1/skills`
//! - `GET /v1/skills/{skill_id}`
//! - `GET /v1/skills/{skill_id}/versions`
//! - `GET /v1/skills/{skill_id}/versions/{version}`
//! - `GET /v1/skills/{skill_id}/versions/{version}/content`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! These requests carry only the Managed Agents beta. The reference notes that requests carrying
//! the `skills-2025-10-02` beta address versions by Unix epoch timestamp instead of version ID, so
//! this client does not send it: versions are addressed by their ID (or `latest`).

use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, Download, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{TokenList, token_list_methods};

const PATH: &str = "v1/skills";
const MAX_LIMIT: u32 = 1000;

/// The stream returned by [`ListSkills::stream`].
pub type SkillStream = Pin<Box<dyn Stream<Item = Result<Skill>> + Send + 'static>>;

/// The stream returned by [`ListSkillVersions::stream`].
pub type SkillVersionStream = Pin<Box<dyn Stream<Item = Result<SkillVersion>> + Send + 'static>>;

/// A skill: a packaged set of instructions and files agents can load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// Always `skill`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Skill ID. Format and length may change.
    pub id: String,
    /// When the skill was created.
    pub created_at: Timestamp,
    /// Single-line label (at most 255 characters), derived from the SKILL.md `name` when not set.
    /// Not unique.
    pub display_name: String,
    /// ID of the newest version, which `latest` resolves to.
    pub latest_version_id: String,
    /// Where the skill comes from.
    pub source: SkillSource,
    /// When the skill was last updated.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A skill's origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    /// The origin.
    #[serde(rename = "type")]
    pub source_type: SkillSourceType,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Where a skill comes from.
    pub enum SkillSourceType {
        /// `custom`: authored by the platform user; private to their workspace.
        Custom = "custom",
        /// `anthropic`: published by Anthropic; shared and read-only.
        Anthropic = "anthropic",
        /// `anthropic_example`: an Anthropic-published sample skill.
        AnthropicExample = "anthropic_example",
        /// `plugin`: resolved from an installed plugin.
        Plugin = "plugin",
    }
}

/// One immutable version of a skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillVersion {
    /// Always `skill_version`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Version ID; addresses the version in paths and pins it in references.
    pub id: String,
    /// When the version was created.
    pub created_at: Timestamp,
    /// Description extracted from the version's SKILL.md.
    pub description: String,
    /// The skill's immutable kebab-case slug; also the top-level directory of its files and the base
    /// name of a downloaded archive.
    pub name: String,
    /// The skill this version belongs to.
    pub skill_id: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/skills`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSkills {
    inner: TokenList,
}

impl ListSkills {
    /// `source`: only skills from this source. The reference documents `custom` and `anthropic`.
    pub fn source(mut self, source: SkillSourceType) -> Self {
        self.inner.set("source", source.as_str());
        self
    }

    token_list_methods!(Skill, TokenPage<Skill>, SkillStream, "Page size, 1 to 1000 (server default 20).");
}

/// Request builder for `GET /v1/skills/{skill_id}/versions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListSkillVersions {
    inner: TokenList,
}

impl ListSkillVersions {
    token_list_methods!(
        SkillVersion,
        TokenPage<SkillVersion>,
        SkillVersionStream,
        "Page size, 1 to 1000 (server default 20)."
    );
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/skills`: custom and Anthropic skills visible to the workspace.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/skills/list)
    pub fn skills(&self) -> ListSkills {
        ListSkills { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), Some(MAX_LIMIT)) }
    }

    /// `GET /v1/skills/{skill_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/skills/retrieve)
    pub async fn skill(&self, skill_id: &str) -> Result<ApiResponse<Skill>> {
        let path = ApiPath::new(PATH).id(skill_id)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }

    /// `GET /v1/skills/{skill_id}/versions`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/skills/versions/list)
    pub fn skill_versions(&self, skill_id: &str) -> ListSkillVersions {
        let path = ApiPath::new(PATH).id(skill_id).map(|path| path.then("versions"));
        ListSkillVersions { inner: TokenList::new(self, path, Some(MAX_LIMIT)) }
    }

    /// `GET /v1/skills/{skill_id}/versions/{version}`. `version` is a version ID or the literal
    /// `latest`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/skills/versions/retrieve)
    pub async fn skill_version(&self, skill_id: &str, version: &str) -> Result<ApiResponse<SkillVersion>> {
        let path = ApiPath::new(PATH).id(skill_id)?.then("versions").id(version)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }

    /// `GET /v1/skills/{skill_id}/versions/{version}/content`: the version's files as a zip archive.
    /// `version` is a version ID.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/skills/versions/download)
    pub async fn skill_version_content(&self, skill_id: &str, version: &str) -> Result<Download> {
        let path = ApiPath::new(PATH).id(skill_id)?.then("versions").id(version)?.then("content");
        self.api.get_download_with(&path, &[], &self.options()).await
    }
}
