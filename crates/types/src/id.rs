//! Strict UUIDv7 identifiers for accounts, spaces and installations.
//!
//! Each kind is its own type, so an account ID can never be passed where a
//! space ID is expected. Text forms are the canonical 36-character lowercase
//! hyphenated UUID; anything else (other versions, upper case, braces, simple
//! form) is refused, so one ID has exactly one spelling in files, API and URLs.
use crate::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use uuid::{Uuid, Variant};

macro_rules! identifier {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Possessing an ID grants nothing: callers check current authority
        /// before acting on it.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);

        impl $name {
            /// Allocates a fresh UUIDv7. Cost: one clock read and random draw; no I/O.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
            /// Accepts an existing UUID only if it is an RFC 4122 version 7 UUID.
            ///
            /// # Errors
            /// Invalid input for any other version or variant. Cost: O(1), no allocation.
            pub fn from_uuid(value: Uuid) -> Result<Self, Error> {
                if value.get_version_num() == 7 && value.get_variant() == Variant::RFC4122 {
                    Ok(Self(value))
                } else {
                    Err(invalid())
                }
            }
            /// Parses the canonical lowercase hyphenated text form.
            ///
            /// # Errors
            /// Invalid input for any other spelling or a non-v7 UUID.
            /// Cost: O(36) with no allocation on success.
            pub fn parse(text: &str) -> Result<Self, Error> {
                if text.len() != 36 || text.bytes().any(|b| b.is_ascii_uppercase()) {
                    return Err(invalid());
                }
                Self::from_uuid(Uuid::parse_str(text).map_err(|_| invalid())?)
            }
            /// The underlying UUID. Cost: O(1) copy.
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.hyphenated().fmt(f)
            }
        }
        impl FromStr for $name {
            type Err = Error;
            fn from_str(text: &str) -> Result<Self, Error> {
                Self::parse(text)
            }
        }
        impl From<$name> for Uuid {
            fn from(value: $name) -> Uuid {
                value.0
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(&self.0.hyphenated())
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = <std::borrow::Cow<'de, str>>::deserialize(d)?;
                Self::parse(&text).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn invalid() -> Error {
    Error::invalid_input("identifier must be a lowercase UUIDv7")
}

identifier!(AccountId, "A person's account; never interchangeable with a space ID.");
identifier!(
    SpaceId,
    "A space, allocated independently of every account ID; it selects the space's storage."
);
identifier!(
    InstallationId,
    "One signed-in client installation, owned by an account."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_canonical_v7_spelling_parses() {
        let id = SpaceId::new();
        let text = id.to_string();
        assert_eq!(SpaceId::parse(&text).unwrap(), id);
        assert!(SpaceId::parse(&text.to_uppercase()).is_err());
        assert!(SpaceId::parse(&id.as_uuid().simple().to_string()).is_err());
        assert!(SpaceId::parse(&Uuid::new_v4().to_string()).is_err());
        assert!(AccountId::from_uuid(Uuid::nil()).is_err());
    }

    #[test]
    fn serde_uses_the_text_form() {
        let id = AccountId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<AccountId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<AccountId>("\"personal\"").is_err());
    }
}
