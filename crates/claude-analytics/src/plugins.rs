//! Per-plugin usage across Cowork and Claude Code.
//!
//! - `GET /v1/organizations/analytics/plugins` → [`PluginUsage`]
//!
//! The `plugin_name` value `third-party` is an aggregate bucket for activity whose client did not
//! report a plugin name, not a plugin.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Install and invocation usage of one plugin, optionally per product, user or RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginUsage {
    /// Claude Code metrics.
    pub claude_code_metrics: PluginSessionMetrics,
    /// Cowork metrics.
    pub cowork_metrics: PluginSessionMetrics,
    /// Distinct users with install or invocation activity.
    pub distinct_user_count: u64,
    /// Distinct users who installed the plugin.
    #[serde(default)]
    pub install_count: Option<u64>,
    /// Plugin invocations.
    pub invocation_count: u64,
    /// Plugin name, or the `third-party` aggregate bucket.
    pub plugin_name: String,
    /// Stable plugin ID when available, for example `serena@claude-plugins-official`.
    #[serde(default)]
    pub plugin_id: Option<String>,
    /// Product surface (`claude_code` or `cowork`), when grouped by `product`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`), when grouped by `rbac_group_id`.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// RBAC group display name; `null` if deleted or unresolved.
    #[serde(default)]
    pub rbac_group_name: Option<String>,
    /// Tagged user ID, when grouped by `user_id`.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Session metrics for one plugin on one surface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSessionMetrics {
    /// Distinct sessions in which the plugin was invoked.
    #[serde(default)]
    pub distinct_session_plugin_used_count: Option<u64>,
}
