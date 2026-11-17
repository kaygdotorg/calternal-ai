//! Independent review expectations for thread tokens, permission modes and grants
//! (docs/reviews/provider-independent-review-2026-11-17.md). Each test states one
//! conformance expectation from docs/agent-instructions.md; a failure names a gap.
//! Real per-space jobs and auth databases; no machine or provider account.
mod support;
use calternal_core::{AgentMode, JsonObject, OperationRisk};
use calternal_server::{
    plugins::{
        Jobs, Session,
        jobs::{Admission, RunEnd},
    },
    turns::access::{Authority, Grant, action_scope},
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
    fn turn(&self, thread: Uuid, session: Session, key: &str) -> (String, u32) {
        let id = self
            .jobs
            .admit(Admission {
                space: self.space.clone(),
                plugin: "calternal-orchestrator".into(),
                account: self.account,
                session,
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
    fn authority(&self, thread: Uuid, job: &str, mode: AgentMode, source: &str) -> Authority {
        Authority {
            actor: calternal_types::Actor::agent(self.account, thread),
            space: self.space.clone(),
            job: job.into(),
            mode,
            source_app: source.into(),
            grants: vec![],
        }
    }
}

fn scope(mode: AgentMode, source: &str, grants: Vec<(String, bool)>) -> Authority {
    Authority {
        actor: calternal_types::Actor::agent(calternal_types::AccountId::new(), Uuid::now_v7()),
        space: Uuid::now_v7().to_string(),
        job: Uuid::now_v7().to_string(),
        mode,
        source_app: source.into(),
        grants: grants
            .into_iter()
            .map(|(app, write)| Grant {
                app,
                write,
                action_hash: None,
            })
            .collect(),
    }
}

/// Least access must never be wider than the default mode: an irreversible or
/// external operation that Approve for me sends to a person must also ask in
/// Least access, even inside the originating app or an approved app grant.
#[test]
fn least_access_asks_for_high_risk_operations_that_approve_for_me_asks_for() {
    let cases: [(&str, &str, Vec<(String, bool)>, OperationRisk); 3] = [
        // Built-in irreversible operation in the originating app.
        (
            "spaces",
            "spaces.delete",
            vec![],
            OperationRisk::Irreversible,
        ),
        // Plugin-declared external effect in the originating app.
        ("mail", "mail.send", vec![], OperationRisk::External),
        // Irreversible operation covered only by an approved app grant.
        (
            "",
            "secrets.revoke",
            vec![("secrets".into(), true)],
            OperationRisk::Irreversible,
        ),
    ];
    for (source, op, grants, risk) in cases {
        let least = scope(AgentMode::LeastAccess, source, grants.clone());
        let default = Authority {
            mode: AgentMode::ApproveForMe,
            ..least.clone()
        };
        let input = obj(json!({"space_id": least.space}));
        assert!(
            default
                .check_operation(op, &input, "r1", true, risk)
                .is_err(),
            "{op}: precondition, Approve for me asks"
        );
        assert!(
            least.check_operation(op, &input, "r1", true, risk).is_err(),
            "{op} ({risk:?}) ran in Least access without a person approval \
             (source app {source:?}, grants {grants:?}) although Approve for me asks"
        );
    }
}

/// Full access executes irreversible and external effects without an action
/// approval; both approval modes refuse those same effects under RulesOnly.
#[test]
fn full_access_runs_irreversible_and_external_without_approval() {
    for (op, risk) in [
        ("spaces.delete", OperationRisk::Irreversible),
        ("mail.send", OperationRisk::External),
    ] {
        let full = scope(AgentMode::FullAccess, "", vec![]);
        let input = obj(json!({"space_id": full.space}));
        assert!(full.check_operation(op, &input, "r1", true, risk).is_ok());
        for mode in [AgentMode::LeastAccess, AgentMode::ApproveForMe] {
            let restricted = Authority {
                mode,
                ..full.clone()
            };
            assert!(
                restricted
                    .check_operation(op, &input, "r1", true, risk)
                    .is_err()
            );
        }
    }
}

/// A Least-access denial creates an exact-action approval request. Once the
/// person approves that request, the same action must be allowed; otherwise the
/// agent cannot make progress in Least access through the request it was given.
#[test]
fn least_access_accepts_the_action_approval_its_denial_created() {
    let f = fixture();
    let thread = Uuid::now_v7();
    let (job, run) = f.turn(thread, Session::Static, "least-approval");
    let mut a = f.authority(thread, &job, AgentMode::LeastAccess, "");
    f.jobs.agent_token(&a, run).unwrap();
    let input = obj(json!({"space_id": f.space}));
    assert!(
        a.check_operation("notes.list", &input, "r1", false, OperationRisk::Read)
            .is_err(),
        "precondition: notes is outside the default Least-access scope"
    );
    // The dispatcher's denial path, then the person's approval of that request.
    let id = f
        .jobs
        .agent_action_request(&a, "notes.list", &input, "r1", OperationRisk::Read, false)
        .unwrap();
    f.jobs.agent_approve(&mut a, &id).unwrap();
    assert!(
        a.grants.iter().any(|g| g.action_hash.as_deref()
            == Some(action_scope("notes.list", &input, "r1", OperationRisk::Read).as_str())),
        "approval recorded the action scope"
    );
    assert!(
        a.check_operation("notes.list", &input, "r1", false, OperationRisk::Read)
            .is_ok(),
        "an approved Least-access request is still denied; grants {:?}",
        a.grants
    );
}

/// The person chooses the mode in the composer for each message. A narrower mode
/// requested for the next turn must take effect, since no other operation can
/// change a thread's mode while no turn is active (agent.mode refuses final turns).
#[test]
fn next_turn_honours_a_narrower_mode_chosen_by_the_person() {
    let f = fixture();
    let thread = Uuid::now_v7();
    let (first, run) = f.turn(thread, Session::Static, "full-turn");
    let full = f.authority(thread, &first, AgentMode::FullAccess, "");
    f.jobs.agent_prepare_scope(&full, run).unwrap();
    f.jobs
        .finish(&f.space, &first, run, RunEnd::Done(json!({})))
        .unwrap();
    f.jobs.agent_revoke_run(&f.space, &first, run).unwrap();

    let (next, run) = f.turn(thread, Session::Static, "least-turn");
    let least = f.authority(thread, &next, AgentMode::LeastAccess, "");
    let saved = f.jobs.agent_prepare_scope(&least, run).unwrap();
    assert_eq!(
        saved.mode,
        AgentMode::LeastAccess,
        "the next turn ran in the previous wider mode although the person chose Least access"
    );
}

/// A thread token acts as the person and must not outlive the person's own sign-in:
/// once the admitting session is no longer live (revoked or unknown), the token
/// must stop authenticating, as ordinary plugin host calls already require
/// (plugin_host.rs `host_calls_recheck_the_callers_sign_in`).
#[test]
fn thread_token_stops_when_the_admitting_person_session_is_not_live() {
    let f = fixture();
    let thread = Uuid::now_v7();
    let revoked = Session::Installation {
        account: f.account,
        installation: calternal_types::InstallationId::new(),
    };
    let (job, run) = f.turn(thread, revoked, "revoked-session");
    let a = f.authority(thread, &job, AgentMode::ApproveForMe, "");
    let token = f.jobs.agent_token(&a, run).unwrap();
    assert!(
        f.jobs.agent_authenticate(&token).unwrap().is_none(),
        "a thread token authenticated although its admitting person session is not live"
    );
    // A real stored admitting proof works until revocation, then both credential
    // entry points refuse it immediately; expiry uses the same account predicate.
    let live = calternal_types::InstallationId::new();
    let c = rusqlite::Connection::open(f._dir.path().join("system/auth.db")).unwrap();
    let now = calternal_types::now_ms();
    c.execute("INSERT INTO installations(id,user_id,kind,name,token_hash,created_at,last_seen_at,expires_at,assertion_at) VALUES(?1,?2,'cli','fixture',?3,?4,?4,?5,?4)",rusqlite::params![live.to_string(),f.account.to_string(),vec![7u8;32],now,now+60000]).unwrap();
    let thread = Uuid::now_v7();
    let (job, run) = f.turn(
        thread,
        Session::Installation {
            account: f.account,
            installation: live,
        },
        "live-proof",
    );
    let a = f.authority(thread, &job, AgentMode::ApproveForMe, "");
    let token = f.jobs.agent_token(&a, run).unwrap();
    let digest = calternal_auth::token::hash(&token);
    assert!(f.jobs.agent_authenticate(&token).unwrap().is_some());
    assert!(
        f.jobs
            .agent_current(&f.space, &job, &digest)
            .unwrap()
            .is_some()
    );
    c.execute(
        "UPDATE installations SET revoked_at=1 WHERE id=?1",
        [live.to_string()],
    )
    .unwrap();
    assert!(f.jobs.agent_authenticate(&token).unwrap().is_none());
    assert!(
        f.jobs
            .agent_current(&f.space, &job, &digest)
            .unwrap()
            .is_none()
    );
    c.execute(
        "UPDATE installations SET revoked_at=NULL,expires_at=1 WHERE id=?1",
        [live.to_string()],
    )
    .unwrap();
    assert!(f.jobs.agent_authenticate(&token).unwrap().is_none());
}
