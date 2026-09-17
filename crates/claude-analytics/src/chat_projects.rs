//! Per-project chat activity.
//!
//! - `GET /v1/organizations/analytics/apps/chat/projects` → [`ChatProjectUsage`]
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::AnalyticsUser;

/// Activity in one claude.ai project, optionally per user or RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatProjectUsage {
    /// Distinct users who used the project.
    pub distinct_user_count: u64,
    /// Messages sent in the project.
    pub message_count: u64,
    /// Tagged project ID (`claude_proj_…`).
    pub project_id: String,
    /// Project name.
    pub project_name: String,
    /// When the project was created; `null` if it was deleted before attribution was recorded.
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    /// Who created the project.
    #[serde(default)]
    pub created_by: Option<AnalyticsUser>,
    /// Distinct conversations in the project.
    #[serde(default)]
    pub distinct_conversation_count: Option<u64>,
    /// Product surface. Documented as a shared field; this endpoint rejects the `product`
    /// dimension.
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
