//! Deliver coded callbacks through the host's authority and effect routes.

use super::{Call, Chain, Host, Outcome, internal};
use calternal_plugin_host::operations::{Delivery, Method, Operations};
use calternal_types::{Actor, Error, Result, SpaceId};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Nested {
    operation: String,
    input: Value,
}

impl Host {
    pub(crate) async fn deliver(
        self: Arc<Self>,
        endpoint: Arc<Operations>,
        actor: Actor,
        chain: Chain,
        delivery: Delivery,
    ) -> Result<Value> {
        let space = delivery.space;
        endpoint
            .invoke(delivery, move |callback| {
                let (host, actor, mut chain) = (self.clone(), actor.clone(), chain.clone());
                async move {
                    let name = match callback.method {
                        Method::Query => "s.query",
                        Method::Read => "s.read",
                        Method::Write => "s.write",
                        Method::State => "s.state",
                        Method::Call => {
                            let nested: Nested = crate::decode(callback.input)?;
                            return host
                                .dispatch_code_call(
                                    actor,
                                    space,
                                    nested,
                                    callback.idempotency_key,
                                    chain,
                                )
                                .await;
                        }
                        Method::Run | Method::RunSend | Method::RunStop if chain.run.is_some() => {
                            return Err(Error::forbidden("A run cannot manage other runs."));
                        }
                        Method::Run => {
                            return host
                                .run_start(
                                    actor,
                                    space,
                                    callback.input,
                                    callback.idempotency_key,
                                    chain.request.clone(),
                                    chain.session_item,
                                    chain.session,
                                )
                                .await;
                        }
                        Method::RunSend => {
                            return host
                                .run_send(
                                    actor,
                                    space,
                                    callback.input,
                                    callback.idempotency_key,
                                    chain.session,
                                )
                                .await;
                        }
                        Method::RunStop => {
                            return host
                                .run_stop(actor, space, callback.input, chain.session)
                                .await;
                        }
                    };
                    chain.selection = None;
                    host.dispatch(
                        actor,
                        space,
                        Call {
                            call: name.into(),
                            input: callback.input,
                            idempotency_key: callback.idempotency_key,
                            slot_selection: None,
                        },
                        chain,
                    )
                    .await
                }
            })
            .await
    }

    fn dispatch_code_call(
        self: Arc<Self>,
        actor: Actor,
        space: SpaceId,
        nested: Nested,
        key: Option<String>,
        chain: Chain,
    ) -> Outcome {
        Box::pin(async move {
            let mut result = self
                .clone()
                .dispatch_code(
                    actor.clone(),
                    space,
                    Call {
                        call: nested.operation,
                        input: nested.input,
                        idempotency_key: key,
                        slot_selection: None,
                    },
                    chain,
                )
                .await?;
            tokio::task::spawn_blocking(move || {
                self.with_space(&actor, space, |storage| {
                    storage.redact_rows(&actor, &mut result);
                    Ok(result)
                })
            })
            .await
            .map_err(internal)?
        })
    }
}
