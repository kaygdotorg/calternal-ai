//! Cross-socket quotas scoped to the receiving account.

use super::MAX_SUBSCRIPTIONS;
use crate::internal;
use calternal_types::{AccountId, Error, ErrorCode, Result};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const CONNECTIONS: usize = 4;
const REFRESH_PER_SECOND: usize = 48;
const TRACKED_ACCOUNTS: usize = 512;
const WINDOW: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct Accounts(Mutex<HashMap<AccountId, Arc<Mutex<Usage>>>>);

struct Usage {
    connections: usize,
    subscriptions: usize,
    refreshes: usize,
    window: Instant,
}

impl Accounts {
    pub(super) fn connect(&self, account: AccountId) -> Result<Connection> {
        let mut accounts = self.0.lock().map_err(internal)?;
        let usage = if let Some(usage) = accounts.get(&account).cloned() {
            usage
        } else {
            accounts.retain(|_, usage| {
                usage
                    .lock()
                    .is_ok_and(|usage| usage.connections != 0 || usage.window.elapsed() < WINDOW)
            });
            if accounts.len() == TRACKED_ACCOUNTS {
                return Err(Error::new(
                    ErrorCode::RetryAfter,
                    "Too many active live accounts.",
                ));
            }
            let usage = Arc::new(Mutex::new(Usage {
                connections: 0,
                subscriptions: 0,
                refreshes: 0,
                window: Instant::now(),
            }));
            accounts.insert(account, usage.clone());
            usage
        };
        {
            let mut current = usage.lock().map_err(internal)?;
            if current.connections >= CONNECTIONS {
                return Err(Error::new(
                    ErrorCode::RetryAfter,
                    "Too many live connections.",
                ));
            }
            current.connections += 1;
        }
        Ok(Connection {
            usage,
            subscriptions: 0,
        })
    }
}

pub(super) struct Connection {
    usage: Arc<Mutex<Usage>>,
    subscriptions: usize,
}

impl Connection {
    pub(super) fn subscribe(&mut self) -> Result<()> {
        let mut usage = self.usage.lock().map_err(internal)?;
        if usage.subscriptions >= MAX_SUBSCRIPTIONS {
            return Err(Error::new(
                ErrorCode::EffectiveScopeLimitExceeded,
                "Too many live subscriptions for this account.",
            ));
        }
        usage.subscriptions += 1;
        self.subscriptions += 1;
        Ok(())
    }

    pub(super) fn unsubscribe(&mut self) {
        if let Ok(mut usage) = self.usage.lock() {
            usage.subscriptions -= 1;
            self.subscriptions -= 1;
        }
    }

    pub(super) fn refresh(&self, count: usize) -> bool {
        let Ok(mut usage) = self.usage.lock() else {
            return false;
        };
        if usage.window.elapsed() >= WINDOW {
            usage.window = Instant::now();
            usage.refreshes = 0;
        }
        if count > REFRESH_PER_SECOND - usage.refreshes {
            return false;
        }
        usage.refreshes += count;
        true
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut usage) = self.usage.lock() {
            usage.connections -= 1;
            usage.subscriptions -= self.subscriptions;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_limits_share_capacity_and_survive_reconnect() {
        let accounts = Accounts::default();
        let account = AccountId::new();
        let mut first = accounts.connect(account).unwrap();
        let mut rest: Vec<_> = (1..CONNECTIONS)
            .map(|_| accounts.connect(account).unwrap())
            .collect();
        assert_eq!(
            accounts.connect(account).err().unwrap().code,
            ErrorCode::RetryAfter
        );
        for _ in 0..MAX_SUBSCRIPTIONS {
            first.subscribe().unwrap();
        }
        assert_eq!(
            rest[0].subscribe().unwrap_err().code,
            ErrorCode::EffectiveScopeLimitExceeded
        );
        assert!(rest[0].refresh(REFRESH_PER_SECOND));
        assert!(!first.refresh(1));
        drop(first);
        drop(rest);
        let mut reconnected = accounts.connect(account).unwrap();
        assert!(!reconnected.refresh(1));
        reconnected.subscribe().unwrap();
        assert!(accounts.connect(AccountId::new()).unwrap().refresh(1));
    }
}
