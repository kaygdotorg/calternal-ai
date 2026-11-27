//! Decode subscription selections and send their initial snapshots.
use super::{
    ClientFrame, MAX_SUBSCRIPTIONS, ServerFrame, Subscription, eval_subscription, run_logs,
    send_error, send_frame,
};
use crate::Host;
use axum::extract::ws::WebSocket;
use calternal_types::{Actor, Error, ErrorCode, SpaceId};
use std::{collections::HashMap, sync::Arc};

pub(super) async fn handle_client_frame(
    host: &Arc<Host>,
    actor: &Actor,
    space: SpaceId,
    frame: ClientFrame,
    subs: &mut HashMap<String, Subscription>,
    socket: &mut WebSocket,
) -> std::result::Result<(), ()> {
    match frame {
        ClientFrame::Ping => send_frame(socket, &ServerFrame::Pong).await,
        ClientFrame::Unsubscribe { sub } => {
            subs.remove(&sub);
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
            if !subs.contains_key(&sub) && subs.len() >= MAX_SUBSCRIPTIONS {
                return send_error(
                    socket,
                    Some(sub),
                    &Error::new(
                        ErrorCode::EffectiveScopeLimitExceeded,
                        "Too many live subscriptions on this session.",
                    ),
                )
                .await;
            }
            match (query, records, run_log) {
                (None, None, Some(request)) => {
                    let mut subscription = Subscription::RunLog {
                        run: request.run,
                        after: request.after,
                        initial: true,
                        finished: false,
                    };
                    if let Err(error) =
                        run_logs::refresh(host, actor, space, &sub, &mut subscription, socket).await
                    {
                        send_error(socket, Some(sub), &error).await?;
                    } else {
                        subs.insert(sub, subscription);
                    }
                    Ok(())
                }
                (Some(input), None, None) => {
                    let mut subscription = Subscription::Query {
                        input,
                        last_payload: None,
                    };
                    if let Err(error) =
                        eval_subscription(host, actor, space, &sub, &mut subscription, None, socket)
                            .await
                    {
                        send_error(socket, Some(sub), &error).await?;
                    } else {
                        subs.insert(sub, subscription);
                    }
                    Ok(())
                }
                (None, Some(req), None) => {
                    let mut subscription = Subscription::Records {
                        item: req.item,
                        last_anchor: req.after.clone(),
                        last_version: 0,
                        last_count: 0,
                        last_meta: None,
                    };
                    if let Err(error) = eval_subscription(
                        host,
                        actor,
                        space,
                        &sub,
                        &mut subscription,
                        Some(req.after),
                        socket,
                    )
                    .await
                    {
                        send_error(socket, Some(sub), &error).await?;
                    } else {
                        subs.insert(sub, subscription);
                    }
                    Ok(())
                }
                _ => {
                    send_error(
                        socket,
                        Some(sub),
                        &Error::invalid_input(
                            "Provide one of query, records or run_log in a subscribe frame.",
                        ),
                    )
                    .await
                }
            }
        }
    }
}
