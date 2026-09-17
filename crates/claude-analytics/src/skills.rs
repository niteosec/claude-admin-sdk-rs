//! Per-skill usage.
//!
//! - `GET /v1/organizations/analytics/skills` → [`SkillUsage`]
//!
//! A skill counts as used only when explicitly activated (invoked by the model or through its slash
//! command); installed or merely available skills have no usage.
//!
//! *Built from Anthropic's API reference (fetched 2026-09-17); not yet verified against a live tenant.*

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::common::Currency;

claude_api_core::string_enum! {
    /// A claude.ai skill's share status.
    pub enum ShareStatus {
        /// `organization`
        Organization = "organization",
        /// `private`
        Private = "private",
        /// `public`
        Public = "public",
    }
}

/// Usage of one skill, optionally per product, user or RBAC group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUsage {
    /// claude.ai chat metrics.
    pub chat_metrics: SkillChatMetrics,
    /// Claude Code metrics.
    pub claude_code_metrics: SkillSessionMetrics,
    /// Cowork metrics.
    pub cowork_metrics: SkillSessionMetrics,
    /// Distinct users who used the skill.
    pub distinct_user_count: u64,
    /// Claude in Office metrics per Office product.
    pub office_metrics: SkillOfficeMetrics,
    /// Skill name; an opaque ID for user, organization and plugin-delivered skills (see
    /// `skill_display_name`).
    pub skill_name: String,
    /// List-price value of attributed member requests, a decimal string in the minor unit of
    /// `currency`. Undiscounted; does not tie to billed spend.
    #[serde(default)]
    pub attributed_list_price: Option<String>,
    /// Currency of the monetary fields; `null` when both are `null`.
    #[serde(default)]
    pub currency: Option<Currency>,
    /// Distinct accounts that enabled the skill (claude.ai only). `null` on scoped rows and in
    /// date-range mode.
    #[serde(default)]
    pub enable_count: Option<u64>,
    /// Estimated overage spend attributed to the skill, a decimal string in the minor unit of
    /// `currency`. An estimate, not a billing number.
    #[serde(default)]
    pub estimated_overage_spend: Option<String>,
    /// Times the skill was invoked.
    #[serde(default)]
    pub invocation_count: Option<u64>,
    /// Product surface, when grouped by `product`.
    #[serde(default)]
    pub product: Option<String>,
    /// RBAC group (`rbac_group_…`), when grouped by `rbac_group_id`.
    #[serde(default)]
    pub rbac_group_id: Option<String>,
    /// RBAC group display name; `null` if deleted or unresolved.
    #[serde(default)]
    pub rbac_group_name: Option<String>,
    /// Share status (claude.ai only).
    #[serde(default)]
    pub share_status: Option<ShareStatus>,
    /// Display name for rows whose `skill_name` is an opaque ID, when it may be disclosed.
    #[serde(default)]
    pub skill_display_name: Option<String>,
    /// Tagged user ID, when grouped by `user_id`.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// claude.ai chat metrics for one skill.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillChatMetrics {
    /// Distinct conversations in which the skill was used.
    #[serde(default)]
    pub distinct_conversation_skill_used_count: Option<u64>,
}

/// Session metrics for one skill on one surface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSessionMetrics {
    /// Distinct sessions in which the skill was used.
    #[serde(default)]
    pub distinct_session_skill_used_count: Option<u64>,
}

/// Claude in Office metrics for one skill, per Office product.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillOfficeMetrics {
    /// Excel.
    pub excel: SkillSessionMetrics,
    /// Outlook.
    pub outlook: SkillSessionMetrics,
    /// PowerPoint.
    pub powerpoint: SkillSessionMetrics,
    /// Word.
    pub word: SkillSessionMetrics,
}
