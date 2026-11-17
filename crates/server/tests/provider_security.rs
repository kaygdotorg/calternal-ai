//! Agent authority uses the account recovery barrier, including failure before intent.
mod support;
use calternal_core::AgentMode;
use calternal_recovery::{Coordinator, Step};
use calternal_server::{
    auth::StoredDirectory,
    plugins::{Jobs, Session, jobs::Admission},
    turns::access::Authority,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use uuid::Uuid;

#[test]
fn agent_tokens_modes_grants_and_discards_publish_before_acknowledgement() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let dir = tempfile::tempdir().unwrap();
    let directory = StoredDirectory::load_or_create(dir.path()).unwrap();
    let dev = support::dev(&directory);
    let protected = dir.path().join("protected");
    calternal_fsio::private::directory(&protected).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicBool::new(false));
    let broken = dir.path().join("not-a-recovery-directory");
    std::fs::write(&broken, b"unavailable").unwrap();
    let (seen, failing) = (count.clone(), fail.clone());
    let recovery = Arc::new(
        Coordinator::with_observer(
            &protected,
            Uuid::now_v7(),
            Arc::new(move |step| {
                if step == Step::BeforeIntent && failing.swap(false, Ordering::SeqCst) {
                    return Coordinator::open(&broken, Uuid::nil()).map(|_| ());
                }
                if step == Step::FileSynced {
                    seen.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            }),
        )
        .unwrap(),
    );
    directory
        .spaces
        .configure_recovery(recovery.clone())
        .unwrap();
    let jobs = Jobs::open(dir.path())
        .unwrap()
        .with_security(directory.spaces.agents());
    let thread = Uuid::now_v7();
    let space = dev.personal.to_string();
    let id = jobs
        .admit(Admission {
            space: space.clone(),
            plugin: "calternal-orchestrator".into(),
            account: dev.owner.id,
            session: Session::Static,
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":thread}}),
            parent: None,
            generation: 0,
            request_key: "authority".into(),
        })
        .unwrap();
    let claimed = jobs.claim(&space, &id).unwrap().unwrap();
    let mut a = Authority {
        actor: calternal_types::Actor::agent(dev.owner.id, thread),
        space: space.clone(),
        job: id.clone(),
        mode: AgentMode::LeastAccess,
        source_app: "calternal-notes".into(),
        grants: vec![],
    };
    count.store(0, Ordering::SeqCst);
    jobs.agent_prepare_scope(&a, claimed.job.runs).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "scope not protected");
    count.store(0, Ordering::SeqCst);
    let token = jobs.agent_token(&a, claimed.job.runs).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "token not protected");
    a.mode = AgentMode::FullAccess;
    fail.store(true, Ordering::SeqCst);
    assert!(jobs.agent_scope_change(&a).is_err());
    assert_eq!(
        jobs.agent_authenticate(&token).unwrap().unwrap().mode,
        AgentMode::LeastAccess
    );
    count.store(0, Ordering::SeqCst);
    jobs.agent_scope_change(&a).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "mode not protected");
    assert!(jobs.agent_authenticate(&token).unwrap().is_none());
    let approval = jobs.agent_request(&a, "tasks", true).unwrap();
    fail.store(true, Ordering::SeqCst);
    assert!(jobs.agent_approve(&mut a, &approval).is_err());
    assert!(jobs.agent_scope(&space, &id).unwrap().grants.is_empty());
    count.store(0, Ordering::SeqCst);
    jobs.agent_approve(&mut a, &approval).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "grant not protected");
    let lease = Uuid::now_v7();
    let machines = directory.spaces.machines();
    fail.store(true, Ordering::SeqCst);
    assert!(
        machines
            .authorize_discard(dev.owner.id, thread, lease)
            .is_err()
    );
    assert!(
        !machines
            .discard_authorized(dev.owner.id, thread, lease)
            .unwrap()
    );
    count.store(0, Ordering::SeqCst);
    machines
        .authorize_discard(dev.owner.id, thread, lease)
        .unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "discard not protected");
    let snapshot = recovery.validate_and_read().unwrap();
    assert_eq!(snapshot.tables["agent_access"].rows.len(), 1);
    assert_eq!(snapshot.tables["agent_approvals"].rows.len(), 1);
    assert_eq!(snapshot.tables["agent_discards"].rows.len(), 1);
    assert!(!serde_json::to_string(&snapshot).unwrap().contains(&token));
}
