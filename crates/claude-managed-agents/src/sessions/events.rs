//! Session events: the union returned by the session and thread event lists.

use std::collections::BTreeMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::agent::{SessionAgent, SessionModelSpeed};
use super::content::{SessionContentBlock, SessionRubric, SessionTextContent};
use super::{SessionBudget, SessionOutcomeResult, SessionUsage};
use crate::runtime::{present, tagged_union};

tagged_union! {
    /// One session event, discriminated by `type` (for example `user.message`).
    ///
    /// Events carry conversation content, tool inputs and tool results; treat them as sensitive.
    /// Unknown event types are kept whole in [`SessionEvent::Other`].
    #[allow(clippy::large_enum_variant, reason = "decoded records, rarely moved")]
    pub enum SessionEvent {
        /// `user.message`: a user message.
        UserMessage(SessionUserMessageEvent) = "user.message",
        /// `user.interrupt`: an interrupt that pauses the agent.
        UserInterrupt(SessionUserInterruptEvent) = "user.interrupt",
        /// `user.tool_confirmation`: approval or denial of a pending tool call.
        UserToolConfirmation(SessionUserToolConfirmationEvent) = "user.tool_confirmation",
        /// `user.custom_tool_result`: the client's result for a custom tool call.
        UserCustomToolResult(SessionUserCustomToolResultEvent) = "user.custom_tool_result",
        /// `user.tool_result`: the client's result for an agent-toolset call (self-hosted
        /// environments only).
        UserToolResult(SessionUserToolResultEvent) = "user.tool_result",
        /// `user.define_outcome`: echo of an outcome definition, with its server-generated ID.
        UserDefineOutcome(SessionDefineOutcomeEvent) = "user.define_outcome",
        /// `agent.custom_tool_use`: the agent called a custom tool.
        AgentCustomToolUse(SessionAgentCustomToolUseEvent) = "agent.custom_tool_use",
        /// `agent.message`: an agent response.
        AgentMessage(SessionAgentMessageEvent) = "agent.message",
        /// `agent.thinking`: a progress signal during extended thinking (no content).
        AgentThinking(SessionBasicEvent) = "agent.thinking",
        /// `agent.mcp_tool_use`: the agent called an MCP tool.
        AgentMcpToolUse(SessionAgentMcpToolUseEvent) = "agent.mcp_tool_use",
        /// `agent.mcp_tool_result`: the result of an MCP tool call.
        AgentMcpToolResult(SessionAgentMcpToolResultEvent) = "agent.mcp_tool_result",
        /// `agent.tool_use`: the agent called a built-in tool.
        AgentToolUse(SessionAgentToolUseEvent) = "agent.tool_use",
        /// `agent.tool_result`: the result of a built-in tool call.
        AgentToolResult(SessionAgentToolResultEvent) = "agent.tool_result",
        /// `agent.thread_message_received`: an agent-to-agent message arrived on this thread.
        AgentThreadMessageReceived(SessionThreadMessageReceivedEvent) = "agent.thread_message_received",
        /// `agent.thread_message_sent`: this thread sent an agent-to-agent message.
        AgentThreadMessageSent(SessionThreadMessageSentEvent) = "agent.thread_message_sent",
        /// `agent.thread_context_compacted`: context was summarized.
        AgentThreadContextCompacted(SessionBasicEvent) = "agent.thread_context_compacted",
        /// `session.error`: a problem during execution.
        SessionError(SessionErrorEvent) = "session.error",
        /// `session.status_rescheduled`: recovering from an error, rescheduled.
        SessionStatusRescheduled(SessionBasicEvent) = "session.status_rescheduled",
        /// `session.status_running`: the agent is working.
        SessionStatusRunning(SessionBasicEvent) = "session.status_running",
        /// `session.status_idle`: the agent is waiting, with the reason.
        SessionStatusIdle(SessionStatusIdleEvent) = "session.status_idle",
        /// `session.status_terminated`: the session ended.
        SessionStatusTerminated(SessionBasicEvent) = "session.status_terminated",
        /// `session.thread_created`: a subagent thread was spawned.
        SessionThreadCreated(SessionThreadLifecycleEvent) = "session.thread_created",
        /// `session.thread_status_running`: a thread started executing.
        SessionThreadStatusRunning(SessionThreadLifecycleEvent) = "session.thread_status_running",
        /// `session.thread_status_idle`: a thread yielded, with the reason.
        SessionThreadStatusIdle(SessionThreadStatusIdleEvent) = "session.thread_status_idle",
        /// `session.thread_status_terminated`: a thread ended.
        SessionThreadStatusTerminated(SessionThreadLifecycleEvent) = "session.thread_status_terminated",
        /// `session.thread_status_rescheduled`: a thread hit a transient error and is retrying.
        SessionThreadStatusRescheduled(SessionThreadLifecycleEvent) = "session.thread_status_rescheduled",
        /// `session.deleted`: the session was deleted; no further events follow.
        SessionDeleted(SessionBasicEvent) = "session.deleted",
        /// `session.updated`: an update changed at least one field; carries only the changes.
        SessionUpdated(SessionUpdatedEvent) = "session.updated",
        /// `session.usage`: a periodic snapshot of cumulative usage and list cost.
        SessionUsage(SessionUsageEvent) = "session.usage",
        /// `span.model_request_start`: a model request began.
        SpanModelRequestStart(SessionBasicEvent) = "span.model_request_start",
        /// `span.model_request_end`: a model request completed.
        SpanModelRequestEnd(SessionModelRequestEndEvent) = "span.model_request_end",
        /// `span.outcome_evaluation_start`: an outcome evaluation cycle began.
        SpanOutcomeEvaluationStart(SessionOutcomeEvaluationProgressEvent) = "span.outcome_evaluation_start",
        /// `span.outcome_evaluation_ongoing`: heartbeat while an evaluation runs.
        SpanOutcomeEvaluationOngoing(SessionOutcomeEvaluationProgressEvent) = "span.outcome_evaluation_ongoing",
        /// `span.outcome_evaluation_end`: an outcome evaluation cycle completed, with the verdict.
        SpanOutcomeEvaluationEnd(SessionOutcomeEvaluationEndEvent) = "span.outcome_evaluation_end",
        /// `system.message`: a mid-conversation system message.
        SystemMessage(SessionSystemMessageEvent) = "system.message",
    }
}

impl SessionEvent {
    /// The event ID, when this is a known event type (or an unknown one carrying a string `id`).
    pub fn id(&self) -> Option<&str> {
        let value = match self {
            Self::Other { raw, .. } => return raw.get("id").and_then(Value::as_str),
            Self::UserMessage(e) => &e.id,
            Self::UserInterrupt(e) => &e.id,
            Self::UserToolConfirmation(e) => &e.id,
            Self::UserCustomToolResult(e) => &e.id,
            Self::UserToolResult(e) => &e.id,
            Self::UserDefineOutcome(e) => &e.id,
            Self::AgentCustomToolUse(e) => &e.id,
            Self::AgentMessage(e) => &e.id,
            Self::AgentMcpToolUse(e) => &e.id,
            Self::AgentMcpToolResult(e) => &e.id,
            Self::AgentToolUse(e) => &e.id,
            Self::AgentToolResult(e) => &e.id,
            Self::AgentThreadMessageReceived(e) => &e.id,
            Self::AgentThreadMessageSent(e) => &e.id,
            Self::SessionError(e) => &e.id,
            Self::SessionStatusIdle(e) => &e.id,
            Self::SessionThreadStatusIdle(e) => &e.id,
            Self::SessionUpdated(e) => &e.id,
            Self::SessionUsage(e) => &e.id,
            Self::SpanModelRequestEnd(e) => &e.id,
            Self::SpanOutcomeEvaluationEnd(e) => &e.id,
            Self::SystemMessage(e) => &e.id,
            Self::AgentThinking(e)
            | Self::AgentThreadContextCompacted(e)
            | Self::SessionStatusRescheduled(e)
            | Self::SessionStatusRunning(e)
            | Self::SessionStatusTerminated(e)
            | Self::SessionDeleted(e)
            | Self::SpanModelRequestStart(e) => &e.id,
            Self::SessionThreadCreated(e)
            | Self::SessionThreadStatusRunning(e)
            | Self::SessionThreadStatusTerminated(e)
            | Self::SessionThreadStatusRescheduled(e) => &e.id,
            Self::SpanOutcomeEvaluationStart(e) | Self::SpanOutcomeEvaluationOngoing(e) => &e.id,
        };
        Some(value)
    }

    /// Fields of a known event type that this crate does not model (empty when everything was
    /// recognised); `None` for [`SessionEvent::Other`], whose whole object is in `raw`.
    pub fn extra(&self) -> Option<&Map<String, Value>> {
        let value = match self {
            Self::Other { .. } => return None,

            Self::UserMessage(e) => &e.extra,
            Self::UserInterrupt(e) => &e.extra,
            Self::UserToolConfirmation(e) => &e.extra,
            Self::UserCustomToolResult(e) => &e.extra,
            Self::UserToolResult(e) => &e.extra,
            Self::UserDefineOutcome(e) => &e.extra,
            Self::AgentCustomToolUse(e) => &e.extra,
            Self::AgentMessage(e) => &e.extra,
            Self::AgentMcpToolUse(e) => &e.extra,
            Self::AgentMcpToolResult(e) => &e.extra,
            Self::AgentToolUse(e) => &e.extra,
            Self::AgentToolResult(e) => &e.extra,
            Self::AgentThreadMessageReceived(e) => &e.extra,
            Self::AgentThreadMessageSent(e) => &e.extra,
            Self::SessionError(e) => &e.extra,
            Self::SessionStatusIdle(e) => &e.extra,
            Self::SessionThreadStatusIdle(e) => &e.extra,
            Self::SessionUpdated(e) => &e.extra,
            Self::SessionUsage(e) => &e.extra,
            Self::SpanModelRequestEnd(e) => &e.extra,
            Self::SpanOutcomeEvaluationEnd(e) => &e.extra,
            Self::SystemMessage(e) => &e.extra,
            Self::AgentThinking(e)
            | Self::AgentThreadContextCompacted(e)
            | Self::SessionStatusRescheduled(e)
            | Self::SessionStatusRunning(e)
            | Self::SessionStatusTerminated(e)
            | Self::SessionDeleted(e)
            | Self::SpanModelRequestStart(e) => &e.extra,
            Self::SessionThreadCreated(e)
            | Self::SessionThreadStatusRunning(e)
            | Self::SessionThreadStatusTerminated(e)
            | Self::SessionThreadStatusRescheduled(e) => &e.extra,
            Self::SpanOutcomeEvaluationStart(e) | Self::SpanOutcomeEvaluationOngoing(e) => &e.extra,
        };
        Some(value)
    }
}

/// An event carrying only `id` and `processed_at` (`agent.thinking`,
/// `agent.thread_context_compacted`, `session.status_rescheduled`, `session.status_running`,
/// `session.status_terminated`, `session.deleted`, `span.model_request_start`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBasicEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `user.message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserMessageEvent {
    /// Event ID.
    pub id: String,
    /// Text, image, document or redacted blocks.
    pub content: Vec<SessionContentBlock>,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `user.interrupt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserInterruptEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// The thread interrupted; absent means every non-archived thread (or the primary alone).
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// A user's decision on a pending tool call.
    pub enum SessionToolConfirmationResult {
        /// `allow`.
        Allow = "allow",
        /// `deny`.
        Deny = "deny",
    }
}

/// `user.tool_confirmation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserToolConfirmationEvent {
    /// Event ID.
    pub id: String,
    /// The decision.
    pub result: SessionToolConfirmationResult,
    /// ID of the `agent.tool_use` or `agent.mcp_tool_use` event decided on.
    pub tool_use_id: String,
    /// Context for a `deny`.
    #[serde(default)]
    pub deny_message: Option<String>,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// The subagent thread the confirmation was routed to; absent for the primary thread.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `user.custom_tool_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserCustomToolResultEvent {
    /// Event ID.
    pub id: String,
    /// ID of the `agent.custom_tool_use` event answered.
    pub custom_tool_use_id: String,
    /// Result content: text, image, document or search result blocks.
    #[serde(default)]
    pub content: Option<Vec<SessionContentBlock>>,
    /// Whether the tool failed.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// The subagent thread the result was routed to; absent for the primary thread.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `user.tool_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUserToolResultEvent {
    /// Event ID.
    pub id: String,
    /// ID of the `agent.tool_use` event answered.
    pub tool_use_id: String,
    /// Result content: text, image, document or search result blocks.
    #[serde(default)]
    pub content: Option<Vec<SessionContentBlock>>,
    /// Whether the tool failed.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// The subagent thread the result was routed to; absent for the primary thread.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `user.define_outcome`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDefineOutcomeEvent {
    /// Event ID.
    pub id: String,
    /// What the agent should produce.
    pub description: String,
    /// Evaluate-then-revise cycles before giving up (default 3, max 20).
    #[serde(default)]
    pub max_iterations: Option<u64>,
    /// Server-generated `outc_…` ID.
    pub outcome_id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The grading rubric.
    pub rubric: SessionRubric,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.custom_tool_use`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentCustomToolUseEvent {
    /// Event ID.
    pub id: String,
    /// Tool input.
    pub input: Map<String, Value>,
    /// Tool name.
    pub name: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Set when cross-posted from a subagent's thread to the primary stream.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentMessageEvent {
    /// Event ID.
    pub id: String,
    /// Text or redacted blocks.
    pub content: Vec<SessionContentBlock>,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// The permission a tool call evaluated to.
    pub enum SessionEvaluatedPermission {
        /// `allow`.
        Allow = "allow",
        /// `ask`.
        Ask = "ask",
        /// `deny`.
        Deny = "deny",
    }
}

tagged_union! {
    /// The resolved permission policy behind `evaluated_permission`, with the judgement under
    /// `auto`. Documented as an open union.
    #[derive(Eq)]
    pub enum SessionToolEvaluation {
        /// `always_allow`.
        AlwaysAllow = "always_allow",
        /// `always_ask`.
        AlwaysAsk = "always_ask",
        /// `auto`: the server judged this call.
        Auto(SessionAutoEvaluation) = "auto",
    }
}

/// The `auto` evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAutoEvaluation {
    /// The server's judgement.
    pub evaluated_permission: SessionAutoJudgement,
}

tagged_union! {
    /// The server's judgement of one call under the `auto` policy. Documented as an open union.
    #[derive(Eq)]
    pub enum SessionAutoJudgement {
        /// `allow`: judged safe.
        Allow = "allow",
        /// `ask`: no judgement reached; held for approval.
        Ask(SessionJudgementReason) = "ask",
        /// `deny`: judged high-risk; the call does not run.
        Deny(SessionJudgementReason) = "deny",
    }
}

/// The grounds of an `ask` or `deny` judgement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionJudgementReason {
    /// Open registry; documented values are `indeterminate` (ask) and `high_risk` (deny).
    pub reason_code: String,
}

/// `agent.mcp_tool_use`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentMcpToolUseEvent {
    /// Event ID.
    pub id: String,
    /// Tool input.
    pub input: Map<String, Value>,
    /// The MCP server providing the tool.
    pub mcp_server_name: String,
    /// Tool name.
    pub name: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The permission the call evaluated to.
    #[serde(default)]
    pub evaluated_permission: Option<SessionEvaluatedPermission>,
    /// The policy that produced `evaluated_permission`.
    #[serde(default)]
    pub evaluation: Option<SessionToolEvaluation>,
    /// Set when cross-posted from a subagent's thread to the primary stream.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.mcp_tool_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentMcpToolResultEvent {
    /// Event ID.
    pub id: String,
    /// ID of the `agent.mcp_tool_use` event.
    pub mcp_tool_use_id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Result content: text, image, document or search result blocks.
    #[serde(default)]
    pub content: Option<Vec<SessionContentBlock>>,
    /// Whether the tool failed.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.tool_use`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentToolUseEvent {
    /// Event ID.
    pub id: String,
    /// Tool input.
    pub input: Map<String, Value>,
    /// Tool name.
    pub name: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The permission the call evaluated to.
    #[serde(default)]
    pub evaluated_permission: Option<SessionEvaluatedPermission>,
    /// The policy that produced `evaluated_permission`.
    #[serde(default)]
    pub evaluation: Option<SessionToolEvaluation>,
    /// Set when cross-posted from a subagent's thread to the primary stream.
    #[serde(default)]
    pub session_thread_id: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.tool_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgentToolResultEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// ID of the `agent.tool_use` event.
    pub tool_use_id: String,
    /// Result content: text, image, document or search result blocks.
    #[serde(default)]
    pub content: Option<Vec<SessionContentBlock>>,
    /// Whether the tool failed.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.thread_message_received`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionThreadMessageReceivedEvent {
    /// Event ID.
    pub id: String,
    /// Text, image, document or redacted blocks.
    pub content: Vec<SessionContentBlock>,
    /// `sthr_…` ID of the sending thread.
    pub from_session_thread_id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The sending callable agent; absent when the primary agent sent it.
    #[serde(default)]
    pub from_agent_name: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `agent.thread_message_sent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionThreadMessageSentEvent {
    /// Event ID.
    pub id: String,
    /// Text, image, document or redacted blocks.
    pub content: Vec<SessionContentBlock>,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// `sthr_…` ID of the receiving thread.
    pub to_session_thread_id: String,
    /// The receiving callable agent; absent when sent to the primary agent.
    #[serde(default)]
    pub to_agent_name: Option<String>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `session.error`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionErrorEvent {
    /// Event ID.
    pub id: String,
    /// What went wrong.
    pub error: SessionEventError,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// The error of a `session.error` event. Unknown error types can still be handled through
    /// `message` and `retry_status` in the raw object.
    #[derive(Eq)]
    pub enum SessionEventError {
        /// `unknown_error`: the documented fallback.
        Unknown(SessionErrorDetail) = "unknown_error",
        /// `model_overloaded_error`.
        ModelOverloaded(SessionErrorDetail) = "model_overloaded_error",
        /// `model_rate_limited_error`.
        ModelRateLimited(SessionErrorDetail) = "model_rate_limited_error",
        /// `model_request_failed_error`.
        ModelRequestFailed(SessionErrorDetail) = "model_request_failed_error",
        /// `mcp_connection_failed_error`.
        McpConnectionFailed(SessionMcpErrorDetail) = "mcp_connection_failed_error",
        /// `mcp_authentication_failed_error`.
        McpAuthenticationFailed(SessionMcpErrorDetail) = "mcp_authentication_failed_error",
        /// `billing_error`: out of credits or spend limit reached.
        Billing(SessionErrorDetail) = "billing_error",
        /// `credential_host_unreachable_error`: a credential's allowed host is outside the
        /// environment's network policy.
        CredentialHostUnreachable(SessionCredentialErrorDetail) = "credential_host_unreachable_error",
    }
}

/// Message and retry status of a session error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionErrorDetail {
    /// Human-readable description.
    pub message: String,
    /// What happens next.
    pub retry_status: SessionRetryStatus,
}

/// An MCP server error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMcpErrorDetail {
    /// The MCP server.
    pub mcp_server_name: String,
    /// Human-readable description.
    pub message: String,
    /// What happens next.
    pub retry_status: SessionRetryStatus,
}

/// A credential error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCredentialErrorDetail {
    /// The affected credential.
    pub credential_id: String,
    /// Human-readable description.
    pub message: String,
    /// What happens next.
    pub retry_status: SessionRetryStatus,
    /// The vault holding the credential.
    pub vault_id: String,
}

tagged_union! {
    /// What happens after a session error.
    #[derive(Eq)]
    pub enum SessionRetryStatus {
        /// `retrying`: the server retries automatically.
        Retrying = "retrying",
        /// `exhausted`: the turn is dead; the session returns to idle.
        Exhausted = "exhausted",
        /// `terminal`: the session will terminate.
        Terminal = "terminal",
    }
}

/// `session.status_idle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatusIdleEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Why the agent stopped.
    pub stop_reason: SessionStopReason,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

tagged_union! {
    /// Why a session or thread went idle.
    #[derive(Eq)]
    pub enum SessionStopReason {
        /// `end_turn`: the turn completed.
        EndTurn = "end_turn",
        /// `requires_action`: blocked on user-input events.
        RequiresAction(SessionRequiresAction) = "requires_action",
        /// `retries_exhausted`: repeated errors exhausted the retry budget.
        RetriesExhausted = "retries_exhausted",
        /// `budget_reached`: the tracked list cost reached the budget.
        BudgetReached = "budget_reached",
    }
}

/// The events a `requires_action` stop waits on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRequiresAction {
    /// IDs of the blocking events.
    pub event_ids: Vec<String>,
}

/// `session.thread_created`, `session.thread_status_running`, `session.thread_status_terminated`
/// and `session.thread_status_rescheduled`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionThreadLifecycleEvent {
    /// Event ID.
    pub id: String,
    /// The callable agent the thread runs.
    pub agent_name: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// `sthr_…` ID of the thread.
    pub session_thread_id: String,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `session.thread_status_idle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionThreadStatusIdleEvent {
    /// Event ID.
    pub id: String,
    /// The agent the thread runs.
    pub agent_name: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// `sthr_…` ID of the thread.
    pub session_thread_id: String,
    /// Why the thread stopped.
    pub stop_reason: SessionStopReason,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `span.outcome_evaluation_start` and `span.outcome_evaluation_ongoing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOutcomeEvaluationProgressEvent {
    /// Event ID.
    pub id: String,
    /// 0-indexed revision cycle.
    pub iteration: u64,
    /// The `outc_…` outcome evaluated.
    pub outcome_id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `span.outcome_evaluation_end`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionOutcomeEvaluationEndEvent {
    /// Event ID.
    pub id: String,
    /// Explanation of the verdict.
    pub explanation: String,
    /// 0-indexed revision cycle.
    pub iteration: u64,
    /// ID of the matching `span.outcome_evaluation_start` event.
    pub outcome_evaluation_start_id: String,
    /// The `outc_…` outcome evaluated.
    pub outcome_id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The verdict. Documented as a string.
    pub result: SessionOutcomeResult,
    /// Grader token usage.
    pub usage: SessionModelUsage,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `span.model_request_end`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModelRequestEndEvent {
    /// Event ID.
    pub id: String,
    /// Whether the request failed.
    #[serde(default)]
    pub is_error: Option<bool>,
    /// ID of the matching `span.model_request_start` event.
    pub model_request_start_id: String,
    /// Token usage of the request.
    pub model_usage: SessionModelUsage,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Token usage of a single model request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModelUsage {
    /// Tokens written to the prompt cache.
    pub cache_creation_input_tokens: u64,
    /// Tokens read from the prompt cache.
    pub cache_read_input_tokens: u64,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Inference speed mode.
    #[serde(default)]
    pub speed: Option<SessionModelSpeed>,
}

/// `session.updated`: only the changed fields are present.
///
/// For `agent`, `budget` and `title`, the outer `Option` says whether the field was present and the
/// inner one whether it was `null`, so "unchanged" and "cleared" stay distinct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUpdatedEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// The new agent snapshot.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    pub agent: Option<Option<SessionAgent>>,
    /// The new budget.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    pub budget: Option<Option<SessionBudget>>,
    /// The full metadata after the update; absent when unchanged or cleared to empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    /// The new title.
    #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    pub title: Option<Option<String>>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `system.message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSystemMessageEvent {
    /// Event ID.
    pub id: String,
    /// Text-only content.
    pub content: Vec<SessionTextContent>,
    /// When the event was processed.
    #[serde(default)]
    pub processed_at: Option<Timestamp>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `session.usage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUsageEvent {
    /// Event ID.
    pub id: String,
    /// When the event was processed.
    pub processed_at: Timestamp,
    /// Cumulative usage at this point.
    pub usage: SessionUsage,
    /// The budget in force.
    #[serde(default)]
    pub budget: Option<SessionBudget>,
    /// Fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
