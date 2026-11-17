//! Who did something: the one attribution every store records the same way.
use crate::AccountId;
use std::fmt;
use uuid::Uuid;

/// The acting account, and how it acted when not directly.
///
/// Authority always comes from `account`: `via` only says which route acted on its
/// behalf, and never adds access. Secrets audit, turn records and (later) content
/// journals all record this type, so they read alike: "<account>", "an agent for
/// <account>, in thread <id>", "plugin <id> for <account>".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Actor {
    /// The account on whose authority the action ran.
    pub account: AccountId,
    /// The route that acted for the account; `None` when the person acted
    /// directly (a signed-in session).
    pub via: Option<Via>,
}

/// A route acting on an account's behalf.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Via {
    /// An agent acting for the account in one thread.
    Agent {
        /// The thread the agent runs in.
        thread: Uuid,
    },
    /// A plugin invocation acting for the account.
    Plugin {
        /// The plugin's manifest ID.
        plugin: String,
    },
}

impl Actor {
    /// The account acting directly. Cost: a copy.
    pub fn account(account: AccountId) -> Self {
        Self { account, via: None }
    }
    /// An agent acting for `account` in `thread`. Cost: a copy.
    pub fn agent(account: AccountId, thread: Uuid) -> Self {
        Self {
            account,
            via: Some(Via::Agent { thread }),
        }
    }
    /// A plugin acting for `account`. Cost: one string allocation.
    pub fn plugin(account: AccountId, plugin: impl Into<String>) -> Self {
        Self {
            account,
            via: Some(Via::Plugin {
                plugin: plugin.into(),
            }),
        }
    }
}

/// The attribution in words, as stores record it: `<account>`,
/// `an agent for <account>, in thread <id>`, or `plugin <id> for <account>`.
impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.via {
            None => write!(f, "{}", self.account),
            Some(Via::Agent { thread }) => {
                write!(
                    f,
                    "an agent for {}, in thread {}",
                    self.account,
                    thread.hyphenated()
                )
            }
            Some(Via::Plugin { plugin }) => write!(f, "plugin {plugin} for {}", self.account),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_reads_the_same_everywhere() {
        let account = AccountId::new();
        let thread = Uuid::now_v7();
        assert_eq!(Actor::account(account).to_string(), account.to_string());
        assert_eq!(
            Actor::agent(account, thread).to_string(),
            format!("an agent for {account}, in thread {thread}")
        );
        assert_eq!(
            Actor::plugin(account, "mail").to_string(),
            format!("plugin mail for {account}")
        );
    }
}
