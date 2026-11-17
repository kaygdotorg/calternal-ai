//! Independent review expectations for person requests and approval lifetimes
//! (docs/reviews/requests-independent-review-2026-11-18.md). Each test states one
//! conformance expectation from docs/requests.md or docs/agent-instructions.md;
//! a failure names a gap. Real per-space jobs and protected auth databases; no
//! machine or provider account.
mod support;
use calternal_core::{AgentMode, JsonObject, OperationRisk};
use calternal_server::{
    plugins::{
        Jobs, Session,
        jobs::{Admission, RunEnd},
    },
    turns::access::Authority,
};
use serde_json::json;
use uuid::Uuid;

fn obj(v: serde_json::Value) -> JsonObject {
    serde_json::from_value(v).unwrap()
}

/// One account, its personal space, and a jobs handle backed by protected auth.
struct Fixture {
    _dir: tempfile::TempDir,
    jobs: Jobs,
    space: String,
    account: calternal_types::AccountId,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let directory = calternal_server::auth::StoredDirectory::load_or_create(dir.path()).unwrap();
    let dev = support::dev(&directory);
    let jobs = Jobs::open(dir.path())
        .unwrap()
        .with_security(directory.spaces.agents());
    Fixture {
        space: dev.personal.to_string(),
        account: dev.owner.id,
        jobs,
        _dir: dir,
    }
}

impl Fixture {
    /// Admit and claim one turn of `thread`, returning its job id and run number.
    fn turn(&self, thread: Uuid, key: &str) -> (String, u32) {
        let id = self
            .jobs
            .admit(Admission {
                space: self.space.clone(),
                plugin: "calternal-orchestrator".into(),
                account: self.account,
                session: Session::Static,
                operation: "agent.turn".into(),
                input: json!({"request":{"thread_id":thread}}),
                parent: None,
                generation: 0,
                request_key: key.into(),
            })
            .unwrap();
        let run = self.jobs.claim(&self.space, &id).unwrap().unwrap();
        (id, run.job.runs)
    }

    fn authority(&self, thread: Uuid, job: &str, mode: AgentMode) -> Authority {
        Authority {
            actor: calternal_types::Actor::agent(self.account, thread),
            space: self.space.clone(),
            job: job.into(),
            mode,
            source_app: String::new(),
            grants: vec![],
        }
    }
}

/// `requests.approve` refuses `once` for `secrets.revoke` because the operation
/// has no durable receipt, so `this_thread` is the only lifetime the person can
/// pick. Approve for me says an irreversible action asks, and the grant binds
/// the original request ID so that "a new action asks again". With no receipt,
/// replaying that request ID is a new execution. Expectation: after the
/// approved action's turn has ended, a later turn of the same thread cannot
/// repeat the irreversible action under the retained grant without asking.
#[test]
fn a_this_thread_grant_for_an_unreceipted_irreversible_action_does_not_authorize_later_turns() {
    let f = fixture();
    let thread = Uuid::now_v7();
    let (job, run) = f.turn(thread, "revoke-turn");
    let a = f.authority(thread, &job, AgentMode::ApproveForMe);
    f.jobs.agent_prepare_scope(&a, run).unwrap();
    f.jobs.agent_token(&a, run).unwrap();
    let input = obj(json!({"space_id": f.space, "name": "smtp"}));
    let (op, rid, risk) = ("secrets.revoke", "revoke-1", OperationRisk::Irreversible);
    assert!(
        a.check_operation(op, &input, rid, true, risk).is_err(),
        "precondition: Approve for me asks before an irreversible action"
    );
    let id = f
        .jobs
        .agent_action_request(&a, op, &input, rid, risk, true)
        .unwrap();
    f.jobs
        .agent_decide(a.actor.account, &id, "approve-revoke", "this_thread", None)
        .unwrap();
    let approved = f.jobs.agent_scope(&f.space, &job).unwrap();
    assert!(
        approved
            .check_operation(op, &input, rid, true, risk)
            .is_ok(),
        "precondition: the approved action may run once"
    );
    f.jobs
        .finish(&f.space, &job, run, RunEnd::Done(json!({})))
        .unwrap();
    f.jobs.agent_revoke_run(&f.space, &job, run).unwrap();

    let (next, run) = f.turn(thread, "later-turn");
    let later = f
        .jobs
        .agent_prepare_scope(&f.authority(thread, &next, AgentMode::ApproveForMe), run)
        .unwrap();
    assert!(
        later.check_operation(op, &input, rid, true, risk).is_err(),
        "a later turn can repeat an unreceipted irreversible action without asking; grants {:?}",
        later.grants
    );
}
