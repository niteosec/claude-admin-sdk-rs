//! Request paths with safely encoded identifiers.

use crate::{Error, Result};

/// A request path relative to the API origin.
///
/// Static route text is split on `/`. Caller-supplied identifiers are added with [`ApiPath::id`],
/// which percent-encodes them as a single segment (a `/` inside an ID cannot add a path level) and
/// rejects empty, `.` and `..` values.
///
/// ```
/// use claude_api_core::ApiPath;
///
/// let path = ApiPath::new("v1/compliance/apps/chats").id("claude_chat_01H5")?.then("messages");
/// # Ok::<(), claude_api_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiPath {
    segments: Vec<String>,
}

impl ApiPath {
    /// A path from static route text, for example `v1/compliance/activities`.
    pub fn new(route: &'static str) -> Self {
        Self { segments: Vec::new() }.then(route)
    }

    /// Appends static route text.
    pub fn then(mut self, route: &'static str) -> Self {
        self.segments.extend(route.split('/').filter(|segment| !segment.is_empty()).map(str::to_owned));
        self
    }

    /// Appends one identifier segment.
    pub fn id(mut self, id: &str) -> Result<Self> {
        if id.is_empty() || id == "." || id == ".." {
            return Err(Error::InvalidArgument(format!("invalid path identifier {id:?}")));
        }
        self.segments.push(id.to_owned());
        Ok(self)
    }

    pub(crate) fn segments(&self) -> &[String] {
        &self.segments
    }
}
