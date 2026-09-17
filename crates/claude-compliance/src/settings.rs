//! Effective organization settings.
//!
//! - `GET /v1/compliance/organizations/{organization_id}/settings`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use std::collections::BTreeMap;

use claude_api_core::{ApiPath, ApiResponse, Result};
use jiff::Timestamp;
use serde::de::{DeserializeOwned, Error as _};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::ComplianceClient;

impl ComplianceClient {
    /// `GET /v1/compliance/organizations/{organization_id}/settings`: the settings in force for an
    /// organization after all policies are applied. Requires `read:compliance_org_data`.
    ///
    /// Settings the organization's administrators cannot change are omitted. Organizations outside
    /// the key's hierarchy return 404.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/compliance/organizations/settings/retrieve).
    pub async fn effective_settings(
        &self,
        organization_uuid: &str,
    ) -> Result<ApiResponse<EffectiveOrganizationSettings>> {
        let path = ApiPath::new("v1/compliance/organizations").id(organization_uuid)?.then("settings");
        self.api.get_json(&path, &[]).await
    }
}

/// The effective settings of one organization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveOrganizationSettings {
    /// `effective_organization_settings` when present.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    /// Compliance API keys configured for the organization hierarchy, oldest first. Secrets are
    /// never included.
    pub api_keys: Vec<ComplianceApiKey>,
    /// The organization.
    pub organization_id: String,
    /// The settings in force.
    pub settings: Vec<Setting>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A Compliance API key, without its secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplianceApiKey {
    /// `compliance_api_key` when present.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    /// Key identifier.
    pub id: String,
    /// When the key was created.
    pub created_at: Timestamp,
    /// The creating user, or `null` when created by automation or the creator no longer exists.
    #[serde(default)]
    pub created_by_id: Option<String>,
    /// Whether the key can authenticate. Deactivated keys are listed for audit visibility.
    pub is_active: bool,
    /// The name given at creation.
    pub name: String,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// When the key stops authenticating, or `null` when it does not expire.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One effective setting, discriminated by `type`.
///
/// Unknown types are kept whole in [`Setting::Other`]. The reference marks `type` optional; a row
/// without it is also kept in `Other`, with an empty `kind`.
#[derive(Debug, Clone, PartialEq)]
pub enum Setting {
    /// `boolean`: a true/false flag.
    Boolean(BooleanSetting),
    /// `integer`: a whole number; `null` means no limit.
    Integer(IntegerSetting),
    /// `string`: a single string; `null` means no value.
    String(StringSetting),
    /// `string_list`: a list of strings.
    StringList(StringListSetting),
    /// `provisioning_mode`: how members are provisioned.
    ProvisioningMode(ProvisioningModeSetting),
    /// `data_retention`: retention periods by data type.
    DataRetention(DataRetentionSetting),
    /// Any other `type`, with the raw object.
    Other {
        /// The `type` value, empty when absent.
        kind: String,
        /// The whole setting object.
        raw: Value,
    },
}

impl Setting {
    /// The wire `type`.
    pub fn setting_type(&self) -> &str {
        match self {
            Setting::Boolean(_) => "boolean",
            Setting::Integer(_) => "integer",
            Setting::String(_) => "string",
            Setting::StringList(_) => "string_list",
            Setting::ProvisioningMode(_) => "provisioning_mode",
            Setting::DataRetention(_) => "data_retention",
            Setting::Other { kind, .. } => kind,
        }
    }
}

claude_api_core::string_enum! {
    /// Names of `boolean` settings.
    pub enum BooleanSettingName {
        /// `ai_powered_artifacts_enabled`
        AiPoweredArtifactsEnabled = "ai_powered_artifacts_enabled",
        /// `api_workbench_feedback_collection_enabled`
        ApiWorkbenchFeedbackCollectionEnabled = "api_workbench_feedback_collection_enabled",
        /// `artifact_connectors_enabled`
        ArtifactConnectorsEnabled = "artifact_connectors_enabled",
        /// `ask_your_org_enabled`
        AskYourOrgEnabled = "ask_your_org_enabled",
        /// `chat_enabled`
        ChatEnabled = "chat_enabled",
        /// `claude_ai_chat_sharing_enabled`
        ClaudeAiChatSharingEnabled = "claude_ai_chat_sharing_enabled",
        /// `claude_ai_feedback_collection_enabled`
        ClaudeAiFeedbackCollectionEnabled = "claude_ai_feedback_collection_enabled",
        /// `claude_ai_integration_sharing_enabled`
        ClaudeAiIntegrationSharingEnabled = "claude_ai_integration_sharing_enabled",
        /// `claude_ai_skill_plugins_scanning_enabled`
        ClaudeAiSkillPluginsScanningEnabled = "claude_ai_skill_plugins_scanning_enabled",
        /// `claude_code_desktop_bypass_permissions_enabled`
        ClaudeCodeDesktopBypassPermissionsEnabled = "claude_code_desktop_bypass_permissions_enabled",
        /// `claude_code_desktop_enabled`
        ClaudeCodeDesktopEnabled = "claude_code_desktop_enabled",
        /// `claude_code_fast_mode_enabled`
        ClaudeCodeFastModeEnabled = "claude_code_fast_mode_enabled",
        /// `claude_code_metrics_logging_enabled`
        ClaudeCodeMetricsLoggingEnabled = "claude_code_metrics_logging_enabled",
        /// `claude_code_remote_control_enabled`
        ClaudeCodeRemoteControlEnabled = "claude_code_remote_control_enabled",
        /// `claude_code_review_enabled`
        ClaudeCodeReviewEnabled = "claude_code_review_enabled",
        /// `claude_code_routines_enabled`
        ClaudeCodeRoutinesEnabled = "claude_code_routines_enabled",
        /// `claude_code_security_enabled`
        ClaudeCodeSecurityEnabled = "claude_code_security_enabled",
        /// `claude_code_trusted_devices_required`
        ClaudeCodeTrustedDevicesRequired = "claude_code_trusted_devices_required",
        /// `claude_code_web_enabled`
        ClaudeCodeWebEnabled = "claude_code_web_enabled",
        /// `claude_code_workflows_enabled`
        ClaudeCodeWorkflowsEnabled = "claude_code_workflows_enabled",
        /// `claude_design_enabled`
        ClaudeDesignEnabled = "claude_design_enabled",
        /// `claude_in_slack_enabled`
        ClaudeInSlackEnabled = "claude_in_slack_enabled",
        /// `claude_science_custom_connectors_enabled`
        ClaudeScienceCustomConnectorsEnabled = "claude_science_custom_connectors_enabled",
        /// `claude_science_custom_skills_enabled`
        ClaudeScienceCustomSkillsEnabled = "claude_science_custom_skills_enabled",
        /// `claude_science_enabled`
        ClaudeScienceEnabled = "claude_science_enabled",
        /// `claude_science_managed_network_allowlist_enabled`
        ClaudeScienceManagedNetworkAllowlistEnabled = "claude_science_managed_network_allowlist_enabled",
        /// `claude_science_memory_enabled`
        ClaudeScienceMemoryEnabled = "claude_science_memory_enabled",
        /// `claude_science_modal_enabled`
        ClaudeScienceModalEnabled = "claude_science_modal_enabled",
        /// `claude_science_scientific_model_endpoints_enabled`
        ClaudeScienceScientificModelEndpointsEnabled = "claude_science_scientific_model_endpoints_enabled",
        /// `claude_science_ssh_hosts_enabled`
        ClaudeScienceSshHostsEnabled = "claude_science_ssh_hosts_enabled",
        /// `code_execution_enabled`
        CodeExecutionEnabled = "code_execution_enabled",
        /// `code_execution_network_egress_enabled`
        CodeExecutionNetworkEgressEnabled = "code_execution_network_egress_enabled",
        /// `connector_tools_default_always_allow`
        ConnectorToolsDefaultAlwaysAllow = "connector_tools_default_always_allow",
        /// `content_redaction_enabled`
        ContentRedactionEnabled = "content_redaction_enabled",
        /// `cowork_trusted_devices_required`
        CoworkTrustedDevicesRequired = "cowork_trusted_devices_required",
        /// `desktop_extension_allowlist_enabled`
        DesktopExtensionAllowlistEnabled = "desktop_extension_allowlist_enabled",
        /// `directory_sync_enabled`
        DirectorySyncEnabled = "directory_sync_enabled",
        /// `frontier_data_use_enabled`
        FrontierDataUseEnabled = "frontier_data_use_enabled",
        /// `group_skill_sharing_enabled`
        GroupSkillSharingEnabled = "group_skill_sharing_enabled",
        /// `hipaa_compliance_enabled`
        HipaaComplianceEnabled = "hipaa_compliance_enabled",
        /// `inline_visualizations_enabled`
        InlineVisualizationsEnabled = "inline_visualizations_enabled",
        /// `ip_allowlist_enabled`
        IpAllowlistEnabled = "ip_allowlist_enabled",
        /// `location_metadata_enabled`
        LocationMetadataEnabled = "location_metadata_enabled",
        /// `member_usage_dashboard_visible`
        MemberUsageDashboardVisible = "member_usage_dashboard_visible",
        /// `memory_enabled`
        MemoryEnabled = "memory_enabled",
        /// `org_wide_skill_sharing_enabled`
        OrgWideSkillSharingEnabled = "org_wide_skill_sharing_enabled",
        /// `public_projects_enabled`
        PublicProjectsEnabled = "public_projects_enabled",
        /// `skill_sharing_enabled`
        SkillSharingEnabled = "skill_sharing_enabled",
        /// `skills_enabled`
        SkillsEnabled = "skills_enabled",
        /// `sso_claude_ai_enforced`
        SsoClaudeAiEnforced = "sso_claude_ai_enforced",
        /// `sso_console_enforced`
        SsoConsoleEnforced = "sso_console_enforced",
        /// `sso_enabled`
        SsoEnabled = "sso_enabled",
        /// `third_party_interactive_content_enabled`
        ThirdPartyInteractiveContentEnabled = "third_party_interactive_content_enabled",
        /// `user_skill_creation_enabled`
        UserSkillCreationEnabled = "user_skill_creation_enabled",
        /// `web_search_enabled`
        WebSearchEnabled = "web_search_enabled",
        /// `work_across_apps_enabled`
        WorkAcrossAppsEnabled = "work_across_apps_enabled",
    }
}

/// A `boolean` setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BooleanSetting {
    /// Setting name.
    pub name: BooleanSettingName,
    /// Enforced value.
    pub value: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Names of `integer` settings.
    pub enum IntegerSettingName {
        /// `account_session_duration_seconds`
        AccountSessionDurationSeconds = "account_session_duration_seconds",
    }
}

/// An `integer` setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegerSetting {
    /// Setting name.
    pub name: IntegerSettingName,
    /// Enforced value; `None` means no limit is in force.
    #[serde(default)]
    pub value: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Names of `string` settings.
    pub enum StringSettingName {
        /// `claude_code_default_worker_environment_id`
        ClaudeCodeDefaultWorkerEnvironmentId = "claude_code_default_worker_environment_id",
        /// `claude_code_default_worker_pool_id`
        ClaudeCodeDefaultWorkerPoolId = "claude_code_default_worker_pool_id",
    }
}

/// A `string` setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringSetting {
    /// Setting name.
    pub name: StringSettingName,
    /// Enforced value; `None` means no value is configured.
    #[serde(default)]
    pub value: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// Names of `string_list` settings.
    pub enum StringListSettingName {
        /// `allowed_invite_domains`
        AllowedInviteDomains = "allowed_invite_domains",
        /// `disabled_admin_request_types`
        DisabledAdminRequestTypes = "disabled_admin_request_types",
        /// `ip_allowlist_ip_ranges`
        IpAllowlistIpRanges = "ip_allowlist_ip_ranges",
    }
}

/// A `string_list` setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringListSetting {
    /// Setting name.
    pub name: StringListSettingName,
    /// Enforced value.
    pub value: Vec<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// How organization members are provisioned under SSO.
    pub enum ProvisioningMode {
        /// `jit_advanced`
        JitAdvanced = "jit_advanced",
        /// `jit_permissive`
        JitPermissive = "jit_permissive",
        /// `login_only`
        LoginOnly = "login_only",
        /// `scim_advanced`
        ScimAdvanced = "scim_advanced",
        /// `scim_permissive`
        ScimPermissive = "scim_permissive",
    }
}

/// The `provisioning_mode` setting.
///
/// A just-in-time mode is reported only while SSO is enabled and a SCIM mode only while directory
/// sync is enabled; otherwise the value is `login_only`, whatever is configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningModeSetting {
    /// Enforced mode.
    pub value: ProvisioningMode,
    /// `sso_provisioning_mode` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The `data_retention` setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRetentionSetting {
    /// Retention periods keyed by data type. `all` covers every type and is then the only key. A
    /// missing key means no administrator-configured period applies to that type (Anthropic's
    /// service defaults may).
    pub value: BTreeMap<String, RetentionPeriod>,
    /// `data_retention_periods` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A retention period, discriminated by `type`.
///
/// Unknown types, and periods without `type`, are kept whole in [`RetentionPeriod::Other`].
#[derive(Debug, Clone, PartialEq)]
pub enum RetentionPeriod {
    /// `fixed`: a window measured from each item's last activity.
    Fixed(FixedRetention),
    /// `indefinite`: kept with no time limit.
    Indefinite(IndefiniteRetention),
    /// Any other `type`, with the raw object.
    Other {
        /// The `type` value, empty when absent.
        kind: String,
        /// The whole period object.
        raw: Value,
    },
}

impl RetentionPeriod {
    /// The wire `type`.
    pub fn period_type(&self) -> &str {
        match self {
            RetentionPeriod::Fixed(_) => "fixed",
            RetentionPeriod::Indefinite(_) => "indefinite",
            RetentionPeriod::Other { kind, .. } => kind,
        }
    }
}

claude_api_core::string_enum! {
    /// Unit of a fixed retention window.
    pub enum RetentionTimescale {
        /// `day`
        Day = "day",
        /// `month`
        Month = "month",
    }
}

/// A `fixed` retention period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedRetention {
    /// Length of the window, in `timescale` units.
    pub duration: u64,
    /// Unit of `duration`.
    pub timescale: RetentionTimescale,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An `indefinite` retention period.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndefiniteRetention {
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Splits `type` off an object. `None` when the value is not an object.
fn untag(raw: &Value) -> Option<(String, Value)> {
    let mut object = raw.as_object()?.clone();
    let kind = match object.remove("type") {
        Some(Value::String(kind)) => kind,
        _ => String::new(),
    };
    Some((kind, Value::Object(object)))
}

fn decode<T: DeserializeOwned, E: serde::de::Error>(body: Value) -> Result<T, E> {
    serde_json::from_value(body).map_err(E::custom)
}

fn tagged<T: Serialize>(kind: &str, inner: &T) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(inner)?;
    if let Value::Object(map) = &mut value {
        map.insert("type".to_owned(), Value::String(kind.to_owned()));
    }
    Ok(value)
}

impl<'de> Deserialize<'de> for Setting {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let (kind, body) = untag(&raw).ok_or_else(|| D::Error::custom("setting is not an object"))?;
        Ok(match kind.as_str() {
            "boolean" => Setting::Boolean(decode(body)?),
            "integer" => Setting::Integer(decode(body)?),
            "string" => Setting::String(decode(body)?),
            "string_list" => Setting::StringList(decode(body)?),
            "provisioning_mode" => Setting::ProvisioningMode(decode(body)?),
            "data_retention" => Setting::DataRetention(decode(body)?),
            _ => Setting::Other { kind, raw },
        })
    }
}

impl Serialize for Setting {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = self.setting_type();
        let value = match self {
            Setting::Boolean(inner) => tagged(kind, inner),
            Setting::Integer(inner) => tagged(kind, inner),
            Setting::String(inner) => tagged(kind, inner),
            Setting::StringList(inner) => tagged(kind, inner),
            Setting::ProvisioningMode(inner) => tagged(kind, inner),
            Setting::DataRetention(inner) => tagged(kind, inner),
            Setting::Other { raw, .. } => Ok(raw.clone()),
        }
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RetentionPeriod {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let (kind, body) = untag(&raw).ok_or_else(|| D::Error::custom("retention period is not an object"))?;
        Ok(match kind.as_str() {
            "fixed" => RetentionPeriod::Fixed(decode(body)?),
            "indefinite" => RetentionPeriod::Indefinite(decode(body)?),
            _ => RetentionPeriod::Other { kind, raw },
        })
    }
}

impl Serialize for RetentionPeriod {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let kind = self.period_type();
        let value = match self {
            RetentionPeriod::Fixed(inner) => tagged(kind, inner),
            RetentionPeriod::Indefinite(inner) => tagged(kind, inner),
            RetentionPeriod::Other { raw, .. } => Ok(raw.clone()),
        }
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn known_setting_round_trips_with_its_tag() {
        let raw = json!({"type": "boolean", "name": "memory_enabled", "value": true});
        let setting: Setting = serde_json::from_value(raw.clone()).unwrap();
        assert!(
            matches!(&setting, Setting::Boolean(b) if b.name == BooleanSettingName::MemoryEnabled && b.extra.is_empty())
        );
        assert_eq!(serde_json::to_value(&setting).unwrap(), raw);
    }

    #[test]
    fn untyped_setting_is_kept_whole() {
        let raw = json!({"name": "memory_enabled", "value": true});
        let setting: Setting = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(setting, Setting::Other { kind: String::new(), raw: raw.clone() });
        assert_eq!(serde_json::to_value(&setting).unwrap(), raw);
    }

    #[test]
    fn non_object_setting_is_an_error() {
        assert!(serde_json::from_value::<Setting>(json!("boolean")).is_err());
    }
}
