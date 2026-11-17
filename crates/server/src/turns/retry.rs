//! Durable provider retry policy; machine silence never enters this scheduler.
use super::blocking;
use crate::{
    api::Shared,
    auth::{Directory, Identity},
    machines::{LeaseState, MachineService},
    plugins::job_sql::now_ms,
};
use calternal_core::{AgentRetrySettings, ProviderBilling, ProviderFailureClass};
use calternal_types::{Capability, Error};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

/// Five retries after the initial call; each is preceded by this wait.
pub const RETRY_SECONDS: [u64; 5] = [30, 60, 120, 240, 480];
/// API-billing availability cadence after the fifth retry fails.
pub const AVAILABILITY_SECONDS: u64 = 3 * 60 * 60;

/// Derive the next durable wait from a classified provider error. The initial
/// failure has `completed_retries=0`; after retry five, API billing checks every
/// three hours. Unknown resets never become guessed subscription wake-ups.
/// No I/O; callers must also prove there is no pending tool effect.
pub fn next(
    settings: AgentRetrySettings,
    class: ProviderFailureClass,
    billing: ProviderBilling,
    reset: Option<u64>,
    completed_retries: u32,
    now: i64,
) -> Option<Value> {
    if !matches!(
        class,
        ProviderFailureClass::Transient | ProviderFailureClass::UsageLimit
    ) {
        return None;
    }
    if class == ProviderFailureClass::UsageLimit && billing == ProviderBilling::Subscription {
        if !settings.subscription_limits {
            return None;
        }
        let at = i64::try_from(reset?).ok()?;
        if at > now {
            return Some(
                json!({"retry_count":completed_retries.saturating_add(1),"next_attempt_ms":at,"reason":"subscription_limit","billing":billing}),
            );
        }
        return None;
    }
    if !settings.provider_errors {
        return None;
    }
    let (wait, count) = if let Some(wait) = RETRY_SECONDS.get(completed_retries as usize) {
        (*wait, completed_retries + 1)
    } else if billing == ProviderBilling::Api {
        (AVAILABILITY_SECONDS, completed_retries.saturating_add(1))
    } else {
        return None;
    };
    Some(
        json!({"retry_count":count,"next_attempt_ms":now.saturating_add((wait*1000)as i64),"reason":"provider_unavailable","billing":billing}),
    )
}

pub(super) fn manifest_defaults<D: Directory>(
    state: &Shared<D>,
) -> Result<AgentRetrySettings, Error> {
    let plugin = state
        .registry
        .get("calternal-orchestrator")
        .ok_or_else(|| Error::unavailable("orchestrator plugin is not installed"))?;
    let boolean = |id: &str| {
        plugin
            .manifest
            .settings
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.default.as_bool())
            .ok_or_else(|| Error::internal("retry setting is not a typed manifest boolean"))
    };
    Ok(AgentRetrySettings {
        provider_errors: boolean("retry-provider-errors")?,
        subscription_limits: boolean("snooze-subscription-limits")?,
    })
}

/// Restore provider wait timers and readmit resolved person waits at startup.
/// Retry waits use one async task
/// per active job (bounded by existing job admission); they hold no process or
/// execution permit and recheck authority and machine fencing every second.
pub async fn recover<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
) -> Result<(), Error> {
    let st = state.clone();
    for (space, id) in blocking(move || st.jobs.agent_retry_waiting()).await? {
        schedule(state, machines, space, id);
    }
    let st = state.clone();
    for (space, id) in blocking(move || st.jobs.recover_request_waits()).await? {
        super::run::schedule(state, machines, space, id);
    }
    Ok(())
}

pub(super) fn schedule<D: Directory>(
    state: &Shared<D>,
    machines: &Arc<MachineService>,
    space: String,
    id: String,
) {
    let (st, svc) = (state.clone(), machines.clone());
    let future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
        Box::pin(async move {
            loop {
                let (s, sp, jid) = (st.clone(), space.clone(), id.clone());
                let Ok(Some(job)) = blocking(move || s.jobs.get(&sp, &jid)).await else {
                    return;
                };
                if job.state != calternal_core::JobState::RetryWaiting {
                    return;
                }
                let enabled = serde_json::from_value::<AgentRetrySettings>(
                    job.input["retry_settings"].clone(),
                )
                .is_ok_and(|settings| {
                    match job.checkpoint["retry"]["reason"].as_str() {
                        Some("subscription_limit") => settings.subscription_limits,
                        Some("provider_unavailable") => settings.provider_errors,
                        _ => false,
                    }
                });
                if !enabled {
                    let (s, sp, jid, run) = (st.clone(), space.clone(), id.clone(), job.runs);
                    let _ = blocking(move || s.jobs.agent_interrupt_wait(&sp, &jid, run)).await;
                    let _ = super::stop_machine(&svc, &job).await;
                    return;
                }
                let who = Identity { id: job.account };
                let Ok(account) = calternal_types::AccountId::parse(&job.account.to_string())
                else {
                    return;
                };
                let lease = job.checkpoint["lease_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());
                let machine_ok = if let Some(lease) = lease {
                    let m = svc.clone();
                    blocking(move || m.run(account, lease))
                        .await
                        .is_ok_and(|r| {
                            matches!(r.state, LeaseState::Running | LeaseState::Assigned)
                        })
                } else {
                    false
                };
                if !machine_ok
                    || !super::session_live(&st, &job).await.unwrap_or(false)
                    || super::authorize(&st, &who, &space, Capability::PluginsUse)
                        .await
                        .is_err()
                {
                    let (s, sp, jid, run) = (st.clone(), space.clone(), id.clone(), job.runs);
                    let _ = blocking(move || s.jobs.agent_interrupt_wait(&sp, &jid, run)).await;
                    return;
                }
                let (s, sp, jid) = (st.clone(), space.clone(), id.clone());
                if blocking(move || s.jobs.agent_retry_due(&sp, &jid, now_ms()))
                    .await
                    .unwrap_or(false)
                {
                    super::run::schedule(&st, &svc, space, id);
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    tokio::spawn(future);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_waits_then_api_cadence_and_subscription_reset() {
        let settings = AgentRetrySettings {
            provider_errors: true,
            subscription_limits: true,
        };
        for (n, seconds) in RETRY_SECONDS.into_iter().enumerate() {
            let v = next(
                settings,
                ProviderFailureClass::Transient,
                ProviderBilling::Api,
                None,
                n as u32,
                1000,
            )
            .unwrap();
            assert_eq!(v["next_attempt_ms"], 1000 + seconds * 1000);
            assert_eq!(v["retry_count"], n + 1);
        }
        assert_eq!(
            next(
                settings,
                ProviderFailureClass::Transient,
                ProviderBilling::Api,
                None,
                5,
                1000
            )
            .unwrap()["next_attempt_ms"],
            10_801_000u64
        );
        assert_eq!(
            next(
                settings,
                ProviderFailureClass::UsageLimit,
                ProviderBilling::Subscription,
                Some(999999),
                2,
                1000
            )
            .unwrap()["next_attempt_ms"],
            999999
        );
        assert!(
            next(
                settings,
                ProviderFailureClass::Authentication,
                ProviderBilling::Api,
                None,
                0,
                1000
            )
            .is_none()
        );
        assert!(
            next(
                settings,
                ProviderFailureClass::Refusal,
                ProviderBilling::Api,
                None,
                0,
                1000
            )
            .is_none()
        );
        assert!(
            next(
                settings,
                ProviderFailureClass::Unknown,
                ProviderBilling::Api,
                None,
                0,
                1000
            )
            .is_none()
        );
    }
}
