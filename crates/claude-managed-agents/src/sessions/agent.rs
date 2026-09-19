//! The resolved agent definition a session or thread runs: a snapshot taken at creation time.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::runtime::tagged_union;

/// The resolved `agent` of a session: a snapshot of the agent at session creation time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgent {
    /// Always `agent`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `agent_…` ID.
    pub id: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// MCP servers the agent connects to.
    pub mcp_servers: Vec<SessionMcpServer>,
    /// Model and model settings.
    pub model: SessionModelConfig,
    /// The coordinator topology, for multiagent sessions.
    #[serde(default)]
    pub multiagent: Option<SessionMultiagent>,
    /// Name.
    pub name: String,
    /// Skills, resolved to concrete versions.
    pub skills: Vec<SessionSkill>,
    /// System prompt.
    #[serde(default)]
    pub system: Option<String>,
    /// Tools.
    pub tools: Vec<SessionAgentTool>,
    /// Agent version.
    pub version: u64,
}

/// The resolved agent of one session thread (a multiagent roster member). The roster itself is not
/// repeated here; read it from [`SessionAgent::multiagent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionThreadAgent {
    /// `agent_…` ID.
    pub id: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// MCP servers the agent connects to.
    pub mcp_servers: Vec<SessionMcpServer>,
    /// Model and model settings.
    pub model: SessionModelConfig,
    /// Name.
    pub name: String,
    /// Skills, resolved to concrete versions.
    pub skills: Vec<SessionSkill>,
    /// System prompt.
    #[serde(default)]
    pub system: Option<String>,
    /// Tools.
    pub tools: Vec<SessionAgentTool>,
    /// Agent version.
    pub version: u64,
}

/// The advisor roster entry: a model the primary thread may consult mid-turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAdvisor {
    /// The advisor model ID.
    pub model: String,
}

tagged_union! {
    /// What a session thread runs, or a member of a multiagent roster.
    #[derive(Eq)]
    #[allow(clippy::large_enum_variant, reason = "decoded records, rarely moved")]
    pub enum SessionRosterAgent {
        /// `agent`: a resolved agent snapshot.
        Agent(SessionThreadAgent) = "agent",
        /// `advisor`: the platform advisor.
        Advisor(SessionAdvisor) = "advisor",
    }
}

/// A multiagent coordinator topology with the full definition of each roster member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMultiagent {
    /// Always `coordinator`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Agents the coordinator may spawn as session threads.
    pub agents: Vec<SessionRosterAgent>,
}

/// An MCP server the agent connects to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMcpServer {
    /// Always `url`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Server name, referenced by `mcp_toolset` tools.
    pub name: String,
    /// Server URL.
    pub url: String,
}

/// Model identifier and settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModelConfig {
    /// Model ID, for example `claude-opus-5`. Documented as a list of known models or any string.
    pub id: String,
    /// How hard Claude works on each turn.
    #[serde(default)]
    pub effort: Option<SessionEffort>,
    /// Region for model inference; unset falls back to the workspace default.
    #[serde(default)]
    pub inference_geo: Option<String>,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<SessionModelSpeed>,
}

claude_api_core::string_enum! {
    /// Inference speed mode.
    pub enum SessionModelSpeed {
        /// `standard`.
        Standard = "standard",
        /// `fast`: faster output at premium pricing.
        Fast = "fast",
    }
}

tagged_union! {
    /// Effort level (`output_config.effort` on every Messages call the session makes).
    #[derive(Eq)]
    pub enum SessionEffort {
        /// `low`.
        Low = "low",
        /// `medium`.
        Medium = "medium",
        /// `high`.
        High = "high",
        /// `xhigh`: not all models accept it.
        Xhigh = "xhigh",
        /// `max`.
        Max = "max",
    }
}

tagged_union! {
    /// A resolved skill.
    #[derive(Eq)]
    pub enum SessionSkill {
        /// `anthropic`: an Anthropic-managed skill.
        Anthropic(SessionSkillVersion) = "anthropic",
        /// `custom`: a user-created skill.
        Custom(SessionSkillVersion) = "custom",
    }
}

/// A skill resolved to a concrete version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSkillVersion {
    /// Skill ID.
    pub skill_id: String,
    /// Version.
    pub version: String,
}

tagged_union! {
    /// A tool entry of the agent.
    #[derive(Eq)]
    pub enum SessionAgentTool {
        /// `agent_toolset_20260401`: the built-in agent tools.
        AgentToolset(SessionAgentToolset) = "agent_toolset_20260401",
        /// `mcp_toolset`: the tools of one MCP server.
        McpToolset(SessionMcpToolset) = "mcp_toolset",
        /// `custom`: a client-executed tool.
        Custom(SessionCustomTool) = "custom",
    }
}

/// The built-in agent toolset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentToolset {
    /// Per-tool configuration.
    pub configs: Vec<SessionAgentToolConfig>,
    /// Defaults for tools without an entry in `configs`.
    pub default_config: SessionToolDefaultConfig,
}

/// The tools of one MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMcpToolset {
    /// Per-tool configuration.
    pub configs: Vec<SessionMcpToolConfig>,
    /// Defaults for tools without an entry in `configs`.
    pub default_config: SessionToolDefaultConfig,
    /// The MCP server, by name.
    pub mcp_server_name: String,
}

/// A custom tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCustomTool {
    /// Description.
    pub description: String,
    /// JSON Schema of the input.
    pub input_schema: SessionCustomToolInputSchema,
    /// Name.
    pub name: String,
}

/// JSON Schema of a custom tool's input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCustomToolInputSchema {
    /// Always `object`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Property schemas.
    #[serde(default)]
    pub properties: Option<Map<String, Value>>,
    /// Required property names.
    #[serde(default)]
    pub required: Option<Vec<String>>,
}

/// Default enablement and permission policy of a toolset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionToolDefaultConfig {
    /// Whether tools are enabled.
    pub enabled: bool,
    /// Permission policy.
    pub permission_policy: SessionPermissionPolicy,
}

/// Configuration of one MCP tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMcpToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Tool name.
    pub name: String,
    /// Permission policy.
    pub permission_policy: SessionPermissionPolicy,
}

tagged_union! {
    /// Configuration of one built-in agent tool, discriminated by `type` (the tool).
    #[derive(Eq)]
    pub enum SessionAgentToolConfig {
        /// `bash`.
        Bash(SessionBuiltinToolConfig) = "bash",
        /// `edit`.
        Edit(SessionBuiltinToolConfig) = "edit",
        /// `read`.
        Read(SessionBuiltinToolConfig) = "read",
        /// `write`.
        Write(SessionBuiltinToolConfig) = "write",
        /// `glob`.
        Glob(SessionBuiltinToolConfig) = "glob",
        /// `grep`.
        Grep(SessionBuiltinToolConfig) = "grep",
        /// `web_fetch`.
        WebFetch(SessionWebFetchToolConfig) = "web_fetch",
        /// `web_search`.
        WebSearch(SessionWebSearchToolConfig) = "web_search",
    }
}

/// Configuration of a built-in tool without extra settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBuiltinToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Tool name (same as the `type`).
    pub name: String,
    /// Permission policy.
    pub permission_policy: SessionPermissionPolicy,
}

/// Configuration of the `web_fetch` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWebFetchToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Always `web_fetch`.
    pub name: String,
    /// Permission policy.
    pub permission_policy: SessionPermissionPolicy,
    /// Domains the tool may fetch.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    /// Domains the tool may not fetch.
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    /// Token cap on fetched content.
    #[serde(default)]
    pub max_content_tokens: Option<u64>,
}

/// Configuration of the `web_search` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWebSearchToolConfig {
    /// Whether the tool is enabled.
    pub enabled: bool,
    /// Always `web_search`.
    pub name: String,
    /// Permission policy.
    pub permission_policy: SessionPermissionPolicy,
    /// Domains results may come from.
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    /// Domains results may not come from.
    #[serde(default)]
    pub blocked_domains: Option<Vec<String>>,
    /// Approximate location for localizing results.
    #[serde(default)]
    pub user_location: Option<SessionUserLocation>,
}

/// Approximate user location for search localization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserLocation {
    /// Always `approximate`.
    #[serde(rename = "type")]
    pub kind: String,
    /// City.
    #[serde(default)]
    pub city: Option<String>,
    /// Two-letter ISO 3166-1 country code.
    #[serde(default)]
    pub country: Option<String>,
    /// Region or state.
    #[serde(default)]
    pub region: Option<String>,
    /// IANA timezone.
    #[serde(default)]
    pub timezone: Option<String>,
}

tagged_union! {
    /// Permission policy for tool execution.
    #[derive(Eq)]
    pub enum SessionPermissionPolicy {
        /// `always_allow`: calls run without confirmation.
        AlwaysAllow = "always_allow",
        /// `always_ask`: calls wait for user confirmation.
        AlwaysAsk = "always_ask",
        /// `auto`: the server judges each call (allow when safe, deny when high-risk, ask when it
        /// cannot decide).
        Auto = "auto",
    }
}
