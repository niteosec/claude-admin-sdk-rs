//! Per-connector usage.
//!
//! - `GET /v1/organizations/analytics/connectors` → [`ConnectorUsage`]
//!
//! Connector names are normalized across sources: `Atlassian MCP server`, `mcp-atlassian` and
//! `atlassian_MCP` all appear as `atlassian`.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Usage of one connector, optionally per product, user or RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorUsage {
    /// claude.ai chat metrics.
    pub chat_metrics: ConnectorChatMetrics,
    /// Claude Code metrics.
    pub claude_code_metrics: ConnectorSessionMetrics,
    /// Normalized connector name, or an opaque connector ID (see `connector_display_name`). The
    /// row's stable key.
    pub connector_name: String,
    /// Cowork metrics.
    pub cowork_metrics: ConnectorSessionMetrics,
    /// Distinct users who used the connector.
    pub distinct_user_count: u64,
    /// Claude in Office metrics per Office product.
    pub office_metrics: ConnectorOfficeMetrics,
    /// Display name for rows whose `connector_name` is an opaque ID. Not unique.
    #[serde(default)]
    pub connector_display_name: Option<String>,
    /// Distinct users whose use ran on their own individual credential.
    #[serde(default)]
    pub individual_auth_distinct_user_count: Option<u64>,
    /// Distinct users whose use ran on Enterprise Managed Auth. `null`, never 0, when not
    /// reported (not enabled, unattributable, no observed auth activity, or before 2026-07-01).
    #[serde(default)]
    pub managed_auth_distinct_user_count: Option<u64>,
    /// Product surface, when grouped by `product`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`), when grouped by `rbac_group_id`.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// RBAC group display name; `null` if deleted or unresolved.
    #[serde(default)]
    pub rbac_group_name: Option<String>,
    /// Tool calls annotated read-only. `null`, never 0, when the read/write split is not enabled
    /// or the day predates 2026-05-29.
    #[serde(default)]
    pub read_call_count: Option<u64>,
    /// Tool calls with no trusted read-only annotation.
    #[serde(default)]
    pub unclassified_call_count: Option<u64>,
    /// Tagged user ID, when grouped by `user_id`.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Tool calls annotated not read-only.
    #[serde(default)]
    pub write_call_count: Option<u64>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// claude.ai chat metrics for one connector.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorChatMetrics {
    /// Distinct conversations in which the connector was used.
    #[serde(default)]
    pub distinct_conversation_connector_used_count: Option<u64>,
}

/// Session metrics for one connector on one surface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorSessionMetrics {
    /// Distinct sessions in which the connector was used.
    #[serde(default)]
    pub distinct_session_connector_used_count: Option<u64>,
}

/// Claude in Office metrics for one connector, per Office product.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorOfficeMetrics {
    /// Excel.
    pub excel: ConnectorSessionMetrics,
    /// Outlook.
    pub outlook: ConnectorSessionMetrics,
    /// PowerPoint.
    pub powerpoint: ConnectorSessionMetrics,
    /// Word.
    pub word: ConnectorSessionMetrics,
}
