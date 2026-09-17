//! Tagged unions discriminated by a `type` field.

/// Declares an enum decoded from an object whose `type` field selects the variant.
///
/// Each variant is either a unit variant (the object carries only `type`) or wraps a struct holding
/// the other fields. Unknown `type` values decode to `Other { kind, raw }` with the whole object, and
/// re-encode unchanged. Fields a known variant does not declare are ignored.
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
        #[derive(Debug, Clone, PartialEq, Eq)]
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
                    $( $tag => $crate::union::tagged_union!(@de raw, $name::$variant $(, $inner)?), )+
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
                            $( .or_else(|| $crate::union::tagged_union!(@ser self, $name::$variant $(, $inner)?)) )+
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

pub(crate) use tagged_union;

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Payload {
        pub id: String,
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
    fn known_variants_round_trip_with_their_tag() {
        for raw in [json!({"type": "empty"}), json!({"type": "full", "id": "x"})] {
            let decoded: Sample = serde_json::from_value(raw.clone()).unwrap();
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
