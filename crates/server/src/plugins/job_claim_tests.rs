//! A plugin result cannot select the native agent interpreter for an ordinary job.
use super::{Jobs, Session, jobs::Admission};
use calternal_types::AccountId;
use serde_json::json;
#[test]
fn native_claim_checks_operation_and_account_in_the_claim_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = Jobs::open(dir.path()).unwrap();
    let account = AccountId::new();
    let space = calternal_types::SpaceId::new().to_string();
    let admit = |operation: &str, key: &str, plugin: &str| {
        jobs.admit(Admission{space:space.clone(),account,session:Session::Static,plugin:plugin.into(),operation:operation.into(),input:json!({"request":{"thread_id":uuid::Uuid::now_v7()},"thread_request":{"forged":true}}),parent:None,generation:1,request_key:key.into()}).unwrap()
    };
    let ordinary = admit("fixture.run", "ordinary", "calternal-orchestrator");
    assert!(
        jobs.claim_turn(&space, &ordinary, Some(account))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        jobs.get(&space, &ordinary).unwrap().unwrap().state,
        calternal_core::JobState::Admitted
    );
    assert!(jobs.claim(&space, &ordinary).unwrap().is_some());
    let fake_native = admit("agent.turn", "fake-native", "fixture");
    assert!(
        jobs.claim_turn(&space, &fake_native, Some(account))
            .unwrap()
            .is_none()
    );
    let native = admit("agent.turn", "native", "calternal-orchestrator");
    assert!(
        jobs.claim_turn(&space, &native, Some(AccountId::new()))
            .unwrap()
            .is_none()
    );
    let claim = jobs
        .claim_turn(&space, &native, Some(account))
        .unwrap()
        .unwrap();
    let queued = jobs.get(&space, &native).unwrap().unwrap().view();
    assert_eq!(queued.state, calternal_core::JobState::Admitted);
    assert!(
        jobs.claim_turn(&space, &native, Some(account))
            .unwrap()
            .is_none()
    );
    let placement = json!({"machine_id":uuid::Uuid::new_v4()});
    jobs.start_placed_turn(&space, &native, claim.job.runs, &claim.live, &placement)
        .unwrap();
    let running = jobs.get(&space, &native).unwrap().unwrap().view();
    assert_eq!(running.state, calternal_core::JobState::Running);
    assert_eq!(running.checkpoint, placement);

    let queued = admit("fixture.run", "erase-queued", "fixture");
    jobs.lifecycle.close(account.as_uuid());
    jobs.lifecycle.drain(account.as_uuid());
    assert_eq!(
        jobs.claim(&space, &queued).err().unwrap().code,
        calternal_types::ErrorCode::NotFound
    );
    assert_eq!(
        jobs.get(&space, &queued).unwrap().unwrap().state,
        calternal_core::JobState::Admitted
    );
    jobs.retire_account(account).unwrap();
    assert_eq!(
        jobs.get(&space, &queued).unwrap().unwrap().state,
        calternal_core::JobState::Cancelled
    );
}

#[test]
fn cancellation_observes_the_prepared_resource_and_prevents_late_preparation() {
    let dir = tempfile::tempdir().unwrap();
    let jobs = std::sync::Arc::new(Jobs::open(dir.path()).unwrap());
    let space = calternal_types::SpaceId::new().to_string();
    let id = jobs
        .admit(Admission {
            space: space.clone(),
            account: AccountId::new(),
            session: Session::Static,
            plugin: "calternal-orchestrator".into(),
            operation: "agent.turn".into(),
            input: json!({"request":{"thread_id":uuid::Uuid::now_v7()}}),
            parent: None,
            generation: 1,
            request_key: "lease-fence".into(),
        })
        .unwrap();
    let claimed = jobs.claim_turn(&space, &id, None).unwrap().unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (cancel_tx, cancel_rx) = std::sync::mpsc::channel();
    let (worker_jobs, worker_space, worker_id) = (jobs.clone(), space.clone(), id.clone());
    let worker = std::thread::spawn(move || {
        entered_rx.recv().unwrap();
        cancel_tx.send(()).unwrap();
        worker_jobs.cancel(&worker_space, &worker_id).unwrap();
        worker_jobs
            .get(&worker_space, &worker_id)
            .unwrap()
            .unwrap()
            .checkpoint
    });
    let result = jobs
        .checkpoint_prepared(&space, &id, claimed.job.runs, &claimed.live, || {
            entered_tx.send(()).unwrap();
            cancel_rx.recv().unwrap();
            Ok(json!({"lease_id":"prepared-before-cancel"}))
        })
        .unwrap();
    assert_eq!(worker.join().unwrap(), result);
    let mut prepared = false;
    assert!(
        jobs.checkpoint_prepared(&space, &id, claimed.job.runs, &claimed.live, || {
            prepared = true;
            Ok(json!({"lease_id":"too-late"}))
        })
        .is_err()
    );
    assert!(!prepared, "canceled work admitted an unobservable resource");
}
