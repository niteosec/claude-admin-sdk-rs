//! Request plumbing shared by the list and report builders.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{
    ApiClient, ApiPath, ApiResponse, Cursor, CursorPage, Error, PageToken, RequestOptions, Result, TokenPage,
};
use futures_core::Stream;
use serde::de::DeserializeOwned;

/// A boxed stream of records. `Unpin`, so `TryStreamExt::try_next` works on it directly.
pub type RecordStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send + 'static>>;

type Query = Vec<(&'static str, String)>;

/// A path whose identifiers may have been rejected; the error surfaces when the request is sent.
fn deferred(path: Result<ApiPath>) -> Result<ApiPath, String> {
    path.map_err(|error| match error {
        Error::InvalidArgument(message) => message,
        other => other.to_string(),
    })
}

fn check_limit(limit: Option<u32>, max: u32) -> Result<()> {
    match limit {
        Some(limit) if !(1..=max).contains(&limit) => {
            Err(Error::InvalidArgument(format!("limit must be between 1 and {max}, got {limit}")))
        }
        _ => Ok(()),
    }
}

/// Query parameters in insertion order, with replace-or-append semantics.
#[derive(Debug, Clone, Default)]
pub(crate) struct Params {
    params: Query,
    invalid: Option<String>,
}

impl Params {
    /// Sets a single-valued parameter, replacing an earlier value.
    pub(crate) fn set(&mut self, key: &'static str, value: impl Into<String>) {
        self.params.retain(|(existing, _)| *existing != key);
        self.params.push((key, value.into()));
    }

    /// Appends one value of a repeatable parameter.
    pub(crate) fn push(&mut self, key: &'static str, value: impl Into<String>) {
        self.params.push((key, value.into()));
    }

    /// Appends one value of a repeatable parameter with a documented maximum item count.
    pub(crate) fn push_bounded(&mut self, key: &'static str, value: impl Into<String>, max_items: usize) {
        self.push(key, value);
        let count = self.params.iter().filter(|(existing, _)| *existing == key).count();
        if count > max_items {
            self.reject(format!("{key} accepts at most {max_items} values"));
        }
    }

    /// Records an argument error, reported when the request is sent.
    pub(crate) fn reject(&mut self, message: String) {
        self.invalid.get_or_insert(message);
    }

    fn extend_into(&self, query: &mut Query) -> Result<()> {
        if let Some(message) = &self.invalid {
            return Err(Error::InvalidArgument(message.clone()));
        }
        query.extend(self.params.iter().cloned());
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum Position {
    After(Cursor),
    Before(Cursor),
}

/// A `first_id` / `last_id` list (`after_id`, `before_id`, `limit`).
#[derive(Debug, Clone)]
pub(crate) struct CursorList {
    api: ApiClient,
    path: Result<ApiPath, String>,
    max_limit: u32,
    limit: Option<u32>,
    position: Option<Position>,
    pub(crate) params: Params,
}

impl CursorList {
    pub(crate) fn new(api: ApiClient, path: Result<ApiPath>, max_limit: u32) -> Self {
        Self { api, path: deferred(path), max_limit, limit: None, position: None, params: Params::default() }
    }

    pub(crate) fn limit(&mut self, limit: u32) {
        self.limit = Some(limit);
    }

    pub(crate) fn after(&mut self, cursor: Cursor) {
        self.position = Some(Position::After(cursor));
    }

    pub(crate) fn before(&mut self, cursor: Cursor) {
        self.position = Some(Position::Before(cursor));
    }

    pub(crate) async fn send<T: DeserializeOwned>(&self) -> Result<ApiResponse<CursorPage<T>>> {
        let path = self.path.clone().map_err(Error::InvalidArgument)?;
        self.api.get_json(&path, &self.query()?).await
    }

    pub(crate) fn stream<T: DeserializeOwned + Send + 'static>(self) -> RecordStream<T> {
        Box::pin(try_stream! {
            if matches!(self.position, Some(Position::Before(_))) {
                Err(Error::InvalidArgument("stream() walks forward from last_id; use after(), not before()".into()))?;
            }
            let mut request = self;
            loop {
                let page = request.send::<T>().await?.body;
                let next = page.last_id;
                for record in page.data {
                    yield record;
                }
                match next {
                    Some(cursor) if page.has_more => request.position = Some(Position::After(cursor)),
                    _ => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Query> {
        check_limit(self.limit, self.max_limit)?;
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        match &self.position {
            Some(Position::After(cursor)) => query.push(("after_id", cursor.as_str().to_owned())),
            Some(Position::Before(cursor)) => query.push(("before_id", cursor.as_str().to_owned())),
            None => {}
        }
        self.params.extend_into(&mut query)?;
        Ok(query)
    }
}

/// A `next_page` list (`page`, `limit`).
#[derive(Debug, Clone)]
pub(crate) struct TokenList {
    api: ApiClient,
    path: Result<ApiPath, String>,
    pub(crate) max_limit: u32,
    limit: Option<u32>,
    page: Option<PageToken>,
    pub(crate) params: Params,
    pub(crate) options: RequestOptions,
}

impl TokenList {
    pub(crate) fn new(api: ApiClient, path: Result<ApiPath>, max_limit: u32) -> Self {
        Self {
            api,
            path: deferred(path),
            max_limit,
            limit: None,
            page: None,
            params: Params::default(),
            options: RequestOptions::default(),
        }
    }

    pub(crate) fn limit(&mut self, limit: u32) {
        self.limit = Some(limit);
    }

    pub(crate) fn page(&mut self, page: PageToken) {
        self.page = Some(page);
    }

    pub(crate) async fn send<T: DeserializeOwned>(&self) -> Result<ApiResponse<TokenPage<T>>> {
        let path = self.path.clone().map_err(Error::InvalidArgument)?;
        self.api.get_json_with(&path, &self.query()?, &self.options).await
    }

    pub(crate) fn stream<T: DeserializeOwned + Send + 'static>(self) -> RecordStream<T> {
        Box::pin(try_stream! {
            let mut request = self;
            loop {
                let page = request.send::<T>().await?.body;
                let next = page.next().cloned();
                for record in page.data {
                    yield record;
                }
                match next {
                    Some(token) => request.page = Some(token),
                    None => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Query> {
        check_limit(self.limit, self.max_limit)?;
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(page) = &self.page {
            query.push(("page", page.as_str().to_owned()));
        }
        self.params.extend_into(&mut query)?;
        Ok(query)
    }
}

/// Declares the `limit`, cursor, `send` and `stream` methods of a cursor-list builder whose `inner`
/// field is a [`CursorList`].
macro_rules! cursor_list_methods {
    ($record:ty, $max:literal) => {
        #[doc = concat!("Page size, 1 to ", stringify!($max), " (server default 20).")]
        pub fn limit(mut self, limit: u32) -> Self {
            self.inner.limit(limit);
            self
        }

        /// Continue after a page's `last_id`. Replaces [`Self::before`].
        pub fn after(mut self, cursor: claude_api_core::Cursor) -> Self {
            self.inner.after(cursor);
            self
        }

        /// Go back before a page's `first_id`. Replaces [`Self::after`].
        pub fn before(mut self, cursor: claude_api_core::Cursor) -> Self {
            self.inner.before(cursor);
            self
        }

        /// Fetches one page.
        pub async fn send(
            &self,
        ) -> claude_api_core::Result<claude_api_core::ApiResponse<claude_api_core::CursorPage<$record>>> {
            self.inner.send().await
        }

        /// Streams every matching record, following `last_id` until `has_more` is false. Starts from
        /// [`Self::after`] when set; [`Self::before`] is rejected.
        pub fn stream(self) -> $crate::RecordStream<$record> {
            self.inner.stream()
        }
    };
}

/// Declares the `limit`, `page`, `send` and `stream` methods of a page-token builder whose `inner`
/// field is a [`TokenList`].
macro_rules! token_list_methods {
    ($record:ty, $limit_doc:literal) => {
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
        ) -> claude_api_core::Result<claude_api_core::ApiResponse<claude_api_core::TokenPage<$record>>> {
            self.inner.send().await
        }

        /// Streams every record, following `next_page` until the walk ends (`has_more: false` or a
        /// `null` token). Starts from [`Self::page`] when set.
        pub fn stream(self) -> $crate::RecordStream<$record> {
            self.inner.stream()
        }
    };
}

pub(crate) use {cursor_list_methods, token_list_methods};
