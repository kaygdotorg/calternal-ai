//! Denial probes against live assignments on the product router.
use std::{net::SocketAddr, sync::Arc};

use calternal_machine::{
    identity::MachineKey,
    protocol::{
        EnrolRequest, Owner, PROTOCOL_VERSION, SPACE_CHANNEL_UPGRADE, SessionFinish,
        SpaceChannelAssignment,
    },
};
use calternal_types::{AccountId, SpaceId};
use serde_json::Value;
use uuid::Uuid;

use crate::{auth::StoredDirectory, machines::MachineService};

/// Enroll the fixture machine through the real one-time code and challenge flow.
pub(super) fn enroll(service: &MachineService, owner: Owner) -> (Uuid, String) {
    let (machine, key) = enroll_key(service, owner);
    (machine, session(service, machine, &key, Uuid::new_v4()))
}

/// Enroll and retain the machine key for reconnect qualification.
pub(super) fn enroll_key(service: &MachineService, owner: Owner) -> (Uuid, MachineKey) {
    let (code, _) = service.issue_code(owner).unwrap();
    let key = MachineKey::generate().unwrap();
    let machine = service
        .enrol(&EnrolRequest {
            code,
            public_key: key.public_key_b64(),
            name: "carrier".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            direct: false,
        })
        .unwrap()
        .machine_id;
    (machine, key)
}

/// Prove a fresh session with the enrolled key and the same machine boot.
pub(super) fn session(
    service: &MachineService,
    machine: Uuid,
    key: &MachineKey,
    boot: Uuid,
) -> String {
    let challenge = service.session_start(machine).unwrap();
    service
        .session_finish(&SessionFinish {
            machine_id: machine,
            signature: key.sign_session(machine, &challenge),
            challenge,
            boot_id: boot,
            protocol: PROTOCOL_VERSION,
            backend: "podman".into(),
            guest_memory_mib: None,
        })
        .unwrap()
        .token
}

/// A second owner and their enrolled machine for cross-binding checks.
#[derive(Clone)]
pub(super) struct CarrierIsolation {
    address: SocketAddr,
    authority: String,
    client: reqwest::Client,
    foreign_space: SpaceId,
    foreign_machine: Uuid,
    foreign_token: String,
}

impl CarrierIsolation {
    /// Give the second owner a private space and an independently signed session.
    pub(super) fn new(
        directory: &StoredDirectory,
        service: &MachineService,
        guest: AccountId,
        address: SocketAddr,
        port: u16,
    ) -> Self {
        let (foreign_machine, foreign_token) = enroll(service, Owner::Account { id: guest });
        Self {
            address,
            authority: format!("localhost:{port}"),
            client: reqwest::Client::new(),
            foreign_space: directory
                .spaces
                .create(guest, "Carrier isolation")
                .unwrap()
                .id,
            foreign_machine,
            foreign_token,
        }
    }

    /// Use HTTP/1.1 with real upgrade headers and assert no switch occurred.
    pub(super) async fn refused(
        &self,
        space: SpaceId,
        assignment: Uuid,
        bearer: Option<&str>,
    ) -> Value {
        let request = self
            .client
            .get(format!(
                "http://{}/api/v1/machine/spaces/{space}/channels/{assignment}",
                self.address
            ))
            .version(reqwest::Version::HTTP_11)
            .header("host", &self.authority)
            .header("connection", "Upgrade")
            .header("upgrade", SPACE_CHANNEL_UPGRADE);
        let request = match bearer {
            Some(token) => request.bearer_auth(token),
            None => request,
        };
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        assert!(!response.headers().contains_key("upgrade"));
        response.json().await.unwrap()
    }

    /// Try a live assignment from mismatched scopes before the legitimate runner.
    pub(super) async fn check(
        &self,
        service: &Arc<MachineService>,
        channel: &SpaceChannelAssignment,
        token: &str,
    ) -> Value {
        let unavailable = self
            .refused(channel.space, Uuid::new_v4(), Some(token))
            .await;
        for (space, bearer) in [
            (self.foreign_space, Some(token)),
            (channel.space, Some(self.foreign_token.as_str())),
            (channel.space, None),
            (channel.space, Some("cm1_unknown")),
        ] {
            assert_eq!(
                self.refused(space, channel.assignment, bearer).await,
                unavailable
            );
        }
        let revoker = service.clone();
        let machine = self.foreign_machine;
        assert!(
            tokio::task::spawn_blocking(move || revoker.revoke_any(machine))
                .await
                .unwrap()
                .unwrap()
        );
        assert_eq!(
            self.refused(channel.space, channel.assignment, Some(&self.foreign_token))
                .await,
            unavailable
        );
        unavailable
    }
}
