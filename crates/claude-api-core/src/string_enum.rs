//! Open string enums.

/// Declares a string-valued enum that keeps unknown values.
///
/// Anthropic adds values to documented enums (statuses, product surfaces, setting types) and asks
/// integrations to pass unknown ones through. Each generated enum has the listed variants plus
/// `Other(String)`, and round-trips any string.
///
/// ```
/// claude_api_core::string_enum! {
///     /// A session status.
///     pub enum Status {
///         /// Running.
///         Active = "active",
///         /// Done.
///         Archived = "archived",
///     }
/// }
///
/// assert_eq!(serde_json::from_str::<Status>(r#""active""#).unwrap(), Status::Active);
/// assert_eq!(serde_json::from_str::<Status>(r#""paused""#).unwrap(), Status::Other("paused".into()));
/// assert_eq!(serde_json::to_string(&Status::Other("paused".into())).unwrap(), r#""paused""#);
/// ```
#[macro_export]
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident = $value:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $vis enum $name {
            $( $(#[$variant_meta])* $variant, )+
            /// A value this crate version does not know.
            Other(::std::string::String),
        }

        impl $name {
            /// The wire value.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )+
                    Self::Other(value) => value,
                }
            }
        }

        impl ::core::convert::From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $( $value => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                let value = <::std::string::String as $crate::__private::serde::Deserialize>::deserialize(deserializer)?;
                ::core::result::Result::Ok(Self::from(value.as_str()))
            }
        }
    };
}
