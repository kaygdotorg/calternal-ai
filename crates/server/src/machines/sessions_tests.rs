//! Machine sync cancellation across the blocking registry handoff.
use super::*;
use calternal_machine::identity::MachineKey;

#[tokio::test]
async fn abandoned_sync_releases_its_poll_before_lease_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let (service, machine, session) = blocking(move || {
        let spaces = calternal_auth::spaces::Spaces::open(&root.join("system/auth.db")).unwrap();
        let service = MachineService::open(
            &root,
            spaces.machines(),
            crate::machines::Settings::default(),
        )
        .unwrap();
        let owner = Owner::Pool;
        let (code, _) = service.issue_code(owner).unwrap();
        let key = MachineKey::generate().unwrap();
        let machine = service
            .enrol(&EnrolRequest {
                code,
                public_key: key.public_key_b64(),
                name: "fixture".into(),
                os: "linux".into(),
                arch: "x86_64".into(),
                direct: false,
            })
            .unwrap()
            .machine_id;
        let challenge = service.session_start(machine).unwrap();
        let session = service
            .session_finish(&SessionFinish {
                machine_id: machine,
                signature: key.sign_session(machine, &challenge),
                challenge,
                boot_id: Uuid::new_v4(),
                protocol: PROTOCOL_VERSION,
                backend: "podman".into(),
                guest_memory_mib: None,
            })
            .unwrap();
        Ok((service, machine, hash32(&session.token)))
    })
    .await
    .unwrap();

    let request = SyncRequest {
        capacity: Capacity::default(),
        leases: vec![],
        discards: vec![],
        ack: 0,
        draining: false,
    };
    let (committed, registered) = tokio::sync::oneshot::channel();
    let (return_result, wait) = std::sync::mpsc::channel();
    let svc = service.clone();
    let response = tokio::task::spawn_blocking(move || {
        let result = svc.sync_begin(machine, &session, &request).unwrap();
        committed.send(()).unwrap();
        wait.recv().unwrap();
        result
    });
    registered.await.unwrap();
    assert_eq!(
        service.lock().unwrap().machines[&machine]
            .live
            .as_ref()
            .unwrap()
            .polls,
        1
    );
    // Disconnect after registration and before the HTTP task receives it.
    drop(response);
    return_result.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while service.lock().unwrap().machines[&machine]
            .live
            .as_ref()
            .unwrap()
            .polls
            != 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the abandoned registration still holds the machine online");

    service
        .lock()
        .unwrap()
        .machines
        .get_mut(&machine)
        .unwrap()
        .last_seen = Instant::now() - service.settings.lease_ttl;
    let svc = service.clone();
    blocking(move || svc.sweep()).await.unwrap();
    assert!(service.lock().unwrap().machines[&machine].live.is_none());
}
