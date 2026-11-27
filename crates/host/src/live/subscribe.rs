//! Decode subscription selections and send their initial snapshots.
use super::wire::LiveSocket;
use super::{
    ClientFrame, Connection, ServerFrame, Subscription, eval_subscription, run_logs, send_error,
    send_frame,
};
use crate::Host;
use calternal_types::{Actor, Error, ErrorCode, SpaceId};
use std::{collections::HashMap, sync::Arc};

pub(super) async fn handle_client_frame(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    frame: ClientFrame,
    subs: &mut HashMap<String, Subscription>,
    admission: &mut Connection,
    socket: &mut LiveSocket,
) -> std::result::Result<(), ()> {
    match frame {
        ClientFrame::Ping => send_frame(socket, ServerFrame::Pong).await.map_err(|_| ()),
        ClientFrame::Unsubscribe { sub } => {
            if subs.remove(&sub).is_some() {
                admission.unsubscribe();
            }
            Ok(())
        }
        ClientFrame::Subscribe {
            sub,
            query,
            records,
            run_log,
        } => {
            if sub.is_empty() || sub.len() > 128 {
                return send_error(
                    socket,
                    Some(sub),
                    &Error::invalid_input("Subscription ID must be 1 to 128 bytes."),
                )
                .await;
            }
            let (mut subscription, initial_after) = match (query, records, run_log) {
                (None, None, Some(request)) => (
                    Subscription::RunLog {
                        run: request.run,
                        after: request.after,
                        initial: true,
                        finished: false,
                    },
                    None,
                ),
                (Some(input), None, None) => (
                    Subscription::Query {
                        input,
                        last_payload: None,
                    },
                    None,
                ),
                (None, Some(req), None) => (
                    Subscription::Records {
                        item: req.item,
                        last_anchor: None,
                        last_version: 0,
                        last_count: 0,
                        last_meta: None,
                    },
                    Some(req.after),
                ),
                _ => {
                    return send_error(
                        socket,
                        Some(sub),
                        &Error::invalid_input(
                            "Provide one of query, records or run_log in a subscribe frame.",
                        ),
                    )
                    .await;
                }
            };
            if !admission.refresh(1) {
                return send_error(
                    socket,
                    Some(sub),
                    &Error::new(ErrorCode::RetryAfter, "Live refresh quota exceeded."),
                )
                .await;
            }
            let new = !subs.contains_key(&sub);
            if new && let Err(error) = admission.subscribe() {
                return send_error(socket, Some(sub), &error).await;
            }
            let result = if matches!(subscription, Subscription::RunLog { .. }) {
                run_logs::refresh(host, actor, space, &sub, &mut subscription, socket).await
            } else {
                eval_subscription(
                    host,
                    actor,
                    space,
                    &sub,
                    &mut subscription,
                    initial_after,
                    socket,
                )
                .await
            };
            if let Err(error) = result {
                if new {
                    admission.unsubscribe();
                }
                send_error(socket, Some(sub), &error).await
            } else {
                subs.insert(sub, subscription);
                Ok(())
            }
        }
    }
}
