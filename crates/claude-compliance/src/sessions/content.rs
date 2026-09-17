//! Transcript content blocks shared by local and remote session messages.

use serde::de::DeserializeOwned;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// A content block, discriminated by `type`.
///
/// Unknown block types are kept whole in [`SessionContentBlock::Other`].
#[derive(Debug, Clone, PartialEq)]
pub enum SessionContentBlock {
    /// `text`.
    Text(SessionTextBlock),
    /// `tool_use`: a tool invocation requested by the assistant.
    ToolUse(SessionToolUseBlock),
    /// `tool_result`: the result of a tool invocation.
    ToolResult(SessionToolResultBlock),
    /// Any other `type`, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole block.
        raw: Value,
    },
}

impl SessionContentBlock {
    /// The wire `type`.
    pub fn kind(&self) -> &str {
        match self {
            Self::Text(_) => "text",
            Self::ToolUse(_) => "tool_use",
            Self::ToolResult(_) => "tool_result",
            Self::Other { kind, .. } => kind,
        }
    }
}

/// `text` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTextBlock {
    /// Text from the user or the assistant.
    pub text: String,
    /// Whether `text` was shortened or stands in for content not shown (server bound ~1 MiB, not
    /// adjustable). On local sessions, a bracketed marker the server inserted is also `true`.
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `tool_use` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionToolUseBlock {
    /// Tool-use ID, for example `toolu_…`.
    #[serde(default)]
    pub id: Option<String>,
    /// The tool arguments as a JSON-encoded string. Not valid JSON when [`Self::truncated`] is set.
    pub input: String,
    /// Tool name.
    pub name: String,
    /// Whether `input` was cut to `tool_use_input_max_bytes`.
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SessionToolUseBlock {
    /// Decodes `input` as JSON. `None` when the input was truncated, since a cut document is not
    /// valid JSON; request the server maximum with `tool_use_input_max_bytes(-1)` to avoid that.
    pub fn parse_input<T: DeserializeOwned>(&self) -> Option<serde_json::Result<T>> {
        (!self.truncated).then(|| serde_json::from_str(&self.input))
    }
}

/// `tool_result` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionToolResultBlock {
    /// Text returned by the tool. Non-text items are omitted by the server.
    pub content: Vec<SessionToolResultItem>,
    /// Whether the tool reported an error.
    pub is_error: bool,
    /// Tool name.
    pub name: String,
    /// The `tool_use` block this answers.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// Whether text items were cut to `tool_result_max_bytes` or items were omitted.
    #[serde(default)]
    pub truncated: bool,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An item inside a `tool_result` block, discriminated by `type`.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionToolResultItem {
    /// `text`.
    Text {
        /// Text returned by the tool.
        text: String,
    },
    /// Any other `type`, with the raw object.
    Other {
        /// The `type` value.
        kind: String,
        /// The whole item.
        raw: Value,
    },
}

/// The `type` of a union object.
pub(super) fn split_tag<E: serde::de::Error>(raw: &Value) -> Result<String, E> {
    raw.get("type").and_then(Value::as_str).map(str::to_owned).ok_or_else(|| E::missing_field("type"))
}

/// Decodes a union object into a variant, without its `type`.
pub(super) fn untagged<T: DeserializeOwned>(mut raw: Value) -> serde_json::Result<T> {
    if let Value::Object(map) = &mut raw {
        map.remove("type");
    }
    serde_json::from_value(raw)
}

/// Encodes a variant with its `type`.
pub(super) fn tagged<T: Serialize>(tag: &str, inner: &T) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(inner)?;
    if let Value::Object(map) = &mut value {
        map.insert("type".to_owned(), Value::String(tag.to_owned()));
    }
    Ok(value)
}

impl<'de> Deserialize<'de> for SessionContentBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let kind = split_tag::<D::Error>(&raw)?;
        let block = match kind.as_str() {
            "text" => untagged(raw).map(Self::Text),
            "tool_use" => untagged(raw).map(Self::ToolUse),
            "tool_result" => untagged(raw).map(Self::ToolResult),
            _ => Ok(Self::Other { kind, raw }),
        };
        block.map_err(D::Error::custom)
    }
}

impl Serialize for SessionContentBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let tag = self.kind();
        let value = match self {
            Self::Text(inner) => tagged(tag, inner),
            Self::ToolUse(inner) => tagged(tag, inner),
            Self::ToolResult(inner) => tagged(tag, inner),
            Self::Other { raw, .. } => Ok(raw.clone()),
        }
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

#[derive(Serialize, Deserialize)]
struct TextItem {
    text: String,
}

impl<'de> Deserialize<'de> for SessionToolResultItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let kind = split_tag::<D::Error>(&raw)?;
        match kind.as_str() {
            "text" => untagged::<TextItem>(raw).map(|item| Self::Text { text: item.text }).map_err(D::Error::custom),
            _ => Ok(Self::Other { kind, raw }),
        }
    }
}

impl Serialize for SessionToolResultItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Text { text } => tagged("text", &TextItem { text: text.clone() }).map_err(S::Error::custom)?,
            Self::Other { raw, .. } => raw.clone(),
        };
        value.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn known_block_round_trips_without_a_duplicate_type_in_extra() {
        let raw = json!({"type": "tool_use", "id": "toolu_x", "input": "{}", "name": "Bash", "truncated": false});
        let block: SessionContentBlock = serde_json::from_value(raw.clone()).unwrap();
        let SessionContentBlock::ToolUse(tool_use) = &block else { panic!("{block:?}") };
        assert!(tool_use.extra.is_empty());
        assert_eq!(serde_json::to_value(&block).unwrap(), raw);
    }

    #[test]
    fn missing_type_is_an_error() {
        assert!(serde_json::from_value::<SessionContentBlock>(json!({"text": "x"})).is_err());
    }
}
