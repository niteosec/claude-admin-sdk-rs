//! Shapes and helpers shared by the chat, project and Code Artifact endpoints.

use claude_api_core::{Error, Result};
use jiff::Timestamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A user reference on a chat, project, project document or Code Artifact: `{id, email_address}`.
///
/// The endpoints return `null` in its place when the user can no longer be resolved (account
/// deleted, or no longer a member of an organization the key may read).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentUser {
    /// `user_…` ID.
    pub id: String,
    /// The user's current email address.
    pub email_address: String,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `created_at.*` / `updated_at.*` operators set on a request, at most one per key.
#[derive(Debug, Clone, Default)]
pub(crate) struct TimeFilters(Vec<(&'static str, Timestamp)>);

impl TimeFilters {
    pub(crate) fn set(&mut self, key: &'static str, at: Timestamp) {
        self.0.retain(|(existing, _)| *existing != key);
        self.0.push((key, at));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn push_to(&self, query: &mut Vec<(&'static str, String)>) {
        query.extend(self.0.iter().map(|(key, at)| (*key, at.to_string())));
    }
}

/// Setter methods for [`TimeFilters`] fields: `field: method => "wire.key", …`.
macro_rules! time_filter_setters {
    ($field:ident: $($method:ident => $key:literal),+ $(,)?) => {
        $(
            #[doc = concat!("`", $key, "`.")]
            pub fn $method(mut self, at: ::jiff::Timestamp) -> Self {
                self.$field.set($key, at);
                self
            }
        )+
    };
}
pub(crate) use time_filter_setters;

/// Implements `kind()`, `Deserialize` and `Serialize` for an enum discriminated by `type` whose known
/// variants wrap one struct each and whose last variant is `Other { kind, raw }`.
macro_rules! tagged_union {
    ($name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        impl $name {
            /// The wire `type`.
            pub fn kind(&self) -> &str {
                match self {
                    $( Self::$variant(_) => $tag, )+
                    Self::Other { kind, .. } => kind,
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> ::std::result::Result<Self, D::Error> {
                use ::serde::de::Error as _;
                let raw = ::serde_json::Value::deserialize(deserializer)?;
                let kind = raw
                    .get("type")
                    .and_then(::serde_json::Value::as_str)
                    .ok_or_else(|| D::Error::missing_field("type"))?
                    .to_owned();
                let decoded = match kind.as_str() {
                    $( $tag => $crate::content_common::untagged(raw).map(Self::$variant), )+
                    _ => Ok(Self::Other { kind, raw }),
                };
                decoded.map_err(D::Error::custom)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error> {
                use ::serde::ser::Error as _;
                let value = match self {
                    $( Self::$variant(inner) => $crate::content_common::tagged($tag, inner), )+
                    Self::Other { raw, .. } => Ok(raw.clone()),
                }
                .map_err(S::Error::custom)?;
                ::serde::Serialize::serialize(&value, serializer)
            }
        }
    };
}
pub(crate) use tagged_union;

/// Decodes a variant body without its `type` key, so the key does not land in `extra`.
pub(crate) fn untagged<T: DeserializeOwned>(mut raw: Value) -> serde_json::Result<T> {
    if let Value::Object(map) = &mut raw {
        map.remove("type");
    }
    serde_json::from_value(raw)
}

/// Encodes a variant body with its `type` key.
pub(crate) fn tagged<T: Serialize>(tag: &str, inner: &T) -> serde_json::Result<Value> {
    let mut value = serde_json::to_value(inner)?;
    if let Value::Object(map) = &mut value {
        map.insert("type".to_owned(), Value::String(tag.to_owned()));
    }
    Ok(value)
}

/// Rejects a page size outside `1..=max`.
pub(crate) fn check_limit(limit: u32, max: u32) -> Result<()> {
    if (1..=max).contains(&limit) {
        Ok(())
    } else {
        Err(Error::InvalidArgument(format!("limit must be between 1 and {max}, got {limit}")))
    }
}

/// Rejects an array parameter with more than `max` items.
pub(crate) fn check_max_items(name: &str, items: &[String], max: usize) -> Result<()> {
    if items.len() <= max {
        Ok(())
    } else {
        Err(Error::InvalidArgument(format!("{name} accepts at most {max} values, got {}", items.len())))
    }
}
