//! Content blocks carried by session events and deployment initial events.

use serde::{Deserialize, Serialize};

use crate::runtime::tagged_union;

tagged_union! {
    /// A content block, discriminated by `type`.
    ///
    /// Each event documents which blocks it carries (a user message: text, image, document,
    /// redacted; a tool result: text, image, document, search result); one union covers them all.
    #[derive(Eq)]
    pub enum SessionContentBlock {
        /// `text`: regular text content.
        Text(SessionTextBlock) = "text",
        /// `image`: an image given inline, by URL or by file ID.
        Image(SessionImageBlock) = "image",
        /// `document`: a document given inline, as text, by URL or by file ID.
        Document(SessionDocumentBlock) = "document",
        /// `redacted`: a placeholder for content withheld by Anthropic model policy.
        Redacted = "redacted",
        /// `search_result`: a web search result.
        SearchResult(SessionSearchResultBlock) = "search_result",
    }
}

/// A `text` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTextBlock {
    /// The text.
    pub text: String,
}

/// An `image` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionImageBlock {
    /// Where the image comes from.
    pub source: SessionImageSource,
}

tagged_union! {
    /// The source of an image block.
    #[derive(Eq)]
    pub enum SessionImageSource {
        /// `base64`: inline image data.
        Base64(SessionBase64Source) = "base64",
        /// `url`: an image fetched from a URL.
        Url(SessionUrlSource) = "url",
        /// `file`: a previously uploaded file.
        File(SessionFileSource) = "file",
    }
}

/// A `document` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDocumentBlock {
    /// Where the document comes from.
    pub source: SessionDocumentSource,
    /// Additional context about the document for the model.
    #[serde(default)]
    pub context: Option<String>,
    /// The document title.
    #[serde(default)]
    pub title: Option<String>,
}

tagged_union! {
    /// The source of a document block.
    #[derive(Eq)]
    pub enum SessionDocumentSource {
        /// `base64`: inline document data.
        Base64(SessionBase64Source) = "base64",
        /// `text`: plain text (`media_type` is `text/plain`).
        Text(SessionBase64Source) = "text",
        /// `url`: a document fetched from a URL.
        Url(SessionUrlSource) = "url",
        /// `file`: a previously uploaded file.
        File(SessionFileSource) = "file",
    }
}

/// Inline data with its MIME type (`base64` image or document sources, `text` document sources,
/// where `data` is the plain text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBase64Source {
    /// The data: base64 for `base64` sources, the text itself for `text` sources.
    pub data: String,
    /// MIME type, for example `image/png`, `application/pdf` or `text/plain`.
    pub media_type: String,
}

/// A `url` source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUrlSource {
    /// The URL to fetch.
    pub url: String,
}

/// A `file` source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFileSource {
    /// ID of a previously uploaded file.
    pub file_id: String,
}

/// A `search_result` content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSearchResultBlock {
    /// Citation settings.
    pub citations: SessionSearchResultCitations,
    /// Text blocks from the result.
    pub content: Vec<SessionTextContent>,
    /// The URL source of the result.
    pub source: String,
    /// The result title.
    pub title: String,
}

/// Citation settings of a search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSearchResultCitations {
    /// Whether citations are enabled.
    pub enabled: bool,
}

/// A text-only block (`{type: "text", text}`): search result content and system message content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTextContent {
    /// Always `text`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The text.
    pub text: String,
}

tagged_union! {
    /// A rubric for grading an outcome.
    #[derive(Eq)]
    pub enum SessionRubric {
        /// `file`: a rubric uploaded through the Files API.
        File(SessionFileSource) = "file",
        /// `text`: an inline rubric.
        Text(SessionTextRubric) = "text",
    }
}

/// An inline `text` rubric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTextRubric {
    /// Rubric content, plain text or markdown.
    pub content: String,
}
