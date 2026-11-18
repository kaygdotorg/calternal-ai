//! Volatile account request state. Challenge and answer are never serialized to storage.
use calternal_core::{PersonRequest, RequestDetail};
use calternal_types::{AccountId, Error, SpaceId};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use uuid::Uuid;

pub(super) const TTL: Duration = Duration::from_secs(10 * 60);
const MAX_ACCOUNT: usize = 16;
const MAX_SHARD: usize = 128;

#[derive(Clone)]
pub(super) struct Target {
    pub space: SpaceId,
    pub thread: Option<Uuid>,
    pub machine: Uuid,
    pub provider: String,
    pub label: String,
    pub login_scope: calternal_machine::protocol::ProviderLoginScope,
    pub policy: crate::plugins::package_store::provider_runtime::SignInPolicy,
    pub retry: bool,
    pub shared: Option<crate::machines::space_execution::SpaceExecution>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prompt {
    pub url: Option<String>,
    pub display_code: Option<String>,
    pub needs_code: bool,
}

pub(super) struct Entry {
    pub id: Uuid,
    pub target: Target,
    pub expires: Instant,
    pub expires_ms: i64,
    pub prompt: Option<Prompt>,
    pub answer: mpsc::Sender<String>,
    pub outcome: Option<bool>,
    pub job: Option<String>,
}

impl Entry {
    pub fn status(&self) -> &'static str {
        match self.outcome {
            None => "pending",
            Some(true) => "done",
            Some(false) => "failed",
        }
    }

    pub fn request(&self) -> Option<PersonRequest> {
        let prompt = self.prompt.as_ref()?;
        Some(PersonRequest {
            id: self.id.to_string(),
            space_id: Some(self.target.space.to_string()),
            thread_id: self.target.thread.map(|thread| thread.to_string()),
            job_id: None,
            detail: RequestDetail::ProviderSignIn {
                provider_id: self.target.provider.clone(),
                label: if self.target.retry {
                    format!("{} needs you to sign in again", self.target.label)
                } else {
                    format!("Sign in to {}", self.target.label)
                },
                url: prompt.url.clone(),
                display_code: prompt.display_code.clone(),
                needs_code: prompt.needs_code,
                expires_at_ms: self.expires_ms,
            },
        })
    }
}

type AccountEntries = HashMap<AccountId, Vec<Entry>>;
/// Sharded short locks for the volatile pending inbox. No I/O under a guard.
#[derive(Default)]
pub(crate) struct Inbox([Mutex<AccountEntries>; 16]);

impl Inbox {
    pub(super) fn access<T>(
        &self,
        account: AccountId,
        f: impl FnOnce(&mut Vec<Entry>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut shard = self.0[usize::from(account.as_uuid().as_bytes()[15] & 15)]
            .lock()
            .map_err(|_| Error::internal("provider sign-in inbox unavailable"))?;
        let now = Instant::now();
        shard.retain(|_, entries| {
            entries.retain(|entry| entry.expires > now);
            !entries.is_empty()
        });
        if !shard.contains_key(&account) && shard.len() >= MAX_SHARD {
            return Err(Error::unavailable("provider sign-in inbox is busy"));
        }
        f(shard.entry(account).or_default())
    }

    pub(super) fn insert(
        &self,
        account: AccountId,
        target: Target,
        job: Option<String>,
    ) -> Result<(Uuid, Option<mpsc::Receiver<String>>), Error> {
        self.access(account, |entries| {
            if let Some(entry) = entries.iter_mut().find(|e| {
                e.outcome.is_none()
                    && e.target.machine == target.machine
                    && e.target.shared == target.shared
                    && e.target.provider == target.provider
                    && e.target.login_scope == target.login_scope
            }) {
                if entry.target.space != target.space {
                    return Err(Error::conflict(
                        "The provider sign-in is in progress in another space.",
                    ));
                }
                if entry.job.is_none() {
                    entry.job = job;
                }
                if entry.target.thread.is_none() {
                    entry.target.thread = target.thread;
                }
                return Ok((entry.id, None));
            }
            if entries.len() >= MAX_ACCOUNT {
                return Err(Error::unavailable(
                    "Too many provider sign-ins for this account.",
                ));
            }
            let id = Uuid::new_v4();
            let (answer, receiver) = mpsc::channel(1);
            entries.push(Entry {
                id,
                target,
                job,
                answer,
                prompt: None,
                outcome: None,
                expires: Instant::now() + TTL,
                expires_ms: calternal_types::now_ms() + TTL.as_millis() as i64,
            });
            Ok((id, Some(receiver)))
        })
    }
}
