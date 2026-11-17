//! Group commits for role writes on the auth worker.
use super::*;
use rusqlite::TransactionBehavior;
use serde_json::Value;
use tokio::sync::oneshot;

/// Bound the number of requests whose results one protected commit can release.
pub(super) const MAX_BATCH: usize = 32;

type Apply = Box<dyn FnOnce(&Connection) -> Result<Value> + Send>;

pub(super) struct RoleWrite {
    pub(super) apply: Apply,
    pub(super) reply: oneshot::Sender<Result<Value>>,
}

pub(super) fn execute(connection: &mut Connection, batch: Vec<RoleWrite>) {
    let (applies, replies): (Vec<_>, Vec<_>) = batch
        .into_iter()
        .map(|write| (write.apply, write.reply))
        .unzip();
    match commit_batch(connection, applies) {
        Ok(results) => {
            for (reply, result) in replies.into_iter().zip(results) {
                let _ = reply.send(result);
            }
        }
        Err(error) => {
            for reply in replies {
                let _ = reply.send(Err(Error::internal(&error)));
            }
        }
    }
}

fn commit_batch(connection: &mut Connection, applies: Vec<Apply>) -> Result<Vec<Result<Value>>> {
    let mut transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql)?;
    let mut results = Vec::with_capacity(applies.len());
    let mut accepted = false;
    for apply in applies {
        let savepoint = transaction.savepoint().map_err(sql)?;
        let result = apply(&savepoint);
        if result.is_ok() {
            savepoint.commit().map_err(sql)?;
            accepted = true;
        } else {
            // The default savepoint behavior rolls back and then releases it.
            savepoint.finish().map_err(sql)?;
        }
        results.push(result);
    }
    if accepted {
        // One recovery proposal and resolution witness cover every success.
        // No caller receives a success until publication completes.
        if let Err(error) = crate::spaces::commit(transaction) {
            for result in &mut results {
                if result.is_ok() {
                    *result = Err(Error::internal(&error));
                }
            }
        }
    } else {
        transaction.rollback().map_err(sql)?;
    }
    Ok(results)
}
