//! Per-user activity.
//!
//! - `GET /v1/organizations/analytics/users` → [`UserActivity`]
//!
//! Per-product metric blocks are always present; a product the organization does not use reports
//! zeros, not `null`. In date-range mode counters are summed across days and distinct counts are
//! recomputed exactly, approximated (HLL, typically under 2% error) or `null`, as each field says.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::AnalyticsUser;

/// One user's activity, or one RBAC group's when grouped by `rbac_group_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserActivity {
    /// claude.ai chat metrics.
    pub chat_metrics: ChatMetrics,
    /// Claude Code metrics.
    pub claude_code_metrics: ClaudeCodeMetrics,
    /// Cowork metrics.
    pub cowork_metrics: CoworkMetrics,
    /// Claude Design metrics.
    pub design_metrics: DesignMetrics,
    /// Claude in Office metrics per Office product.
    pub office_metrics: OfficeMetrics,
    /// Claude Science metrics.
    pub science_metrics: ScienceMetrics,
    /// Web searches performed.
    pub web_search_count: u64,
    /// Distinct active users in a grouped row; `null` on per-user rows.
    #[serde(default)]
    pub distinct_user_count: Option<u64>,
    /// Latest UTC day with counted activity inside the requested window. Omitted while
    /// last-activity reporting is not enabled for the organization.
    #[serde(default)]
    pub last_activity_date: Option<Date>,
    /// RBAC group (`rbac_group_…`), when grouped by `rbac_group_id`.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// RBAC group display name; `null` if deleted or unresolved.
    #[serde(default)]
    pub rbac_group_name: Option<String>,
    /// The user, on per-user rows.
    #[serde(default)]
    pub user: Option<AnalyticsUser>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// claude.ai chat metrics for one user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMetrics {
    /// MCP connector invocations.
    pub connectors_used_count: u64,
    /// Distinct artifacts created (exact in date-range mode).
    pub distinct_artifacts_created_count: u64,
    /// Distinct claude.ai connectors used.
    #[serde(default)]
    pub distinct_connectors_used_count: Option<u64>,
    /// Distinct conversations participated in.
    #[serde(default)]
    pub distinct_conversation_count: Option<u64>,
    /// Distinct files uploaded.
    #[serde(default)]
    pub distinct_files_uploaded_count: Option<u64>,
    /// Distinct projects created (exact in date-range mode).
    pub distinct_projects_created_count: u64,
    /// Distinct projects used.
    #[serde(default)]
    pub distinct_projects_used_count: Option<u64>,
    /// Distinct shared artifacts viewed.
    #[serde(default)]
    pub distinct_shared_artifacts_viewed_count: Option<u64>,
    /// Distinct skills used.
    #[serde(default)]
    pub distinct_skills_used_count: Option<u64>,
    /// Messages sent.
    pub message_count: u64,
    /// Times a shared conversation in a project was opened.
    pub shared_conversations_viewed_count: u64,
    /// Messages that used extended thinking.
    pub thinking_message_count: u64,
}

/// Claude Code metrics for one user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeMetrics {
    /// Core activity.
    pub core_metrics: ClaudeCodeCoreMetrics,
    /// Accept/reject counts for file-modification tools.
    pub tool_actions: ToolActions,
}

/// Core Claude Code activity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeCoreMetrics {
    /// Artifacts created in sessions; counted from 2026-08-17, 0 before.
    pub artifacts_created_count: u64,
    /// Commits made via Claude Code.
    pub commit_count: u64,
    /// Distinct sessions.
    #[serde(default)]
    pub distinct_session_count: Option<u64>,
    /// Lines added and removed.
    pub lines_of_code: LinesOfCode,
    /// Pull requests created via Claude Code.
    pub pull_request_count: u64,
}

/// Lines of code added and removed via Claude Code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinesOfCode {
    /// Lines added.
    pub added_count: u64,
    /// Lines removed.
    pub removed_count: u64,
}

/// Accept/reject counts per Claude Code file-modification tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActions {
    /// Edit tool.
    pub edit_tool: ToolActionCounts,
    /// MultiEdit tool.
    pub multi_edit_tool: ToolActionCounts,
    /// NotebookEdit tool.
    pub notebook_edit_tool: ToolActionCounts,
    /// Write tool.
    pub write_tool: ToolActionCounts,
}

/// Accepted and rejected proposals for one tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActionCounts {
    /// Proposals accepted.
    pub accepted_count: u64,
    /// Proposals rejected.
    pub rejected_count: u64,
}

/// Cowork metrics for one user.
///
/// The plugin and file-edit fields are `null` while those metrics are not enabled for the
/// organization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoworkMetrics {
    /// Tool actions completed.
    pub action_count: u64,
    /// Artifacts created; counted from 2026-08-17, 0 before.
    pub artifacts_created_count: u64,
    /// Connector invocations.
    pub connectors_used_count: u64,
    /// Dispatch (background agent) turns completed.
    pub dispatch_turn_count: u64,
    /// Distinct connectors used.
    #[serde(default)]
    pub distinct_connectors_used_count: Option<u64>,
    /// Distinct sessions.
    #[serde(default)]
    pub distinct_session_count: Option<u64>,
    /// Distinct skills used.
    #[serde(default)]
    pub distinct_skills_used_count: Option<u64>,
    /// Messages sent.
    pub message_count: u64,
    /// Skill invocations.
    pub skills_used_count: u64,
    /// Distinct plugins used.
    #[serde(default)]
    pub distinct_plugins_used_count: Option<u64>,
    /// Successful Edit tool calls.
    #[serde(default)]
    pub edit_tool_count: Option<u64>,
    /// Successful file-edit tool calls (Edit, MultiEdit, Write, NotebookEdit).
    #[serde(default)]
    pub file_edit_count: Option<u64>,
    /// Successful MultiEdit tool calls.
    #[serde(default)]
    pub multi_edit_tool_count: Option<u64>,
    /// Successful NotebookEdit tool calls.
    #[serde(default)]
    pub notebook_edit_tool_count: Option<u64>,
    /// Plugin invocations.
    #[serde(default)]
    pub plugins_used_count: Option<u64>,
    /// Distinct sessions with at least one successful file edit.
    #[serde(default)]
    pub sessions_with_file_edits_count: Option<u64>,
    /// Successful Write tool calls.
    #[serde(default)]
    pub write_tool_count: Option<u64>,
}

/// Claude Design metrics for one user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignMetrics {
    /// Distinct projects created (exact in date-range mode).
    pub distinct_projects_created_count: u64,
    /// Distinct projects worked in.
    #[serde(default)]
    pub distinct_projects_used_count: Option<u64>,
    /// Distinct sessions.
    #[serde(default)]
    pub distinct_session_count: Option<u64>,
    /// Messages sent.
    pub message_count: u64,
}

/// Claude in Office metrics per Office product.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficeMetrics {
    /// Excel.
    pub excel: OfficeProductMetrics,
    /// Outlook.
    pub outlook: OfficeProductMetrics,
    /// PowerPoint.
    pub powerpoint: OfficeProductMetrics,
    /// Word.
    pub word: OfficeProductMetrics,
}

/// Claude in Office metrics within one Office product.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficeProductMetrics {
    /// MCP connector invocations.
    pub connectors_used_count: u64,
    /// Distinct MCP connectors used.
    #[serde(default)]
    pub distinct_connectors_used_count: Option<u64>,
    /// Distinct sessions.
    #[serde(default)]
    pub distinct_session_count: Option<u64>,
    /// Distinct skills used.
    #[serde(default)]
    pub distinct_skills_used_count: Option<u64>,
    /// Messages sent.
    pub message_count: u64,
    /// Skill invocations.
    pub skills_used_count: u64,
}

/// Claude Science metrics for one user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScienceMetrics {
    /// Delegations to a specialized agent.
    pub delegation_count: u64,
    /// Distinct sessions.
    #[serde(default)]
    pub distinct_session_count: Option<u64>,
    /// Messages sent.
    pub message_count: u64,
    /// Remote compute jobs launched.
    pub remote_compute_job_count: u64,
    /// Skill invocations.
    pub skills_used_count: u64,
}
