//! Vaults and their credentials:
//!
//! - `GET /v1/vaults`
//! - `GET /v1/vaults/{vault_id}`
//! - `GET /v1/vaults/{vault_id}/credentials`
//! - `GET /v1/vaults/{vault_id}/credentials/{credential_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! A vault holds the credentials agents use during sessions: OAuth and static bearer credentials for
//! MCP servers, and secrets substituted into outbound requests through environment variables.
//!
//! **What is not returned.** The secret values (`token`, `access_token`, `refresh_token`,
//! `client_secret`, `secret_value`) are write-only and never appear in responses; a credential
//! record describes where and how a secret is used, not the secret. Should a field with one of those
//! names ever arrive, the `Debug` output of these types masks it.

use std::collections::BTreeMap;
use std::fmt;
use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, PageToken, Result};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{Paged, RedactedMap, TokenList, config_union, token_list_methods};

const PATH: &str = "v1/vaults";
const MAX_LIMIT: u32 = 100;

/// The stream returned by [`ListVaults::stream`].
pub type VaultStream = Pin<Box<dyn Stream<Item = Result<Vault>> + Send + 'static>>;

/// The stream returned by [`ListVaultCredentials::stream`].
pub type VaultCredentialStream = Pin<Box<dyn Stream<Item = Result<VaultCredential>> + Send + 'static>>;

/// A vault that stores credentials for agents to use during sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    /// Always `vault`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `vlt_…` ID.
    pub id: String,
    /// When the vault was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// When the vault was created.
    pub created_at: Timestamp,
    /// Human-readable name.
    pub display_name: String,
    /// Caller-defined key-value metadata.
    pub metadata: BTreeMap<String, String>,
    /// When the vault was last updated.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A credential stored in a vault. Carries no secret values.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredential {
    /// Always `vault_credential`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `vcrd_…` ID.
    pub id: String,
    /// When the credential was archived; `null` while active.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// How the credential authenticates.
    pub auth: VaultCredentialAuth,
    /// When the credential was created.
    pub created_at: Timestamp,
    /// Caller-defined key-value metadata.
    pub metadata: BTreeMap<String, String>,
    /// When the credential was last updated.
    pub updated_at: Timestamp,
    /// The vault the credential belongs to.
    pub vault_id: String,
    /// Human-readable name; `null` when unset.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Fields not in the documented schema. Masked by name in `Debug` output when secret-named.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for VaultCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultCredential")
            .field("object_type", &self.object_type)
            .field("id", &self.id)
            .field("archived_at", &self.archived_at)
            .field("auth", &self.auth)
            .field("created_at", &self.created_at)
            .field("metadata", &self.metadata)
            .field("updated_at", &self.updated_at)
            .field("vault_id", &self.vault_id)
            .field("display_name", &self.display_name)
            .field("extra", &RedactedMap(&self.extra))
            .finish()
    }
}

config_union! {
    /// How a credential authenticates, discriminated by `type`. Unknown kinds keep their raw object
    /// (secret-named fields are masked in `Debug`).
    pub enum VaultCredentialAuth {
        /// `mcp_oauth`: OAuth for an MCP server.
        McpOAuth(VaultCredentialMcpOAuth) = "mcp_oauth",
        /// `static_bearer`: a static bearer token for an MCP server.
        StaticBearer(VaultCredentialStaticBearer) = "static_bearer",
        /// `environment_variable`: a secret substituted into outbound requests at egress.
        EnvironmentVariable(VaultCredentialEnvironmentVariable) = "environment_variable",
    }
}

/// OAuth credential details for an MCP server. The access and refresh tokens are not returned.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialMcpOAuth {
    /// The MCP server this credential authenticates against.
    pub mcp_server_url: String,
    /// When the access token expires.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// Refresh configuration; `null` when the credential cannot refresh.
    #[serde(default)]
    pub refresh: Option<VaultCredentialOAuthRefresh>,
    /// Fields not in the documented schema. Masked by name in `Debug` output when secret-named.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for VaultCredentialMcpOAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultCredentialMcpOAuth")
            .field("mcp_server_url", &self.mcp_server_url)
            .field("expires_at", &self.expires_at)
            .field("refresh", &self.refresh)
            .field("extra", &RedactedMap(&self.extra))
            .finish()
    }
}

/// OAuth refresh configuration. The client secret is not returned.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialOAuthRefresh {
    /// OAuth client ID.
    pub client_id: String,
    /// Token endpoint used to refresh the access token.
    pub token_endpoint: String,
    /// How the client authenticates to the token endpoint.
    pub token_endpoint_auth: VaultCredentialTokenEndpointAuth,
    /// OAuth resource indicator.
    #[serde(default)]
    pub resource: Option<String>,
    /// OAuth scope for the refresh request.
    #[serde(default)]
    pub scope: Option<String>,
    /// Fields not in the documented schema. Masked by name in `Debug` output when secret-named.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for VaultCredentialOAuthRefresh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultCredentialOAuthRefresh")
            .field("client_id", &self.client_id)
            .field("token_endpoint", &self.token_endpoint)
            .field("token_endpoint_auth", &self.token_endpoint_auth)
            .field("resource", &self.resource)
            .field("scope", &self.scope)
            .field("extra", &RedactedMap(&self.extra))
            .finish()
    }
}

config_union! {
    /// Token endpoint client authentication, discriminated by `type`.
    pub enum VaultCredentialTokenEndpointAuth {
        /// `none`: no client authentication.
        None = "none",
        /// `client_secret_basic`: HTTP Basic with the client credentials.
        ClientSecretBasic = "client_secret_basic",
        /// `client_secret_post`: client credentials in the POST body.
        ClientSecretPost = "client_secret_post",
    }
}

/// Static bearer credential details for an MCP server. The token is not returned.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialStaticBearer {
    /// The MCP server this credential authenticates against.
    pub mcp_server_url: String,
    /// Fields not in the documented schema. Masked by name in `Debug` output when secret-named.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for VaultCredentialStaticBearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultCredentialStaticBearer")
            .field("mcp_server_url", &self.mcp_server_url)
            .field("extra", &RedactedMap(&self.extra))
            .finish()
    }
}

/// Environment variable credential details. The secret value is never returned.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialEnvironmentVariable {
    /// Where in outbound requests the secret is substituted.
    pub injection_location: VaultCredentialInjectionLocation,
    /// Hosts the secret is substituted on.
    pub networking: VaultCredentialNetworking,
    /// Name of the environment variable holding the placeholder.
    pub secret_name: String,
    /// Fields not in the documented schema. Masked by name in `Debug` output when secret-named.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for VaultCredentialEnvironmentVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultCredentialEnvironmentVariable")
            .field("injection_location", &self.injection_location)
            .field("networking", &self.networking)
            .field("secret_name", &self.secret_name)
            .field("extra", &RedactedMap(&self.extra))
            .finish()
    }
}

/// Where an environment variable secret is substituted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialInjectionLocation {
    /// Whether the placeholder is substituted in request bodies.
    pub body: bool,
    /// Whether the placeholder is substituted in request header values.
    pub header: bool,
}

config_union! {
    /// Outbound hosts an environment variable secret is substituted on, discriminated by `type`.
    pub enum VaultCredentialNetworking {
        /// `unrestricted`: any host the session's environment network policy allows.
        Unrestricted = "unrestricted",
        /// `limited`: only the listed hosts.
        Limited(VaultCredentialLimitedNetworking) = "limited",
    }
}

/// The hosts a `limited` secret is substituted on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialLimitedNetworking {
    /// Hostnames; an entry matches the host exactly, and a `*.` entry matches any subdomain of the
    /// named domain but not the domain itself.
    pub allowed_hosts: Vec<String>,
}

/// A page of vaults. The reference marks `data` optional; a missing list decodes as empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultPage {
    /// The vaults.
    #[serde(default)]
    pub data: Vec<Vault>,
    /// Token for the next page, `null` at the end.
    #[serde(default)]
    pub next_page: Option<PageToken>,
}

/// A page of vault credentials. The reference marks `data` optional; a missing list decodes as
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultCredentialPage {
    /// The credentials.
    #[serde(default)]
    pub data: Vec<VaultCredential>,
    /// Token for the next page, `null` at the end.
    #[serde(default)]
    pub next_page: Option<PageToken>,
}

impl Paged<Vault> for VaultPage {
    fn into_parts(self) -> (Vec<Vault>, Option<PageToken>) {
        (self.data, self.next_page)
    }
}

impl Paged<VaultCredential> for VaultCredentialPage {
    fn into_parts(self) -> (Vec<VaultCredential>, Option<PageToken>) {
        (self.data, self.next_page)
    }
}

/// Request builder for `GET /v1/vaults`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListVaults {
    inner: TokenList,
}

impl ListVaults {
    /// `include_archived`: include archived vaults.
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(Vault, VaultPage, VaultStream, "Page size, 1 to 100 (server default 20).");
}

/// Request builder for `GET /v1/vaults/{vault_id}/credentials`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListVaultCredentials {
    inner: TokenList,
}

impl ListVaultCredentials {
    /// `include_archived`: include archived credentials.
    pub fn include_archived(mut self, include: bool) -> Self {
        self.inner.set("include_archived", include.to_string());
        self
    }

    token_list_methods!(
        VaultCredential,
        VaultCredentialPage,
        VaultCredentialStream,
        "Page size, 1 to 100 (server default 20)."
    );
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/vaults`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/vaults/list)
    pub fn vaults(&self) -> ListVaults {
        ListVaults { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), Some(MAX_LIMIT)) }
    }

    /// `GET /v1/vaults/{vault_id}`.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/vaults/retrieve)
    pub async fn vault(&self, vault_id: &str) -> Result<ApiResponse<Vault>> {
        let path = ApiPath::new(PATH).id(vault_id)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }

    /// `GET /v1/vaults/{vault_id}/credentials`. Secret values are not returned.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/vaults/credentials/list)
    pub fn vault_credentials(&self, vault_id: &str) -> ListVaultCredentials {
        let path = ApiPath::new(PATH).id(vault_id).map(|path| path.then("credentials"));
        ListVaultCredentials { inner: TokenList::new(self, path, Some(MAX_LIMIT)) }
    }

    /// `GET /v1/vaults/{vault_id}/credentials/{credential_id}`. Secret values are not returned.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/vaults/credentials/retrieve)
    pub async fn vault_credential(&self, vault_id: &str, credential_id: &str) -> Result<ApiResponse<VaultCredential>> {
        let path = ApiPath::new(PATH).id(vault_id)?.then("credentials").id(credential_id)?;
        self.api.get_json_with(&path, &[], &self.options()).await
    }
}
