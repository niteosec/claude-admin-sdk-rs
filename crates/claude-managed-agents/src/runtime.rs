//! Plumbing shared by the runtime endpoints (sessions, deployments, memory, dreams): the page
//! envelope, the list-request builder and the `type`-tagged union macro.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Error, PageToken, RequestOptions, Result};
use futures_core::Stream;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// A page from a runtime list endpoint: `{data, next_page}`, plus `prev_page` on the session list.
///
/// Pass `next_page` (or, on the session list, `prev_page`) back as `page` to move through the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimePage<T> {
    /// The records. Documented as optional on some endpoints; an absent `data` decodes as empty.
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    /// Token for the next page, `null` at the end.
    #[serde(default)]
    pub next_page: Option<PageToken>,
    /// Token for the previous page, `null` on the first page. Only the session list documents it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_page: Option<PageToken>,
    /// Envelope fields this crate does not model.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A boxed stream of records. `Unpin`, so `TryStreamExt::try_next` works on it directly.
pub(crate) type Records<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send + 'static>>;

/// Decodes a present field as `Some(value)`, so `null` becomes `Some(None)` and an absent field
/// (through `#[serde(default)]`) stays `None`.
pub(crate) fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

/// The `type` value of objects whose `type` is a single constant and which are also decoded as a
/// variant of a [`tagged_union!`] (where the union strips `type` before decoding the payload).
pub(crate) fn memory_type() -> String {
    "memory".to_owned()
}

/// A path whose identifiers may have been rejected; the error surfaces when the request is sent.
pub(crate) fn deferred(path: Result<ApiPath>) -> Result<ApiPath, String> {
    path.map_err(|error| match error {
        Error::InvalidArgument(message) => message,
        other => other.to_string(),
    })
}

/// Fetches one record through the client's transport.
pub(crate) async fn get<T: DeserializeOwned>(
    api: &ApiClient,
    path: Result<ApiPath>,
    query: &[(&str, String)],
    options: &RequestOptions,
) -> Result<ApiResponse<T>> {
    api.get_json_with(&path?, query, options).await
}

/// A `next_page` list request: `limit`, `page` and endpoint filters, in insertion order.
#[derive(Debug, Clone)]
pub(crate) struct ListRequest {
    api: ApiClient,
    options: RequestOptions,
    path: Result<ApiPath, String>,
    max_limit: Option<u32>,
    limit: Option<u32>,
    page: Option<PageToken>,
    params: Vec<(&'static str, String)>,
    invalid: Option<String>,
}

impl ListRequest {
    pub(crate) fn new(api: ApiClient, options: RequestOptions, path: Result<ApiPath>, max_limit: Option<u32>) -> Self {
        Self {
            api,
            options,
            path: deferred(path),
            max_limit,
            limit: None,
            page: None,
            params: Vec::new(),
            invalid: None,
        }
    }

    pub(crate) fn limit(&mut self, limit: u32) {
        self.limit = Some(limit);
    }

    pub(crate) fn limit_value(&self) -> Option<u32> {
        self.limit
    }

    pub(crate) fn page(&mut self, page: PageToken) {
        self.page = Some(page);
    }

    /// Sets a single-valued parameter, replacing an earlier value.
    pub(crate) fn set(&mut self, key: &'static str, value: impl Into<String>) {
        self.params.retain(|(existing, _)| *existing != key);
        self.params.push((key, value.into()));
    }

    /// Appends one value of a repeatable parameter.
    pub(crate) fn push(&mut self, key: &'static str, value: impl Into<String>) {
        self.params.push((key, value.into()));
    }

    /// The value of a single-valued parameter, when set.
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(existing, _)| *existing == key).map(|(_, value)| value.as_str())
    }

    /// Records an argument error, reported when the request is sent.
    pub(crate) fn reject(&mut self, message: String) {
        self.invalid.get_or_insert(message);
    }

    pub(crate) fn query(&self) -> Result<Vec<(&'static str, String)>> {
        if let Some(message) = &self.invalid {
            return Err(Error::InvalidArgument(message.clone()));
        }
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            match self.max_limit {
                Some(max) if !(1..=max).contains(&limit) => {
                    return Err(Error::InvalidArgument(format!("limit must be between 1 and {max}, got {limit}")));
                }
                _ => query.push(("limit", limit.to_string())),
            }
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        query.extend(self.params.iter().cloned());
        Ok(query)
    }

    pub(crate) async fn send<T: DeserializeOwned>(&self) -> Result<ApiResponse<RuntimePage<T>>> {
        let path = self.path.clone().map_err(Error::InvalidArgument)?;
        let query = self.query()?;
        self.api.get_json_with(&path, &query, &self.options).await
    }

    pub(crate) fn stream<T: DeserializeOwned + Send + 'static>(self) -> Records<T> {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send::<T>().await?.body;
                for record in page.data {
                    yield record;
                }
                match page.next_page {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }
}

/// Declares the `limit`, `page`, `send` and `stream` methods of a list builder whose `inner` field
/// is a [`ListRequest`].
macro_rules! list_methods {
    ($record:ty, $stream:ty, $limit_doc:literal) => {
        #[doc = $limit_doc]
        pub fn limit(mut self, limit: u32) -> Self {
            self.inner.limit(limit);
            self
        }

        /// Start from a previous response's `next_page`.
        pub fn page(mut self, page: claude_api_core::PageToken) -> Self {
            self.inner.page(page);
            self
        }

        /// Fetches one page.
        pub async fn send(
            &self,
        ) -> claude_api_core::Result<claude_api_core::ApiResponse<$crate::runtime::RuntimePage<$record>>> {
            if let Some(message) = self.check() {
                return Err(claude_api_core::Error::InvalidArgument(message));
            }
            self.inner.send().await
        }

        /// Streams every record, following `next_page` until it is `null`. Starts from
        /// [`Self::page`] when set.
        pub fn stream(self) -> $stream {
            let check = self.check();
            let mut inner = self.inner;
            if let Some(message) = check {
                inner.reject(message);
            }
            inner.stream()
        }
    };
}

/// Declares an enum decoded from an object whose `type` field selects the variant.
///
/// Each variant is either a unit variant (the object carries only `type`) or wraps a struct holding
/// the other fields; `type` is removed before the struct is decoded and put back on encoding.
/// Unknown `type` values decode to `Other { kind, raw }` with the whole object and re-encode
/// unchanged.
macro_rules! tagged_union {
    (@de $raw:ident, $name:ident :: $variant:ident) => {
        Ok($name::$variant)
    };
    (@de $raw:ident, $name:ident :: $variant:ident, $inner:ty) => {
        serde_json::from_value::<$inner>($raw).map($name::$variant)
    };
    (@ser $this:expr, $name:ident :: $variant:ident) => {
        matches!($this, $name::$variant).then(|| Ok(serde_json::Value::Object(serde_json::Map::new())))
    };
    (@ser $this:expr, $name:ident :: $variant:ident, $inner:ty) => {
        if let $name::$variant(inner) = $this { Some(serde_json::to_value(inner)) } else { None }
    };
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident $( ($inner:ty) )? = $tag:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant $( ($inner) )?, )+
            /// A `type` this crate version does not know, with the whole object.
            Other {
                /// The `type` value.
                kind: String,
                /// The whole object.
                raw: serde_json::Value,
            },
        }

        impl $name {
            /// The wire `type`.
            pub fn kind(&self) -> &str {
                match self {
                    $( Self::$variant { .. } => $tag, )+
                    Self::Other { kind, .. } => kind,
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                use serde::de::Error as _;
                let raw = serde_json::Value::deserialize(deserializer)?;
                let kind = raw
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| D::Error::missing_field("type"))?
                    .to_owned();
                let decoded: serde_json::Result<$name> = match kind.as_str() {
                    $( $tag => {
                        #[allow(unused_mut, unused_variables)]
                        let mut raw = raw;
                        if let serde_json::Value::Object(map) = &mut raw {
                            map.remove("type");
                        }
                        $crate::runtime::tagged_union!(@de raw, $name::$variant $(, $inner)?)
                    } )+
                    _ => Ok($name::Other { kind, raw }),
                };
                decoded.map_err(D::Error::custom)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::Error as _;
                let value = match self {
                    $name::Other { raw, .. } => raw.clone(),
                    _ => {
                        let mut value = None::<serde_json::Result<serde_json::Value>>
                            $( .or_else(|| $crate::runtime::tagged_union!(@ser self, $name::$variant $(, $inner)?)) )+
                            .unwrap_or_else(|| Ok(serde_json::Value::Object(serde_json::Map::new())))
                            .map_err(S::Error::custom)?;
                        if let serde_json::Value::Object(map) = &mut value {
                            map.insert("type".to_owned(), serde_json::Value::String(self.kind().to_owned()));
                        }
                        value
                    }
                };
                value.serialize(serializer)
            }
        }
    };
}

pub(crate) use {list_methods, tagged_union};

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::{Map, Value, json};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Payload {
        pub id: String,
        #[serde(flatten)]
        pub extra: Map<String, Value>,
    }

    tagged_union! {
        /// Test union.
        pub enum Sample {
            /// Unit.
            Empty = "empty",
            /// With fields.
            Full(Payload) = "full",
        }
    }

    #[test]
    fn known_variants_round_trip_with_their_tag_and_no_tag_in_extra() {
        for raw in [json!({"type": "empty"}), json!({"type": "full", "id": "x", "later": 1})] {
            let decoded: Sample = serde_json::from_value(raw.clone()).unwrap();
            if let Sample::Full(payload) = &decoded {
                assert_eq!(payload.extra.keys().collect::<Vec<_>>(), ["later"]);
            }
            assert_eq!(serde_json::to_value(&decoded).unwrap(), raw);
        }
    }

    #[test]
    fn unknown_type_is_kept_whole() {
        let raw = json!({"type": "future", "n": 1});
        let decoded: Sample = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(decoded.kind(), "future");
        assert_eq!(serde_json::to_value(&decoded).unwrap(), raw);
    }

    #[test]
    fn missing_type_or_bad_payload_is_an_error() {
        assert!(serde_json::from_value::<Sample>(json!({"id": "x"})).is_err());
        assert!(serde_json::from_value::<Sample>(json!({"type": "full"})).is_err());
    }
}
