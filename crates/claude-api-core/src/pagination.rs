//! Pagination shapes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An opaque pagination cursor (`first_id` / `last_id`, passed back as `after_id` / `before_id`).
///
/// Anthropic documents cursors as opaque and unstable. Observed values are base64 JSON, not the
/// activity IDs shown in the documentation example. Do not parse or construct them.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor(String);

impl Cursor {
    /// Restores a cursor persisted from an earlier response.
    pub fn from_persisted(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The cursor string, for persisting.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cursor").field(&self.0).finish()
    }
}

/// A cursor-paginated list: `{data, has_more, first_id, last_id}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CursorPage<T> {
    /// The records.
    pub data: Vec<T>,
    /// Whether another page exists in the requested direction.
    pub has_more: bool,
    /// Cursor to the first record; pass as `before_id` to go back. `null` on an empty page.
    #[serde(default)]
    pub first_id: Option<Cursor>,
    /// Cursor to the last record; pass as `after_id` to go forward. `null` on an empty page.
    #[serde(default)]
    pub last_id: Option<Cursor>,
}
