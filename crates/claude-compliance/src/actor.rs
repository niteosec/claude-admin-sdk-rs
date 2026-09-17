//! The `actor` union on activities.

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Who or what performed an activity, discriminated by `actor.type`.
///
/// Unknown actor types are kept whole in [`Actor::Other`], as Anthropic asks integrations to pass
/// them through. Fields a variant does not declare are ignored.
#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    /// `user_actor`: a signed-in claude.ai or Claude Console user. *Verified live.*
    User(UserActor),
    /// `api_actor`: a request made with a customer-issued API key, including every Compliance API
    /// call. *Verified live.*
    Api(ApiActor),
    /// `admin_api_key_actor`: an admin managing the organization with an Admin API key. *Documented.*
    AdminApiKey(AdminApiKeyActor),
    /// `unauthenticated_user_actor`: an action before sign-in completed. *Documented.*
    UnauthenticatedUser(UnauthenticatedUserActor),
    /// `anthropic_actor`: Anthropic acting on the organization. *Documented.*
    Anthropic(AnthropicActor),
    /// `scim_directory_sync_actor`: an identity provider pushing a SCIM change. *Documented.*
    ScimDirectorySync(ScimDirectorySyncActor),
    /// Any other `actor.type`, with the raw object.
    Other {
        /// The `type` value.
        actor_type: String,
        /// The whole actor object.
        raw: Value,
    },
}

impl Actor {
    /// The wire `type`.
    pub fn actor_type(&self) -> &str {
        match self {
            Actor::User(_) => "user_actor",
            Actor::Api(_) => "api_actor",
            Actor::AdminApiKey(_) => "admin_api_key_actor",
            Actor::UnauthenticatedUser(_) => "unauthenticated_user_actor",
            Actor::Anthropic(_) => "anthropic_actor",
            Actor::ScimDirectorySync(_) => "scim_directory_sync_actor",
            Actor::Other { actor_type, .. } => actor_type,
        }
    }
}

/// `user_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserActor {
    /// `user_…` ID.
    pub user_id: Option<String>,
    /// Email address.
    pub email_address: Option<String>,
    /// Client IP.
    pub ip_address: Option<String>,
    /// Client user agent.
    pub user_agent: Option<String>,
}

/// `api_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiActor {
    /// `apikey_…` ID. For an Admin API key this is the same identifier as the `admin_api_key_…` ID
    /// in `admin_api_key_created`, under a different prefix.
    pub api_key_id: Option<String>,
    /// Client IP.
    pub ip_address: Option<String>,
    /// Client user agent.
    pub user_agent: Option<String>,
}

/// `admin_api_key_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdminApiKeyActor {
    /// `admin_api_key_…` ID.
    pub admin_api_key_id: Option<String>,
    /// Client IP.
    pub ip_address: Option<String>,
    /// Client user agent.
    pub user_agent: Option<String>,
}

/// `unauthenticated_user_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnauthenticatedUserActor {
    /// The email address entered before sign-in completed.
    pub unauthenticated_email_address: Option<String>,
    /// Client IP.
    pub ip_address: Option<String>,
    /// Client user agent.
    pub user_agent: Option<String>,
}

/// `anthropic_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnthropicActor {
    /// Always `null`; present for shape consistency with `user_actor`.
    pub email_address: Option<String>,
}

/// `scim_directory_sync_actor`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScimDirectorySyncActor {
    /// WorkOS event ID.
    pub workos_event_id: Option<String>,
    /// Directory ID.
    pub directory_id: Option<String>,
    /// For example `OktaSCIMV2` or `AzureSCIMV2`.
    pub idp_connection_type: Option<String>,
}

impl<'de> Deserialize<'de> for Actor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let actor_type =
            raw.get("type").and_then(Value::as_str).ok_or_else(|| D::Error::missing_field("type"))?.to_owned();
        let actor = match actor_type.as_str() {
            "user_actor" => serde_json::from_value(raw).map(Actor::User),
            "api_actor" => serde_json::from_value(raw).map(Actor::Api),
            "admin_api_key_actor" => serde_json::from_value(raw).map(Actor::AdminApiKey),
            "unauthenticated_user_actor" => serde_json::from_value(raw).map(Actor::UnauthenticatedUser),
            "anthropic_actor" => serde_json::from_value(raw).map(Actor::Anthropic),
            "scim_directory_sync_actor" => serde_json::from_value(raw).map(Actor::ScimDirectorySync),
            _ => Ok(Actor::Other { actor_type, raw }),
        };
        actor.map_err(D::Error::custom)
    }
}

impl Serialize for Actor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        fn tagged<T: Serialize>(tag: &str, inner: &T) -> serde_json::Result<Value> {
            let mut value = serde_json::to_value(inner)?;
            if let Value::Object(map) = &mut value {
                map.insert("type".to_owned(), Value::String(tag.to_owned()));
            }
            Ok(value)
        }
        let tag = self.actor_type();
        let value = match self {
            Actor::User(inner) => tagged(tag, inner),
            Actor::Api(inner) => tagged(tag, inner),
            Actor::AdminApiKey(inner) => tagged(tag, inner),
            Actor::UnauthenticatedUser(inner) => tagged(tag, inner),
            Actor::Anthropic(inner) => tagged(tag, inner),
            Actor::ScimDirectorySync(inner) => tagged(tag, inner),
            Actor::Other { raw, .. } => Ok(raw.clone()),
        }
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_actor_type_is_kept_whole() {
        let raw = json!({"type": "robot_actor", "serial": 7});
        let actor: Actor = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(actor, Actor::Other { actor_type: "robot_actor".into(), raw: raw.clone() });
        assert_eq!(serde_json::to_value(&actor).unwrap(), raw);
    }

    #[test]
    fn known_actor_round_trips_with_its_tag() {
        let raw =
            json!({"type": "api_actor", "api_key_id": "apikey_x", "ip_address": "192.0.2.1", "user_agent": "curl"});
        let actor: Actor = serde_json::from_value(raw.clone()).unwrap();
        assert!(matches!(&actor, Actor::Api(api) if api.api_key_id.as_deref() == Some("apikey_x")));
        assert_eq!(serde_json::to_value(&actor).unwrap(), raw);
    }

    #[test]
    fn missing_type_is_an_error() {
        assert!(serde_json::from_value::<Actor>(json!({"user_id": "user_x"})).is_err());
    }
}
