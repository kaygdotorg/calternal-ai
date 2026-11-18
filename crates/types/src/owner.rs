use crate::{AccountId, SpaceId};

/// The account or space that owns a storage scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// Personal storage and authority belonging to one account.
    Account(AccountId),
    /// Shared by a space; authority comes from the space's capability sets.
    Space(SpaceId),
}

impl Owner {
    /// Storage kind word, also bound into every ciphertext.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Account(_) => "account",
            Self::Space(_) => "space",
        }
    }
    /// Directory grouping for this owner in the storage roots.
    pub fn directory(&self) -> &'static str {
        match self {
            Self::Account(_) => "accounts",
            Self::Space(_) => "spaces",
        }
    }

    /// The owner's UUID.
    pub fn uuid(&self) -> uuid::Uuid {
        match self {
            Self::Account(id) => id.as_uuid(),
            Self::Space(id) => id.as_uuid(),
        }
    }
}
