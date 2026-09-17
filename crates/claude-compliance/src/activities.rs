//! `GET /v1/compliance/activities`.

use std::pin::Pin;

use async_stream::try_stream;
use claude_api_core::{ApiClient, ApiResponse, Cursor, CursorPage, Error, Result};
use futures_core::Stream;
use jiff::Timestamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::actor::Actor;

const PATH: &str = "v1/compliance/activities";
const MAX_LIMIT: u32 = 5000;

/// Activity `type` values this crate has observed on the wire. The feed has hundreds of types and
/// Anthropic adds more; [`Activity::activity_type`] is a plain string so none are lost.
pub mod activity_types {
    /// A Compliance API request, including the reads of whoever is consuming the feed.
    pub const COMPLIANCE_API_ACCESSED: &str = "compliance_api_accessed";
    /// An Admin API key was created.
    pub const ADMIN_API_KEY_CREATED: &str = "admin_api_key_created";
    /// The organization's Compliance API settings changed.
    pub const ORG_COMPLIANCE_API_SETTINGS_UPDATED: &str = "org_compliance_api_settings_updated";
}

/// A page of activities.
pub type ActivityPage = ApiResponse<CursorPage<Activity>>;

/// The stream returned by [`ListActivities::stream`]. Boxed so it is `Unpin` and works with
/// `TryStreamExt::try_next` directly.
pub type ActivityStream = Pin<Box<dyn Stream<Item = Result<Activity>> + Send + 'static>>;

/// One Activity Feed record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// `activity_…` ID.
    pub id: String,
    /// When the activity occurred.
    pub created_at: Timestamp,
    /// `org_…` ID, or `null` for events not tied to an organization (sign-in, Compliance API calls).
    #[serde(default)]
    pub organization_id: Option<String>,
    /// The organization as a UUID; same scoping as `organization_id`.
    #[serde(default)]
    pub organization_uuid: Option<String>,
    /// Who or what acted.
    pub actor: Actor,
    /// The activity type, for example `claude_chat_created`.
    #[serde(rename = "type")]
    pub activity_type: String,
    /// Type-specific fields. They sit at the top level of the record next to `type` (for example
    /// `scopes` on `admin_api_key_created`); decode them with [`Activity::details_as`].
    #[serde(flatten)]
    pub details: Map<String, Value>,
}

impl Activity {
    /// Decodes the type-specific fields into `T`.
    pub fn details_as<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_value(Value::Object(self.details.clone()))
    }

    /// The request details when this is a `compliance_api_accessed` record.
    pub fn compliance_api_access(&self) -> Option<ComplianceApiAccess> {
        if self.activity_type != activity_types::COMPLIANCE_API_ACCESSED {
            return None;
        }
        self.details_as().ok()
    }
}

/// The fields of a `compliance_api_accessed` activity. *Verified live.*
///
/// Every Compliance API request is recorded this way within about a second, including the
/// consumer's own reads. `request_id` equals the `request-id` response header of that call
/// ([`ResponseMeta::request_id`](claude_api_core::ResponseMeta::request_id)).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComplianceApiAccess {
    /// The request ID.
    pub request_id: Option<String>,
    /// HTTP method.
    pub request_method: Option<String>,
    /// Response status.
    pub status_code: Option<u16>,
    /// Full request URL, query string included.
    pub url: Option<String>,
    /// Request body; `null` for GET.
    pub request_body: Option<Value>,
}

#[derive(Debug, Clone)]
enum Position {
    After(Cursor),
    Before(Cursor),
}

/// Request builder for `GET /v1/compliance/activities`. Results are newest first.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListActivities {
    api: ApiClient,
    limit: Option<u32>,
    position: Option<Position>,
    activity_types: Vec<String>,
    actor_ids: Vec<String>,
    organization_ids: Vec<String>,
    created_at: Vec<(&'static str, Timestamp)>,
}

impl ListActivities {
    pub(crate) fn new(api: ApiClient) -> Self {
        Self {
            api,
            limit: None,
            position: None,
            activity_types: Vec::new(),
            actor_ids: Vec::new(),
            organization_ids: Vec::new(),
            created_at: Vec::new(),
        }
    }

    /// Page size, 1 to 5000 (server default 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Continue after a page's `last_id`: older activities. Replaces [`Self::before`].
    pub fn after(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::After(cursor));
        self
    }

    /// Go back before a page's `first_id`: newer activities. Replaces [`Self::after`].
    pub fn before(mut self, cursor: Cursor) -> Self {
        self.position = Some(Position::Before(cursor));
        self
    }

    /// Adds an `activity_types[]` filter. The server rejects values it does not know with a 400.
    pub fn activity_type(mut self, activity_type: impl Into<String>) -> Self {
        self.activity_types.push(activity_type.into());
        self
    }

    /// Adds an `actor_ids[]` filter (`user_…` IDs).
    pub fn actor_id(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_ids.push(actor_id.into());
        self
    }

    /// Adds an `organization_ids[]` filter (UUID or `org_…` form).
    ///
    /// Observed behaviour: the result still includes activities with no organization (such as
    /// `compliance_api_accessed`), and an organization outside the key's scope is a 403
    /// ([`ApiError::is_organization_out_of_scope`](claude_api_core::ApiError::is_organization_out_of_scope)).
    pub fn organization_id(mut self, organization_id: impl Into<String>) -> Self {
        self.organization_ids.push(organization_id.into());
        self
    }

    /// `created_at.gte`.
    pub fn created_at_gte(self, at: Timestamp) -> Self {
        self.created_at("created_at.gte", at)
    }

    /// `created_at.gt`.
    pub fn created_at_gt(self, at: Timestamp) -> Self {
        self.created_at("created_at.gt", at)
    }

    /// `created_at.lte`.
    pub fn created_at_lte(self, at: Timestamp) -> Self {
        self.created_at("created_at.lte", at)
    }

    /// `created_at.lt`.
    pub fn created_at_lt(self, at: Timestamp) -> Self {
        self.created_at("created_at.lt", at)
    }

    fn created_at(mut self, key: &'static str, at: Timestamp) -> Self {
        self.created_at.retain(|(existing, _)| *existing != key);
        self.created_at.push((key, at));
        self
    }

    /// Fetches one page.
    pub async fn send(&self) -> Result<ActivityPage> {
        self.api.get_json(PATH, &self.query()?).await
    }

    /// Streams every matching activity, following `last_id` into older pages until `has_more` is
    /// false. Starts from [`Self::after`] when set; [`Self::before`] is rejected because the stream
    /// only walks toward older records.
    ///
    /// Persist a cursor only after storing the records it covers: cursors are safe to reuse when a
    /// request fails.
    pub fn stream(self) -> ActivityStream {
        Box::pin(try_stream! {
            if matches!(self.position, Some(Position::Before(_))) {
                Err(Error::InvalidArgument("stream() walks toward older activities; use after(), not before()".into()))?;
            }
            let mut request = self;
            loop {
                let page = request.send().await?.body;
                let next = page.last_id;
                for activity in page.data {
                    yield activity;
                }
                match next {
                    Some(cursor) if page.has_more => request.position = Some(Position::After(cursor)),
                    _ => break,
                }
            }
        })
    }

    fn query(&self) -> Result<Vec<(&'static str, String)>> {
        let mut query = Vec::new();
        if let Some(limit) = self.limit {
            if !(1..=MAX_LIMIT).contains(&limit) {
                return Err(Error::InvalidArgument(format!("limit must be between 1 and {MAX_LIMIT}, got {limit}")));
            }
            query.push(("limit", limit.to_string()));
        }
        match &self.position {
            Some(Position::After(cursor)) => query.push(("after_id", cursor.as_str().to_owned())),
            Some(Position::Before(cursor)) => query.push(("before_id", cursor.as_str().to_owned())),
            None => {}
        }
        query.extend(self.activity_types.iter().map(|value| ("activity_types[]", value.clone())));
        query.extend(self.actor_ids.iter().map(|value| ("actor_ids[]", value.clone())));
        query.extend(self.organization_ids.iter().map(|value| ("organization_ids[]", value.clone())));
        query.extend(self.created_at.iter().map(|(key, at)| (*key, at.to_string())));
        Ok(query)
    }
}
