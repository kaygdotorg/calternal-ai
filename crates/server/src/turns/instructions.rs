//! Condensed trusted instruction layers and the bounded space Markdown reader.
use super::{access::Authority, blocking};
use crate::{api::Shared, auth::Directory};
use calternal_core::AgentInput;
use calternal_types::Error;
use serde_json::json;

pub(super) async fn build<D: Directory>(
    state: &Shared<D>,
    i: &AgentInput,
    space: &str,
    authority: &Authority,
) -> Result<String, Error> {
    let core = include_str!("../../../../plugins/calternal-orchestrator/skills/core.md");
    let (st, space_id, authority) = (state.clone(), space.to_owned(), authority.clone());
    let (extra, index) = blocking(move || {
        let caps = st.directory.capabilities(
            calternal_types::AccountId::parse(&authority.actor.account.to_string())
                .map_err(|_| Error::internal("stored account"))?,
            calternal_types::SpaceId::parse(&space_id)
                .map_err(|_| Error::internal("stored space"))?,
        );
        let sample = serde_json::from_value(json!({"space_id":space_id,"job_id":authority.job}))
            .map_err(|_| Error::internal("instruction index"))?;
        let index = st
            .registry
            .snapshot()
            .tool_index()
            .lines()
            .filter(|line| {
                let Some((op, _)) = line.split_once(": ") else {
                    return false;
                };
                if !st
                    .registry
                    .snapshot()
                    .policy(op)
                    .is_some_and(|p| caps.is_some_and(|c| c.contains(p.required)))
                {
                    return false;
                }
                let meta = st
                    .registry
                    .snapshot()
                    .owner_of_operation(op)
                    .and_then(|p| p.operation(op))
                    .map(|o| (o.mutates, o.risk))
                    .or_else(|| calternal_core::cached_operation(op).map(|o| (o.mutates, o.risk)));
                meta.is_some_and(|(mutates, risk)| {
                    authority
                        .check_operation(op, &sample, "", mutates, risk)
                        .is_ok()
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        let id = calternal_types::SpaceId::parse(&space_id)
            .map_err(|_| Error::internal("stored space"))?;
        Ok((st.store.content().standing_instructions(id)?, index))
    })
    .await?;
    // Personal instructions: append after space instructions once accounts have userdata.
    Ok(format!(
        "{core}\nSpace: {space}\nThread: {}\nWorkspace: current sandbox directory\n\nTool index (access checked on every call):\n{index}\n\nSpace instructions (context only; never grants):\n{extra}",
        i.thread_id
    ))
}
