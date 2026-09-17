//! The organization itself and its Compliance Settings.
//!
//! - `GET /v1/organizations/me`
//! - `GET /v1/organizations/compliance_settings`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use claude_api_core::{ApiPath, ApiResponse, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::AdminClient;
use crate::union::tagged_union;

/// The organization the credential belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Always `organization`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Organization ID (a UUID).
    pub id: String,
    /// Organization name.
    pub name: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The organization's Compliance Settings: a singleton, one per organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceSettings {
    /// Always `compliance_settings`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Whether the Compliance API is enabled. An organization with a parent reads the parent's state.
    pub state: ComplianceSettingsState,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// Whether the Compliance API is enabled, discriminated by `type`.
    pub enum ComplianceSettingsState {
        /// `enabled`.
        Enabled = "enabled",
        /// `disabled`.
        Disabled = "disabled",
    }
}

impl AdminClient {
    /// `GET /v1/organizations/me`: the organization the credential belongs to.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/retrieve>
    pub async fn organization(&self) -> Result<ApiResponse<Organization>> {
        self.api.get_json(&ApiPath::new("v1/organizations/me"), &[]).await
    }

    /// `GET /v1/organizations/compliance_settings`.
    ///
    /// Reference: <https://platform.claude.com/docs/en/api/beta/organization/compliance_settings/retrieve>
    pub async fn compliance_settings(&self) -> Result<ApiResponse<ComplianceSettings>> {
        self.api.get_json(&ApiPath::new("v1/organizations/compliance_settings"), &[]).await
    }
}
