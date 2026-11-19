//! Validate parked worker output and restore deadline tasks without worker permits.
use super::*;
use crate::plugins::{
    Plugin,
    job_sql::now_ms,
    jobs::{
        Job,
        waits::{Park, Resolution, Timer, WaitSource},
    },
    live::Live,
};
use calternal_core::{ApprovalLifetime, RequestDetail};

pub(super) async fn park<D: Directory>(
    state: &Shared<D>,
    job: &Job,
    live: &Arc<Live>,
    plugin: &Arc<Plugin>,
    value: Value,
) -> Result<(), Error> {
    if !resumable(&plugin.manifest, &job.operation) {
        return Err(Error::invalid_input(
            "durable wait needs a replayable run operation",
        ));
    }
    let (st, job, live, plugin) = (state.clone(), job.clone(), live.clone(), plugin.clone());
    let timer = blocking(move || {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Output {
            #[serde(rename = "status")]
            _status: String,
            wait: Park,
        }
        let park = serde_json::from_value::<Output>(value)
            .map_err(|_| Error::invalid_input("invalid durable wait"))?
            .wait;
        let detail = match &park.source {
            WaitSource::Deadline => None,
            WaitSource::Question {
                form,
                schema_operation,
            } => {
                if plugin.operation(schema_operation).is_none() {
                    return Err(Error::invalid_input(
                        "question answer schema operation is undeclared",
                    ));
                }
                let label = crate::question_form::label(form)?;
                Some(RequestDetail::Question {
                    label,
                    form: form.clone(),
                })
            }
            WaitSource::Approval => Some(RequestDetail::Approval {
                app: job.plugin.clone(),
                write: false,
                lifetimes: vec![ApprovalLifetime::Once],
                action: [("label".into(), json!(park.reason))].into_iter().collect(),
            }),
            WaitSource::Webhook { route, .. } => {
                if !plugin
                    .manifest
                    .webhooks
                    .iter()
                    .any(|webhook| &webhook.id == route)
                    || !plugin.manifest.secrets.contains(route)
                    || calternal_secrets::SecretName::for_plugin(&job.plugin, route).is_err()
                {
                    return Err(Error::invalid_input(
                        "webhook route and signing secret must be declared",
                    ));
                }
                None
            }
        };
        let timer = st.jobs.park_signal(&job, &live, park, detail)?;
        if timer.is_some() {
            invalidate(&st, job.account);
        }
        Ok(timer)
    })
    .await?;
    if let Some(timer) = timer {
        schedule_deadline(state, timer);
    }
    Ok(())
}

pub(super) fn schedule_deadline<D: Directory>(state: &Shared<D>, timer: Timer) {
    let state = state.clone();
    let timer = Arc::new(timer);
    let notice = state.jobs.register_wait_timer(&timer);
    let task: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> = Box::pin(
        async move {
            let mut confirm = true;
            loop {
                if confirm {
                    let (st, timer) = (state.clone(), timer.clone());
                    match blocking(move || st.jobs.wait_is_pending(&timer)).await {
                        Ok(false) => break,
                        Ok(true) => confirm = false,
                        Err(error) if error.code == calternal_types::ErrorCode::NotFound => break,
                        Err(_) => {
                            tokio::select! {
                                _ = notice.notified() => break,
                                _ = tokio::time::sleep(Duration::from_secs(1)) => continue,
                            }
                        }
                    }
                }
                let delay = timer.deadline_ms.saturating_sub(now_ms()).max(0) as u64;
                tokio::select! {
                    _ = notice.notified() => break,
                    _ = tokio::time::sleep(Duration::from_millis(delay.min(30_000))) => (),
                }
                if now_ms() < timer.deadline_ms {
                    continue;
                }
                let (st, t) = (state.clone(), timer.clone());
                match blocking(move || st.jobs.resolve_wait(&t.space, &t.id, &t.job, None)).await {
                    Ok(Resolution::Woken) => {
                        invalidate_async(&state, timer.account).await;
                        schedule(&state, timer.space.clone(), timer.job.clone());
                        break;
                    }
                    Ok(_) if now_ms() < timer.deadline_ms => continue,
                    Ok(_) => break,
                    Err(error) if error.code == calternal_types::ErrorCode::NotFound => break,
                    Err(error) => {
                        tracing::warn!(job = %timer.job, reason = %error.message, "plugin deadline retry");
                        tokio::select! {
                            _ = notice.notified() => break,
                            _ = tokio::time::sleep(Duration::from_secs(1)) => (),
                        }
                    }
                }
            }
            state.jobs.forget_wait_timer(&timer, &notice);
        },
    );
    tokio::spawn(task);
}

async fn invalidate_async<D: Directory>(state: &Shared<D>, account: calternal_types::AccountId) {
    let st = state.clone();
    let _ = blocking(move || {
        invalidate(&st, account);
        Ok(())
    })
    .await;
}

fn invalidate<D: Directory>(state: &Shared<D>, account: calternal_types::AccountId) {
    if let Err(error) = state.inputs.invalidate(account) {
        tracing::warn!(%account, reason = %error.message, "plugin Inbox invalidation failed");
    }
}
