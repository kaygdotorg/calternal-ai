//! Distinct canonical UUIDv7 account and space identities at the worker boundary.
use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, Variant};

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Uuid);
        impl $name {
            /// Parse a full lowercase RFC 4122 UUIDv7, refusing aliases and other versions.
            /// Cost: O(36) with no allocation on success, no I/O or locks.
            pub fn parse(text: &str) -> Result<Self, Error> {
                let uuid = Uuid::parse_str(text).map_err(|_| Error::Invalid)?;
                if text.len() != 36
                    || text.bytes().any(|b| b.is_ascii_uppercase())
                    || uuid.get_version_num() != 7
                    || uuid.get_variant() != Variant::RFC4122
                {
                    return Err(Error::Invalid);
                }
                Ok(Self(uuid))
            }
            /// Borrow the validated UUID for protected host adapter lookups.
            /// Cost: O(1), no allocation, I/O or locks.
            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(value: String) -> Result<Self, Error> {
                Self::parse(&value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0.to_string()
            }
        }
    };
}
identity!(
    AccountId,
    "An account UUIDv7; never interchangeable with a space identifier."
);
identity!(
    SpaceId,
    "A space UUIDv7 allocated independently of every account identifier."
);
