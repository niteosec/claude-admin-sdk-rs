//! Plumbing shared by the agent configuration modules: page-token list requests, tagged unions and
//! secret-safe `Debug` output.

use std::fmt;
use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiPath, ApiResponse, Error, PageToken, RequestOptions, Result, TokenPage};
use futures_core::Stream;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::ManagedAgentsClient;

/// A boxed stream of records. `Unpin`, so `TryStreamExt::try_next` works on it directly.
pub(crate) type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send + 'static>>;

/// A list envelope that yields its records and the token of the next page.
pub(crate) trait Paged<T> {
    fn into_parts(self) -> (Vec<T>, Option<PageToken>);
}

impl<T> Paged<T> for TokenPage<T> {
    fn into_parts(self) -> (Vec<T>, Option<PageToken>) {
        let next = self.next().cloned();
        (self.data, next)
    }
}

/// A `next_page` list (`limit`, `page` and endpoint filters) sent with the client's headers.
#[derive(Debug, Clone)]
pub(crate) struct TokenList {
    api: ApiClient,
    options: RequestOptions,
    path: Result<ApiPath, String>,
    max_limit: Option<u32>,
    limit: Option<u32>,
    page: Option<PageToken>,
    params: Vec<(&'static str, String)>,
}

impl TokenList {
    /// A list of `path`. `max_limit` is the documented page-size maximum, when there is one.
    pub(crate) fn new(client: &ManagedAgentsClient, path: Result<ApiPath>, max_limit: Option<u32>) -> Self {
        Self {
            api: client.api.clone(),
            options: client.options(),
            path: path.map_err(|error| match error {
                Error::InvalidArgument(message) => message,
                other => other.to_string(),
            }),
            max_limit,
            limit: None,
            page: None,
            params: Vec::new(),
        }
    }

    /// Adds an `anthropic-beta` value on top of the Managed Agents beta.
    pub(crate) fn beta(mut self, beta: &'static str) -> Self {
        self.options = self.options.beta(beta);
        self
    }

    pub(crate) fn limit(&mut self, limit: u32) {
        self.limit = Some(limit);
    }

    pub(crate) fn page(&mut self, page: PageToken) {
        self.page = Some(page);
    }

    /// Sets a single-valued parameter, replacing an earlier value.
    pub(crate) fn set(&mut self, key: &'static str, value: impl Into<String>) {
        self.params.retain(|(existing, _)| *existing != key);
        self.params.push((key, value.into()));
    }

    pub(crate) async fn send<P: DeserializeOwned>(&self) -> Result<ApiResponse<P>> {
        let path = self.path.clone().map_err(Error::InvalidArgument)?;
        let query = self.query()?;
        self.api.get_json_with(&path, &query, &self.options).await
    }

    pub(crate) fn stream<T, P>(self) -> BoxStream<T>
    where
        T: Send + 'static,
        P: DeserializeOwned + Paged<T> + Send + 'static,
    {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let (records, next) = request.send::<P>().await?.body.into_parts();
                for record in records {
                    yield record;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            let max = self.max_limit.unwrap_or(i32::MAX as u32);
            if !(1..=max).contains(&limit) {
                return Err(Error::InvalidArgument(format!("limit must be between 1 and {max}, got {limit}")));
            }
            query.push(("limit", limit.to_string()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        query.extend(self.params.iter().cloned());
        Ok(query)
    }
}

/// Declares the `limit`, `page`, `send` and `stream` methods of a list builder whose `inner` field
/// is a [`TokenList`] and whose envelope is `$page`.
macro_rules! token_list_methods {
    ($record:ty, $page:ty, $stream:ty, $limit_doc:literal) => {
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
        pub async fn send(&self) -> claude_api_core::Result<claude_api_core::ApiResponse<$page>> {
            self.inner.send().await
        }

        /// Streams every record, following `next_page` until it is `null`. Starts from
        /// [`Self::page`] when set.
        pub fn stream(self) -> $stream {
            self.inner.stream::<$record, $page>()
        }
    };
}

/// Declares an enum decoded from an object whose `type` field selects the variant.
///
/// Each variant is either a unit variant (the object carries only `type`) or wraps a struct holding
/// the other fields; `type` is removed before the struct is decoded and put back on encode. Unknown
/// `type` values decode to `Other { kind, raw }` with the whole object and re-encode unchanged.
/// `Debug` masks secret-named fields inside `raw` (see [`RedactedValue`]).
macro_rules! config_union {
    (@de $raw:ident, $name:ident :: $variant:ident) => {
        Ok($name::$variant)
    };
    (@de $raw:ident, $name:ident :: $variant:ident, $inner:ty) => {
        serde_json::from_value::<$inner>($crate::config_support::without_type($raw)).map($name::$variant)
    };
    (@ser $this:expr, $name:ident :: $variant:ident) => {
        matches!($this, $name::$variant).then(|| Ok(serde_json::Value::Object(serde_json::Map::new())))
    };
    (@ser $this:expr, $name:ident :: $variant:ident, $inner:ty) => {
        if let $name::$variant(inner) = $this { Some(serde_json::to_value(inner)) } else { None }
    };
    (@dbg $this:expr, $f:ident, $name:ident :: $variant:ident) => {
        if let $name::$variant = $this {
            return $f.write_str(stringify!($variant));
        }
    };
    (@dbg $this:expr, $f:ident, $name:ident :: $variant:ident, $inner:ty) => {
        if let $name::$variant(inner) = $this {
            return $f.debug_tuple(stringify!($variant)).field(inner).finish();
        }
    };
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident $( ($inner:ty) )? = $tag:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq)]
        #[allow(clippy::large_enum_variant)]
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

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                if let Self::Other { kind, raw } = self {
                    return f
                        .debug_struct("Other")
                        .field("kind", kind)
                        .field("raw", &$crate::config_support::RedactedValue(raw))
                        .finish();
                }
                $( $crate::config_support::config_union!(@dbg self, f, $name::$variant $(, $inner)?); )+
                f.write_str(self.kind())
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
                    $( $tag => $crate::config_support::config_union!(@de raw, $name::$variant $(, $inner)?), )+
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
                            $( .or_else(|| $crate::config_support::config_union!(@ser self, $name::$variant $(, $inner)?)) )+
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

pub(crate) use {config_union, token_list_methods};

/// The object without its `type` discriminator, so a variant struct's `extra` does not capture it.
pub(crate) fn without_type(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.remove("type");
    }
    value
}

/// Field names the Managed Agents documentation calls secret: the write-only credential values
/// (`token`, `access_token`, `refresh_token`, `client_secret`, `secret_value`) and the tunnel
/// connector token. None is documented in a GET response; if one ever appears in an unknown field,
/// `Debug` output masks it.
const SECRET_FIELDS: &[&str] =
    &["token", "access_token", "refresh_token", "client_secret", "secret_value", "tunnel_token"];

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(redact_map(map)),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

fn redact_map(map: &Map<String, Value>) -> Map<String, Value> {
    map.iter()
        .map(|(key, value)| {
            let value = if SECRET_FIELDS.contains(&key.as_str()) {
                Value::String("[redacted]".to_owned())
            } else {
                redact(value)
            };
            (key.clone(), value)
        })
        .collect()
}

/// `Debug` for a raw JSON value with secret-named fields masked at any depth.
pub(crate) struct RedactedValue<'a>(pub(crate) &'a Value);

impl fmt::Debug for RedactedValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&redact(self.0), f)
    }
}

/// `Debug` for an `extra` map with secret-named fields masked at any depth.
pub(crate) struct RedactedMap<'a>(pub(crate) &'a Map<String, Value>);

impl fmt::Debug for RedactedMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&redact_map(self.0), f)
    }
}

/// Validates a documented `int32` value with a lower bound.
pub(crate) fn check_int32(name: &str, value: u32, min: u32) -> Result<()> {
    if value < min || value > i32::MAX as u32 {
        return Err(Error::InvalidArgument(format!("{name} must be between {min} and {}, got {value}", i32::MAX)));
    }
    Ok(())
}
