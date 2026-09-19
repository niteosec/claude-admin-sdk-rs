//! Agent definitions:
//!
//! - `GET /v1/agents`
//! - `GET /v1/agents/{agent_id}`
//! - `GET /v1/agents/{agent_id}/versions`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! An agent is a reusable, versioned configuration: the model, system prompt, tools, MCP servers,
//! skills, permission policies and multi-agent roster a session runs with. Every change that alters
//! the configuration creates a new version, starting at 1; the version list is the full history.
//! Every union (tools, tool configs, permission policies, MCP servers, skills, effort, multi-agent
//! topology) keeps unknown kinds in an `Other` variant.

use std::collections::BTreeMap;
use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{TokenList, check_int32, config_union, token_list_methods};

const PATH: &str = "v1/agents";
const MAX_LIMIT: u32 = 100;

/// The stream returned by [`ListAgents::stream`] and [`ListAgentVersions::stream`].
pub type AgentStream = Pin<Box<dyn Stream<Item = Result<Agent>> + Send + 'static>>;

/// A Managed Agents agent, as one version of its configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    /// Always `agent`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `agent_…` ID.
    pub id: String,
    /// When the agent was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the agent was created.
    pub created_at: Timestamp,
    /// Description; `null` when unset.
    #[serde(default)]
    pub description: Option<String>,
    /// MCP servers the agent connects to. Their tools are configured by an `mcp_toolset` entry in
    /// [`Self::tools`] naming the server.
    pub mcp_servers: Vec<AgentMcpServer>,
    /// Caller-defined key-value metadata.
    pub metadata: BTreeMap<String, String>,
    /// The model and its settings.
    pub model: AgentModelConfig,
    /// Resolved coordinator topology; `null` for a single-agent configuration.
    #[serde(default)]
    pub multiagent: Option<AgentMultiagent>,
    /// Name.
    pub name: String,
    /// Skills, each resolved to a version.
    pub skills: Vec<AgentSkill>,
    /// System prompt; `null` when unset.
    #[serde(default)]
    pub system: Option<String>,
    /// Toolsets and custom tools.
    pub tools: Vec<AgentTool>,
    /// When the agent was last modified.
    pub updated_at: Timestamp,
    /// The configuration version this record describes. Starts at 1 and increments when the agent
    /// is modified.
    pub version: u32,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Model identifier and configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelConfig {
    /// Model ID, for example `claude-opus-5`. The reference lists known IDs but also accepts any
    /// string, so this is kept as text.
    pub id: String,
    /// How hard Claude works on each turn (`output_config.effort` on every Messages call). The
    /// server fills in the default when the agent omits it.
    #[serde(default)]
    pub effort: Option<AgentEffort>,
    /// Region for model inference; unset falls through to the workspace's `default_inference_geo`.
    #[serde(default)]
    pub inference_geo: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<AgentSpeed>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// An effort level, discriminated by `type`.
    pub enum AgentEffort {
        /// `low`: favours latency over reasoning depth.
        Low = "low",
        /// `medium`: balances latency and reasoning depth.
        Medium = "medium",
        /// `high`: favours reasoning depth.
        High = "high",
        /// `xhigh`: extra-high; not every model accepts it.
        Xhigh = "xhigh",
        /// `max`: favours reasoning depth over latency.
        Max = "max",
    }
}

claude_api_core::string_enum! {
    /// Inference speed mode.
    pub enum AgentSpeed {
        /// `standard`
        Standard = "standard",
        /// `fast`: faster output token generation at premium pricing.
        Fast = "fast",
    }
}

config_union! {
    /// Multi-agent topology, discriminated by `type`.
    pub enum AgentMultiagent {
        /// `coordinator`: the agent may spawn the listed agents as session threads.
        Coordinator(AgentCoordinator) = "coordinator",
    }
}

/// A resolved coordinator roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCoordinator {
    /// Agents the coordinator may spawn as session threads, each resolved to a version, and
    /// advisors the primary thread may consult.
    pub agents: Vec<AgentRosterEntry>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// A coordinator roster entry, discriminated by `type`.
    pub enum AgentRosterEntry {
        /// `agent`: another agent pinned to a version.
        Agent(AgentReference) = "agent",
        /// `advisor`: a model the session's primary thread may consult mid-turn.
        Advisor(AgentAdvisor) = "advisor",
    }
}

/// A resolved agent reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReference {
    /// `agent_…` ID.
    pub id: String,
    /// The pinned version.
    pub version: u32,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A platform advisor roster entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAdvisor {
    /// The advisor model ID.
    pub model: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// An MCP server the agent connects to, discriminated by `type`.
    pub enum AgentMcpServer {
        /// `url`: a remote MCP server reached over HTTP.
        Url(AgentMcpServerUrl) = "url",
    }
}

/// A URL MCP server definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMcpServerUrl {
    /// Server name, referenced by `mcp_toolset` entries.
    pub name: String,
    /// Server URL.
    pub url: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// A skill attached to the agent, discriminated by `type`.
    pub enum AgentSkill {
        /// `anthropic`: an Anthropic-managed skill.
        Anthropic(AgentSkillReference) = "anthropic",
        /// `custom`: a user-created skill.
        Custom(AgentSkillReference) = "custom",
    }
}

/// A skill resolved to a version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSkillReference {
    /// Skill ID: a short name such as `xlsx` for Anthropic skills, `skill_…` for custom ones.
    pub skill_id: String,
    /// The resolved version.
    pub version: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// An entry in the agent's `tools`, discriminated by `type`.
    pub enum AgentTool {
        /// `agent_toolset_20260401`: the pre-built tools (bash, file editing, search, web).
        AgentToolset20260401(AgentToolset) = "agent_toolset_20260401",
        /// `mcp_toolset`: the tools of one MCP server in [`Agent::mcp_servers`].
        McpToolset(AgentMcpToolset) = "mcp_toolset",
        /// `custom`: a tool the caller executes.
        Custom(AgentCustomTool) = "custom",
    }
}

/// The pre-built agent toolset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolset {
    /// Per-tool overrides. Documented as required; defaults to empty because the agent setup guide
    /// shows a response without it.
    #[serde(default)]
    pub configs: Vec<AgentToolConfig>,
    /// Resolved defaults for every tool in the toolset (permission policy `always_allow` unless
    /// configured).
    pub default_config: AgentToolsetDefaultConfig,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Resolved defaults for a toolset's tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolsetDefaultConfig {
    /// Whether the toolset's tools are enabled. Documented as required; optional here because the
    /// agent setup guide shows a response without it.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Permission policy for tool execution.
    pub permission_policy: AgentPermissionPolicy,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// A permission policy for tool execution, discriminated by `type`.
    pub enum AgentPermissionPolicy {
        /// `always_allow`: calls run without confirmation.
        AlwaysAllow = "always_allow",
        /// `always_ask`: calls wait for user confirmation.
        AlwaysAsk = "always_ask",
        /// `auto`: the server judges each call; safe calls run, high-risk calls are denied, and
        /// calls it cannot judge wait for confirmation.
        Auto = "auto",
    }
}

config_union! {
    /// A per-tool override in the agent toolset, discriminated by `type`.
    pub enum AgentToolConfig {
        /// `bash`
        Bash(AgentBuiltinToolConfig) = "bash",
        /// `edit`
        Edit(AgentBuiltinToolConfig) = "edit",
        /// `read`
        Read(AgentBuiltinToolConfig) = "read",
        /// `write`
        Write(AgentBuiltinToolConfig) = "write",
        /// `glob`
        Glob(AgentBuiltinToolConfig) = "glob",
        /// `grep`
        Grep(AgentBuiltinToolConfig) = "grep",
        /// `web_fetch`
        WebFetch(AgentWebFetchToolConfig) = "web_fetch",
        /// `web_search`
        WebSearch(AgentWebSearchToolConfig) = "web_search",
    }
}

/// Configuration of a pre-built tool without tool-specific settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBuiltinToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// The tool name (the same value as `type`).
    pub name: String,
    /// Permission policy for the tool.
    pub permission_policy: AgentPermissionPolicy,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Configuration of the `web_fetch` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWebFetchToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Always `web_fetch`.
    pub name: String,
    /// Permission policy for the tool.
    pub permission_policy: AgentPermissionPolicy,
    /// The only hosts the tool can reach.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    /// Hosts the tool cannot reach.
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    /// Cap on fetched page content included in the context.
    #[serde(default)]
    pub max_content_tokens: Option<u32>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Configuration of the `web_search` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWebSearchToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Always `web_search`.
    pub name: String,
    /// Permission policy for the tool.
    pub permission_policy: AgentPermissionPolicy,
    /// The only hosts the tool can reach.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    /// Hosts the tool cannot reach.
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    /// Approximate user location for localizing results.
    #[serde(default)]
    pub user_location: Option<AgentUserLocation>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Approximate user location for web search localization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentUserLocation {
    /// Location precision; only `approximate` is documented.
    #[serde(rename = "type")]
    pub location_type: String,
    /// City name.
    #[serde(default)]
    pub city: Option<String>,
    /// Two-letter ISO 3166-1 country code, uppercase.
    #[serde(default)]
    pub country: Option<String>,
    /// Region or state name.
    #[serde(default)]
    pub region: Option<String>,
    /// IANA time zone, for example `America/Los_Angeles`.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The tools of one MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMcpToolset {
    /// Per-tool overrides.
    #[serde(default)]
    pub configs: Vec<AgentMcpToolConfig>,
    /// Resolved defaults for every tool from the server (permission policy `always_ask` unless
    /// configured).
    pub default_config: AgentToolsetDefaultConfig,
    /// The `name` of the server in [`Agent::mcp_servers`].
    pub mcp_server_name: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A per-tool override in an MCP toolset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMcpToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// The MCP tool name.
    pub name: String,
    /// Permission policy for the tool.
    pub permission_policy: AgentPermissionPolicy,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A custom tool the caller executes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCustomTool {
    /// Description shown to the model.
    pub description: String,
    /// JSON Schema of the tool input.
    pub input_schema: AgentCustomToolInputSchema,
    /// Tool name.
    pub name: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// JSON Schema for a custom tool's input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCustomToolInputSchema {
    /// Always `object`.
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Property schemas by name.
    #[serde(default)]
    pub properties: Option<Map<String, Value>>,
    /// Required property names.
    #[serde(default)]
    pub required: Option<Vec<String>>,
    /// Other JSON Schema keywords.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/agents`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListAgents {
    inner: TokenList,
}

impl ListAgents {
    /// `created_at[gte]`: agents created at or after this time.
    pub fn created_at_gte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[gte]", at.to_string());
        self
    }

    /// `created_at[lte]`: agents created at or before this time.
    pub fn created_at_lte(mut self, at: Timestamp) -> Self {
        self.inner.set("created_at[lte]", at.to_string());
        self
    }

    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(Agent, TokenPage<Agent>, AgentStream, "Page size, 1 to 100 (server default 20).");
}

/// Request builder for `GET /v1/agents/{agent_id}/versions`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListAgentVersions {
    inner: TokenList,
}

impl ListAgentVersions {
    token_list_methods!(Agent, TokenPage<Agent>, AgentStream, "Page size, 1 to 100 (server default 20).");
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/agents`: agents in the workspace, each at its current version. Archived agents are
    /// excluded unless [`ListAgents::include_archived`] is set.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/agents/list)
    pub fn agents(&self) -> ListAgents {
        ListAgents { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), Some(MAX_LIMIT)) }
    }

    /// `GET /v1/agents/{agent_id}`: the agent at its most recent version.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/agents/retrieve)
    pub async fn agent(&self, agent_id: &str) -> Result<ApiResponse<Agent>> {
        let path = ApiPath::new(PATH).id(agent_id)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }

    /// `GET /v1/agents/{agent_id}?version=…`: the agent as it was at `version` (at least 1).
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/agents/retrieve)
    pub async fn agent_at_version(&self, agent_id: &str, version: u32) -> Result<ApiResponse<Agent>> {
        check_int32("version", version, 1)?;
        let path = ApiPath::new(PATH).id(agent_id)?;
        self.api.get_json_with(&path, &[("version", version.to_string())], &self.options()).await
    }

    /// `GET /v1/agents/{agent_id}/versions`: every version of the agent's configuration.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/agents/versions/list)
    pub fn agent_versions(&self, agent_id: &str) -> ListAgentVersions {
        let path = ApiPath::new(PATH).id(agent_id).map(|path| path.then("versions"));
        ListAgentVersions { inner: TokenList::new(self, path, Some(MAX_LIMIT)) }
    }
}
