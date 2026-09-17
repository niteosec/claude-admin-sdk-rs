//! Customer-managed encryption key (CMEK) configurations.
//!
//! - `GET /v1/organizations/external_keys`
//! - `GET /v1/organizations/external_keys/{external_key_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Error, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::list::{TokenList, token_list_methods};
use crate::union::tagged_union;

const MAX_ID_LEN: usize = 2048;

/// An external key configuration. Newest first in lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalKey {
    /// Always `external_key`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `ekey_…` ID, or the AWS KMS key ARN on Claude Platform on AWS.
    pub id: String,
    /// Whether any live or archived workspace encrypts with this configuration.
    pub attachment: ExternalKeyAttachment,
    /// When the configuration was created.
    pub created_at: Timestamp,
    /// Display name, if set.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Data residency geo.
    pub geo: String,
    /// KMS provider identity.
    pub provider_config: ExternalKeyProviderConfig,
    /// When the configuration was last updated.
    pub updated_at: Timestamp,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// Attachment state, discriminated by `type`.
    pub enum ExternalKeyAttachment {
        /// `attached`: used by the encryption path.
        Attached = "attached",
        /// `unattached`: inert.
        Unattached = "unattached",
    }
}

tagged_union! {
    /// KMS provider configuration, discriminated by `type`.
    pub enum ExternalKeyProviderConfig {
        /// `aws`.
        Aws(AwsExternalKeyConfig) = "aws",
        /// `gcp`.
        Gcp(GcpExternalKeyConfig) = "gcp",
        /// `azure`.
        Azure(AzureExternalKeyConfig) = "azure",
    }
}

/// `aws` provider configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwsExternalKeyConfig {
    /// Full ARN of the KMS key.
    pub kms_arn: String,
    /// AWS region; derived from `kms_arn` when absent.
    #[serde(default)]
    pub region: Option<String>,
    /// **Deprecated** upstream and ignored by the service.
    #[serde(default)]
    pub role_arn: Option<String>,
}

/// `gcp` provider configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcpExternalKeyConfig {
    /// Full resource name of the Cloud KMS key.
    pub key_name: String,
}

/// `azure` provider configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AzureExternalKeyConfig {
    /// Key name within the vault.
    pub key_name: String,
    /// Azure AD tenant ID.
    pub tenant_id: String,
    /// Key Vault data-plane URI.
    pub vault_uri: String,
    /// Single-tenant application (client) ID; `None` for Anthropic's multitenant app.
    #[serde(default)]
    pub client_id: Option<String>,
}

/// Request builder for `GET /v1/organizations/external_keys`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListExternalKeys {
    inner: TokenList,
}

impl ListExternalKeys {
    token_list_methods!(ExternalKey, "Page size, 1 to 100 (server default 20).");
}

impl AdminClient {
    /// `GET /v1/organizations/external_keys`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/external_keys/list>
    pub fn external_keys(&self) -> ListExternalKeys {
        let path = Ok(ApiPath::new("v1/organizations/external_keys"));
        ListExternalKeys { inner: TokenList::new(self.api.clone(), path, 100) }
    }

    /// `GET /v1/organizations/external_keys/{external_key_id}`. The ID may be an AWS KMS key ARN; it
    /// is sent as one encoded path segment. At most 2048 characters.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/external_keys/retrieve>
    pub async fn external_key(&self, external_key_id: &str) -> Result<ApiResponse<ExternalKey>> {
        if external_key_id.chars().count() > MAX_ID_LEN {
            return Err(Error::InvalidArgument(format!("external_key_id is longer than {MAX_ID_LEN} characters")));
        }
        self.api.get_json(&ApiPath::new("v1/organizations/external_keys").id(external_key_id)?, &[]).await
    }
}
