//! Binary downloads.

use std::pin::Pin;

use bytes::Bytes;
use futures_core::Stream;
use futures_util::TryStreamExt;
use http::HeaderMap;

use crate::response::{ResponseMeta, header};
use crate::{Error, Result};

/// A stream of body chunks.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>;

/// A successful binary response whose body has not been read yet.
///
/// Retries apply until the response headers arrive; a failure while reading the body is returned
/// to the caller, who can retry the whole download.
pub struct Download {
    /// Response metadata.
    pub meta: ResponseMeta,
    /// `Content-Type`. For stored documents this can name the original format even when the bytes
    /// are extracted text: identify formats from the bytes.
    pub content_type: Option<String>,
    /// `Content-Length`, when sent (chunked responses omit it).
    pub content_length: Option<u64>,
    /// `Content-MD5` (base64, RFC 1864) of the served bytes.
    pub content_md5: Option<String>,
    /// File name decoded from `Content-Disposition` (`filename*=utf-8''…`, falling back to
    /// `filename=`). **Untrusted user input**: never use it as a path without sanitising.
    pub filename: Option<String>,
    response: reqwest::Response,
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download")
            .field("meta", &self.meta)
            .field("content_type", &self.content_type)
            .field("content_length", &self.content_length)
            .field("content_md5", &self.content_md5)
            .field("filename", &self.filename)
            .finish_non_exhaustive()
    }
}

impl Download {
    pub(crate) fn new(meta: ResponseMeta, response: reqwest::Response) -> Self {
        let headers = response.headers();
        Self {
            meta,
            content_type: header(headers, "content-type").map(str::to_owned),
            content_length: response.content_length(),
            content_md5: header(headers, "content-md5").map(str::to_owned),
            filename: filename(headers),
            response,
        }
    }

    /// The body as a stream of chunks.
    pub fn into_stream(self) -> ByteStream {
        Box::pin(self.response.bytes_stream().map_err(Error::from))
    }

    /// The whole body in memory.
    pub async fn bytes(self) -> Result<Bytes> {
        Ok(self.response.bytes().await?)
    }
}

fn filename(headers: &HeaderMap) -> Option<String> {
    let disposition = header(headers, "content-disposition")?;
    let params = || disposition.split(';').map(str::trim);
    let extended = params().find_map(|param| {
        let value = param.strip_prefix("filename*=")?;
        let (charset, rest) = value.split_once('\'')?;
        let (_language, encoded) = rest.split_once('\'')?;
        let decoded = percent_encoding::percent_decode_str(encoded);
        if charset.eq_ignore_ascii_case("utf-8") {
            decoded.decode_utf8().ok().map(|name| name.into_owned())
        } else {
            Some(decoded.decode_utf8_lossy().into_owned())
        }
    });
    extended.or_else(|| {
        params().find_map(|param| param.strip_prefix("filename=").map(|value| value.trim_matches('"').to_owned()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    fn with(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("content-disposition", HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn decodes_rfc5987_filenames() {
        assert_eq!(
            filename(&with("attachment; filename*=utf-8''dashboard%20mockup%E2%80%94v1.pdf")).as_deref(),
            Some("dashboard mockup—v1.pdf")
        );
    }

    #[test]
    fn falls_back_to_the_plain_parameter() {
        assert_eq!(filename(&with(r#"attachment; filename="report.csv""#)).as_deref(), Some("report.csv"));
        assert_eq!(filename(&HeaderMap::new()), None);
    }
}
