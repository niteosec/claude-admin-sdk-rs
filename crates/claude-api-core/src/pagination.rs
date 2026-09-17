//! Pagination shapes.
//!
//! Anthropic's organization APIs use two schemes. Cursor lists return `first_id` / `last_id` and
//! take `after_id` / `before_id`. Page-token lists return `next_page` and take `page`; some of them
//! also return `has_more`, others (the session endpoints) only `next_page`.

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

/// An opaque page token (`next_page`, passed back as `page`).
///
/// Some tokens expire (local session message walks: 24 hours after the first page). Do not parse
/// or construct them.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageToken(String);

impl PageToken {
    /// Restores a token persisted from an earlier response.
    pub fn from_persisted(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The token string, for persisting.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PageToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PageToken").field(&self.0).finish()
    }
}

/// A page-token list: `{data, has_more?, next_page}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TokenPage<T> {
    /// The records.
    pub data: Vec<T>,
    /// Whether another page exists. Absent on endpoints that signal the end only with a `null`
    /// `next_page`.
    #[serde(default)]
    pub has_more: Option<bool>,
    /// Token for the next page, `null` at the end.
    #[serde(default)]
    pub next_page: Option<PageToken>,
}

impl<T> TokenPage<T> {
    /// The token to request next, or `None` when the walk is complete. `has_more: false` wins over a
    /// non-null token.
    pub fn next(&self) -> Option<&PageToken> {
        match self.has_more {
            Some(false) => None,
            _ => self.next_page.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_page_end_detection() {
        let page: TokenPage<u8> = serde_json::from_str(r#"{"data":[],"next_page":"t"}"#).unwrap();
        assert_eq!(page.next().map(PageToken::as_str), Some("t"));
        let page: TokenPage<u8> = serde_json::from_str(r#"{"data":[],"has_more":false,"next_page":"t"}"#).unwrap();
        assert_eq!(page.next(), None);
        let page: TokenPage<u8> = serde_json::from_str(r#"{"data":[],"next_page":null}"#).unwrap();
        assert_eq!(page.next(), None);
    }
}
