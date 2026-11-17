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
    assert!(
        jobs.claim_turn(&space, &native, Some(account))
            .unwrap()
            .is_some()
    );
}
