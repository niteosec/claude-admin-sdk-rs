//! The request body Anthropic sends: the prompt frame and its parts.

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::string_enum::string_enum;

/// The request body, discriminated by the top-level `type`.
///
/// `prompt` is the only event sent today. Other event types will be added; they arrive as
/// [`HookRequest::Other`], and the documented response to one is an allow verdict, not an error
/// status (an error status is a webhook failure and counts toward the circuit breaker).
#[derive(Debug, Clone, PartialEq)]
pub enum HookRequest {
    /// `prompt`: sent once per governed inference request, before inference begins.
    Prompt(PromptFrame),
    /// Any other top-level `type`, with the raw object.
    Other {
        /// The `type` value.
        event_type: String,
        /// The whole request body.
        raw: Value,
    },
}

impl HookRequest {
    /// The `type` value for prompt frames.
    pub const PROMPT: &'static str = "prompt";

    /// Parses a request body. Verify the signature over the same bytes first; see
    /// [`Verifier::verify_request`](crate::Verifier::verify_request).
    pub fn from_slice(body: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(body)
    }

    /// The wire `type`.
    pub fn event_type(&self) -> &str {
        match self {
            HookRequest::Prompt(_) => Self::PROMPT,
            HookRequest::Other { event_type, .. } => event_type,
        }
    }

    /// The body's `request_id`, when present. Documented to equal the `webhook-id` header.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            HookRequest::Prompt(frame) => Some(&frame.request_id),
            HookRequest::Other { raw, .. } => raw.get("request_id").and_then(Value::as_str),
        }
    }

    /// The prompt frame, when this is one.
    pub fn as_prompt(&self) -> Option<&PromptFrame> {
        match self {
            HookRequest::Prompt(frame) => Some(frame),
            HookRequest::Other { .. } => None,
        }
    }
}

impl<'de> Deserialize<'de> for HookRequest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut raw = Value::deserialize(deserializer)?;
        let event_type =
            raw.get("type").and_then(Value::as_str).ok_or_else(|| D::Error::missing_field("type"))?.to_owned();
        if event_type != Self::PROMPT {
            return Ok(HookRequest::Other { event_type, raw });
        }
        if let Value::Object(map) = &mut raw {
            map.remove("type");
        }
        serde_json::from_value(raw).map(HookRequest::Prompt).map_err(D::Error::custom)
    }
}

impl Serialize for HookRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            HookRequest::Prompt(frame) => {
                tagged("type", Self::PROMPT, frame).map_err(S::Error::custom)?.serialize(serializer)
            }
            HookRequest::Other { raw, .. } => raw.serialize(serializer),
        }
    }
}

/// The prompt frame: the conversation transcript up to the point of inference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptFrame {
    /// Opaque per-inference-call identifier for correlation. Equals the `webhook-id` header; a
    /// connection-failure retry reuses it, so it works as an idempotency key.
    pub request_id: String,
    /// Opaque identifier for the organization the request belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// The principal the request is attributed to.
    pub actor: Actor,
    /// The originating application.
    pub source: Source,
    /// The transcript. A turn whose every block is excluded is omitted, so user and assistant
    /// turns need not alternate.
    pub messages: Vec<Message>,
    /// Opaque conversation identifier, when one exists. Don't parse it. For Claude Code it is a
    /// best-effort, client-asserted session identifier.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Public model identifier, when available.
    #[serde(default)]
    pub model: Option<String>,
    /// Reserved extension map, documented as string keys to string values and sent empty today.
    /// Kept as raw JSON so any future key or value parses; absent and `null` read as empty.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub metadata: Map<String, Value>,
    /// Top-level fields this crate version does not know.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn null_as_empty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Map<String, Value>, D::Error> {
    Ok(Option::<Map<String, Value>>::deserialize(deserializer)?.unwrap_or_default())
}

string_enum! {
    /// `source.application`: an open string. Advisory routing metadata, not a trust boundary.
    pub enum Application {
        /// claude.ai.
        ClaudeAi = "claude-ai",
        /// Claude Code.
        ClaudeCode = "claude-code",
        /// Cowork.
        Cowork = "cowork",
        /// The synthetic request sent by **Test connection** and by circuit-breaker recovery
        /// checks. Carries no user content; answer it normally.
        ConfigTest = "config-test",
    }
}

/// `source`: the originating application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// The application.
    pub application: Application,
}

/// `actor`: the principal the request is attributed to, discriminated by `actor.type`.
///
/// `user` is the only type sent today; a future type guarantees only that `type` is present and is
/// kept whole in [`Actor::Other`].
#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    /// `user`.
    User(UserActor),
    /// Any other `actor.type`, with the raw object.
    Other {
        /// The `type` value.
        actor_type: String,
        /// The whole actor object.
        raw: Value,
    },
}

impl Actor {
    /// The wire `type`.
    pub fn actor_type(&self) -> &str {
        match self {
            Actor::User(_) => "user",
            Actor::Other { actor_type, .. } => actor_type,
        }
    }
}

/// `user` actor. Both fields can be `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserActor {
    /// A tagged identifier, stable across requests for the same account.
    #[serde(default)]
    pub id: Option<String>,
    /// Email address, when available.
    #[serde(default)]
    pub email_address: Option<String>,
}

impl<'de> Deserialize<'de> for Actor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let actor_type =
            raw.get("type").and_then(Value::as_str).ok_or_else(|| D::Error::missing_field("type"))?.to_owned();
        match actor_type.as_str() {
            "user" => serde_json::from_value(raw).map(Actor::User).map_err(D::Error::custom),
            _ => Ok(Actor::Other { actor_type, raw }),
        }
    }
}

impl Serialize for Actor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Actor::User(inner) => tagged("type", "user", inner).map_err(S::Error::custom)?.serialize(serializer),
            Actor::Other { raw, .. } => raw.serialize(serializer),
        }
    }
}

string_enum! {
    /// `messages[].role`.
    pub enum Role {
        /// The end user. Tool results also appear under this role.
        User = "user",
        /// Claude.
        Assistant = "assistant",
    }
}

/// One turn of the transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// `user` or `assistant`.
    pub role: Role,
    /// The turn's content blocks.
    pub content: Vec<ContentBlock>,
}

/// A content block, discriminated by `type`.
///
/// Apart from `type`, a `text` block's `text`, and a `tool_result` block's `content` and
/// `is_error`, every documented field can be `null`. A block of an unrecognized type guarantees
/// only `type`; it is kept whole in [`ContentBlock::Other`] and must not cause a rejection.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    /// `text`.
    Text(TextBlock),
    /// `tool_use`.
    ToolUse(ToolUseBlock),
    /// `tool_result`.
    ToolResult(ToolResultBlock),
    /// `attachment`.
    Attachment(AttachmentBlock),
    /// Any other block `type`, with the raw object.
    Other {
        /// The `type` value.
        block_type: String,
        /// The whole block.
        raw: Value,
    },
}

impl ContentBlock {
    /// The wire `type`.
    pub fn block_type(&self) -> &str {
        match self {
            ContentBlock::Text(_) => "text",
            ContentBlock::ToolUse(_) => "tool_use",
            ContentBlock::ToolResult(_) => "tool_result",
            ContentBlock::Attachment(_) => "attachment",
            ContentBlock::Other { block_type, .. } => block_type,
        }
    }
}

/// `text` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBlock {
    /// The text content.
    pub text: String,
}

/// `tool_use` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// The identifier the matching tool result references.
    #[serde(default)]
    pub id: Option<String>,
    /// The tool's name.
    #[serde(default)]
    pub tool_name: Option<String>,
    /// The arguments the model passed to the tool.
    #[serde(default)]
    pub input: Option<Value>,
}

/// `tool_result` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// The tool's output as text, parts joined by newlines. Binary parts such as images are
    /// replaced by placeholder markers.
    pub content: String,
    /// Whether the tool call failed.
    pub is_error: bool,
    /// The tool's name.
    #[serde(default)]
    pub tool_name: Option<String>,
    /// The `id` of the matching `tool_use` block.
    #[serde(default)]
    pub tool_use_id: Option<String>,
}

/// `attachment` block. Raw attachment bytes are never sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentBlock {
    /// The original file name or path. `null` for an image, for example.
    #[serde(default)]
    pub file_name: Option<String>,
    /// The attachment's media type.
    #[serde(default)]
    pub media_type: Option<String>,
    /// The size of the original file.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// Text content when available: extracted document text, an audio transcript, or link
    /// metadata.
    #[serde(default)]
    pub text: Option<String>,
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let block_type =
            raw.get("type").and_then(Value::as_str).ok_or_else(|| D::Error::missing_field("type"))?.to_owned();
        let block = match block_type.as_str() {
            "text" => serde_json::from_value(raw).map(ContentBlock::Text),
            "tool_use" => serde_json::from_value(raw).map(ContentBlock::ToolUse),
            "tool_result" => serde_json::from_value(raw).map(ContentBlock::ToolResult),
            "attachment" => serde_json::from_value(raw).map(ContentBlock::Attachment),
            _ => Ok(ContentBlock::Other { block_type, raw }),
        };
        block.map_err(D::Error::custom)
    }
}

impl Serialize for ContentBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let tag = self.block_type();
        let value = match self {
            ContentBlock::Text(inner) => tagged("type", tag, inner),
            ContentBlock::ToolUse(inner) => tagged("type", tag, inner),
            ContentBlock::ToolResult(inner) => tagged("type", tag, inner),
            ContentBlock::Attachment(inner) => tagged("type", tag, inner),
            ContentBlock::Other { raw, .. } => Ok(raw.clone()),
        }
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

fn tagged<T: Serialize>(key: &str, tag: &str, inner: &T) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(inner)?;
    if let Value::Object(map) = &mut value {
        map.insert(key.to_owned(), Value::String(tag.to_owned()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_type_is_an_error() {
        assert!(serde_json::from_value::<HookRequest>(json!({"request_id": "req_x"})).is_err());
        assert!(serde_json::from_value::<ContentBlock>(json!({"text": "hi"})).is_err());
        assert!(serde_json::from_value::<Actor>(json!({"id": "user_x"})).is_err());
    }

    #[test]
    fn metadata_absent_or_null_reads_empty() {
        let base = json!({
            "type": "prompt", "request_id": "req_x", "actor": {"type": "user"},
            "source": {"application": "claude-code"}, "messages": []
        });
        let HookRequest::Prompt(frame) = serde_json::from_value(base.clone()).unwrap() else { panic!() };
        assert!(frame.metadata.is_empty());
        assert!(frame.extra.is_empty());
        assert_eq!(frame.actor, Actor::User(UserActor::default()));
        assert_eq!(frame.source.application, Application::ClaudeCode);

        let mut with_null = base;
        with_null["metadata"] = Value::Null;
        let HookRequest::Prompt(frame) = serde_json::from_value(with_null).unwrap() else { panic!() };
        assert!(frame.metadata.is_empty());
    }

    #[test]
    fn known_block_with_a_missing_required_field_is_an_error() {
        assert!(serde_json::from_value::<ContentBlock>(json!({"type": "tool_result", "content": "x"})).is_err());
    }
}
