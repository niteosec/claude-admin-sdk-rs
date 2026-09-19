//! User profiles:
//!
//! - `GET /v1/user_profiles`
//! - `GET /v1/user_profiles/{user_profile_id}`
//!
//! *Built from Anthropic's API reference (fetched 2026-09-19); not yet verified against a live
//! workspace.*
//!
//! A user profile represents an end user (or resold-to company) a platform acts for. These requests
//! add the [`USER_PROFILES_BETA`] header the reference examples send, on top of the Managed Agents
//! beta. Under that beta the reference says `external_id` is present; under the later
//! `user-profiles-2026-09-04` beta its value moves to `external_user_details.reference_id`.

use std::collections::BTreeMap;
use std::pin::Pin;

use claude_api_core::{ApiPath, ApiResponse, Result, TokenPage};
use futures_core::Stream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config_support::{TokenList, token_list_methods};

const PATH: &str = "v1/user_profiles";

/// The `anthropic-beta` value the user profile reference examples send.
pub const USER_PROFILES_BETA: &str = "user-profiles-2026-08-18";

/// The stream returned by [`ListUserProfiles::stream`].
pub type UserProfileStream = Pin<Box<dyn Stream<Item = Result<UserProfile>> + Send + 'static>>;

/// An end user or resold-to company a platform uses the API on behalf of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfile {
    /// Always `user_profile`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// `uprof_…` ID.
    pub id: String,
    /// When the profile was created.
    pub created_at: Timestamp,
    /// Caller-defined key-value metadata (at most 16 pairs).
    pub metadata: BTreeMap<String, String>,
    /// Anthropic's trust grants, keyed by grant name. A key is absent when no grant is active or in
    /// flight.
    pub trust_grants: BTreeMap<String, UserProfileTrustGrant>,
    /// When the profile was last updated.
    pub updated_at: Timestamp,
    /// How the platform uses the API for this entity.
    #[serde(default)]
    pub access_type: Option<UserProfileAccessType>,
    /// The platform's own identifier for the user; not enforced unique.
    #[serde(default)]
    pub external_id: Option<String>,
    /// What the platform states about the entity; not verified by Anthropic.
    #[serde(default)]
    pub external_user_details: Option<UserProfileExternalUserDetails>,
    /// When the platform onboarded the user.
    #[serde(default)]
    pub external_user_onboarded_at: Option<Timestamp>,
    /// Real-world name of the entity (company or individual).
    #[serde(default)]
    pub name: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A trust grant on a user profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileTrustGrant {
    /// Grant status.
    pub status: UserProfileTrustGrantStatus,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// A trust grant status.
    pub enum UserProfileTrustGrantStatus {
        /// `active`
        Active = "active",
        /// `pending`
        Pending = "pending",
        /// `rejected`
        Rejected = "rejected",
    }
}

claude_api_core::string_enum! {
    /// How a platform uses the API for a profile's entity.
    pub enum UserProfileAccessType {
        /// `application`: an end user of a product that uses the API behind the scenes.
        Application = "application",
        /// `passthrough`: a company the platform resells raw inference to.
        Passthrough = "passthrough",
    }
}

/// What the platform states about a profile's entity. Every field is `null` until supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileExternalUserDetails {
    /// The account status on the platform (the platform's decision, independent of trust grants).
    #[serde(default)]
    pub account_status: Option<UserProfileAccountStatus>,
    /// ISO 3166-1 alpha-2 country code.
    #[serde(default)]
    pub country: Option<String>,
    /// Platform-computed hash of the email address.
    #[serde(default)]
    pub email_hash: Option<String>,
    /// Kind of entity.
    #[serde(default)]
    pub entity_type: Option<UserProfileEntityType>,
    /// Platform-computed hash of the name.
    #[serde(default)]
    pub name_hash: Option<String>,
    /// When the platform onboarded the entity.
    #[serde(default)]
    pub onboarded_at: Option<Timestamp>,
    /// The platform's own reference for the entity.
    #[serde(default)]
    pub reference_id: Option<String>,
    /// Fields not in the documented schema.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

claude_api_core::string_enum! {
    /// An entity's account status on the platform.
    pub enum UserProfileAccountStatus {
        /// `active`
        Active = "active",
        /// `suspended`: restricted, may be restored.
        Suspended = "suspended",
        /// `blocked`: barred.
        Blocked = "blocked",
    }
}

claude_api_core::string_enum! {
    /// The kind of entity a profile represents.
    pub enum UserProfileEntityType {
        /// `individual`
        Individual = "individual",
        /// `business`
        Business = "business",
        /// `non_profit`
        NonProfit = "non_profit",
        /// `government`
        Government = "government",
    }
}

/// Sort direction for [`ListUserProfiles::order`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserProfileOrder {
    /// `asc`
    Asc,
    /// `desc`
    Desc,
}

impl UserProfileOrder {
    /// The wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

/// Sort key for [`ListUserProfiles::order_by`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserProfileOrderBy {
    /// `created_at`
    CreatedAt,
    /// `name`
    Name,
}

impl UserProfileOrderBy {
    /// The wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::Name => "name",
        }
    }
}

/// Request builder for `GET /v1/user_profiles`.
#[derive(Debug, Clone)]
#[must_use = "a request does nothing until sent or streamed"]
pub struct ListUserProfiles {
    inner: TokenList,
}

impl ListUserProfiles {
    /// `order`: sort direction.
    pub fn order(mut self, order: UserProfileOrder) -> Self {
        self.inner.set("order", order.as_str());
        self
    }

    /// `order_by`: sort key.
    pub fn order_by(mut self, order_by: UserProfileOrderBy) -> Self {
        self.inner.set("order_by", order_by.as_str());
        self
    }

    token_list_methods!(
        UserProfile,
        TokenPage<UserProfile>,
        UserProfileStream,
        "Page size (at least 1; the reference documents no maximum or default)."
    );
}

impl crate::ManagedAgentsClient {
    /// `GET /v1/user_profiles`. Adds the [`USER_PROFILES_BETA`] header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/user_profiles/list)
    pub fn user_profiles(&self) -> ListUserProfiles {
        ListUserProfiles { inner: TokenList::new(self, Ok(ApiPath::new(PATH)), None).beta(USER_PROFILES_BETA) }
    }

    /// `GET /v1/user_profiles/{user_profile_id}`. Adds the [`USER_PROFILES_BETA`] header.
    ///
    /// [Reference](https://platform.claude.com/docs/en/api/beta/user_profiles/retrieve)
    pub async fn user_profile(&self, user_profile_id: &str) -> Result<ApiResponse<UserProfile>> {
        let path = ApiPath::new(PATH).id(user_profile_id)?;
        self.api.get_json_with(&path, &[], &self.options().beta(USER_PROFILES_BETA)).await
    }
}
