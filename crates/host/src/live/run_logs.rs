//! Authorized run log replay through the live window transport.
use super::wire::LiveSocket;
use super::{ServerFrame, Subscription, send_frame};
use crate::Host;
use calternal_types::{Actor, Error, Result, SpaceId};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) run: Uuid,
    pub(super) after: Option<u64>,
}

pub(super) async fn refresh(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    sub: &str,
    subscription: &mut Subscription,
    socket: &mut LiveSocket,
) -> Result<()> {
    let Subscription::RunLog {
        run,
        after,
        initial,
        finished,
    } = subscription
    else {
        return Ok(());
    };
    if *finished {
        return Ok(());
    }
    let batch = host.run_log(actor.clone(), space, *run, *after).await?;
    let offset = batch.offset();
    *finished = batch.stopped();
    if *initial || batch.has_changes() {
        send_frame(
            socket,
            &ServerFrame::RunLog {
                sub: sub.to_owned(),
                batch,
            },
        )
        .await
        .map_err(|_| Error::unavailable("The live socket closed."))?;
    }
    *initial = false;
    *after = Some(offset);
    Ok(())
}
