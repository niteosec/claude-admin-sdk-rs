//! Environments:
//!
//! - `GET /v1/environments`
//! - `GET /v1/environments/{environment_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! An environment is where sessions run: an Anthropic cloud sandbox with a network policy and
//! pre-installed packages, or a self-hosted runner. The self-hosted worker protocol
//! (`/v1/environments/{environment_id}/work/…`) is out of scope for this read-only client.

use std::collections::BTreeMap;
use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{TokenList, config_union, token_list_methods};

const PATH: &str = "v1/environments";
const MAX_LIMIT: u32 = 1000;

/// The stream returned by [`ListEnvironments::stream`].
pub type EnvironmentStream = Pin<Box<dyn Stream<Item = Result<Environment>> + Send + 'static>>;

/// A cloud or self-hosted environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    /// Always `environment`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `env_…` ID.
    pub id: String,
    /// When the environment was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// Cloud or self-hosted configuration.
    pub config: EnvironmentConfig,
    /// When the environment was created.
    pub created_at: Timestamp,
    /// Description; `null` when unset.
    #[serde(default)]
    pub description: Option<String>,
    /// Caller-defined key-value metadata.
    pub metadata: BTreeMap<String, String>,
    /// Human-readable name.
    pub name: String,
    /// When the environment was last updated.
    pub updated_at: Timestamp,
    /// Visibility: the whole organization, or only the owning account.
    #[serde(default)]
    pub scope: Option<EnvironmentScope>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Who can see an environment.
    pub enum EnvironmentScope {
        /// `organization`: every account.
        Organization = "organization",
        /// `account`: only the owning account.
        Account = "account",
    }
}

config_union! {
    /// Environment configuration, discriminated by `type`.
    pub enum EnvironmentConfig {
        /// `cloud`: an Anthropic cloud sandbox.
        Cloud(EnvironmentCloudConfig) = "cloud",
        /// `self_hosted`: sessions run on the customer's own workers.
        SelfHosted = "self_hosted",
    }
}

/// Cloud sandbox configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentCloudConfig {
    /// Network policy.
    pub networking: EnvironmentNetworking,
    /// Packages installed in the sandbox.
    pub packages: EnvironmentPackages,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

config_union! {
    /// A sandbox network policy, discriminated by `type`.
    pub enum EnvironmentNetworking {
        /// `unrestricted`: any outbound host.
        Unrestricted = "unrestricted",
        /// `limited`: only the listed hosts, plus the optional categories.
        Limited(EnvironmentLimitedNetwork) = "limited",
    }
}

/// A `limited` network policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentLimitedNetwork {
    /// Also allows the MCP server endpoints configured on the agent.
    pub allow_mcp_servers: bool,
    /// Also allows public package registries (PyPI, npm, …).
    pub allow_package_managers: bool,
    /// Domains the container can reach.
    pub allowed_hosts: Vec<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Packages installed in a cloud sandbox, by package manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentPackages {
    /// `packages` when present.
    #[serde(rename = "type", default)]
    pub packages_type: Option<String>,
    /// Ubuntu/Debian packages.
    pub apt: Vec<String>,
    /// Rust crates.
    pub cargo: Vec<String>,
    /// Ruby gems.
    pub gem: Vec<String>,
    /// Go packages.
    pub go: Vec<String>,
    /// Node.js packages.
    pub npm: Vec<String>,
    /// Python packages.
    pub pip: Vec<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Request builder for `GET /v1/environments`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListEnvironments {
    inner: TokenList,
}

impl ListEnvironments {
    /// `include_archived` (server default `false`).
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(
        Environment,
        TokenPage<Environment>,
        EnvironmentStream,
        "Page size, 1 to 1000 (server default 20)."
    );
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/environments`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/environments/list)
    pub fn environments(&self) -> ListEnvironments {
        ListEnvironments { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), Some(MAX_LIMIT)) }
    }

    /// `GET /v1/environments/{environment_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/environments/retrieve)
    pub async fn environment(&self, environment_id: &str) -> Result<ApiResponse<Environment>> {
        let path = ApiPath::new(PATH).id(environment_id)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }
}
