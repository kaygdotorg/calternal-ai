//! Online client conformance against a separately built real disposable server.
use calternal_client::{CancellationToken, Client, ClientError, PendingCall, operations};
use calternal_core::*;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Server {
    child: Child,
    data: PathBuf,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.data);
    }
}
#[tokio::test]
#[ignore = "requires CALTERNAL_SERVER_BIN pointing to the built real server"]
async fn real_server_operations_receipts_and_authority() {
    let binary = std::env::var_os("CALTERNAL_SERVER_BIN").expect("set CALTERNAL_SERVER_BIN");
    let data = std::env::temp_dir().join(format!(
        "calternal-client-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let child = Command::new(binary)
        .args(["serve", "--development", "--port", "0", "--data-dir"])
        .arg(&data)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server { child, data };
    let startup = server.data.join("startup.json");
    for _ in 0..300 {
        if startup.exists() {
            break;
        }
        assert!(
            server.child.try_wait().unwrap().is_none(),
            "server exited during startup"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let info: serde_json::Value =
        serde_json::from_slice(&std::fs::read(startup).expect("server startup timeout")).unwrap();
    let token = std::fs::read_to_string(info["token_files"]["owner"].as_str().unwrap()).unwrap();
    let url = info["base_url"].as_str().unwrap();
    let client = Client::new(url, token.trim()).unwrap();
    let cancel = CancellationToken::new();
    let discovery = client.describe_cancellable(&cancel).await.unwrap();
    assert_eq!(discovery.protocol_version, PROTOCOL_VERSION);
    let spaces = client
        .call(
            &PendingCall::<operations::SpacesList>::new(&SpacesListInput {}).unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    let space = spaces.spaces.first().unwrap().id.clone();
    let pending = PendingCall::<operations::NotesCreate>::new(&NotesCreateInput {
        space_id: space.clone(),
        title: "Rust client".into(),
        markdown: "first\n".into(),
    })
    .unwrap();
    let created = client.call(&pending, &cancel).await.unwrap();
    let replay = client.call(&pending, &cancel).await.unwrap();
    assert_eq!(created, replay);
    let resolved = client
        .resolve("notes", &created.note.id, &cancel)
        .await
        .unwrap();
    assert_eq!(resolved.space_id, space);
    let get = PendingCall::<operations::NotesGet>::new(&NotesGetInput {
        space_id: String::new(),
        id: created.note.id.clone(),
    })
    .unwrap();
    assert_eq!(client.call(&get, &cancel).await.unwrap(), created);
    let bad = Client::new(url, "invalid-session").unwrap();
    assert!(matches!(
        bad.describe_cancellable(&cancel).await,
        Err(ClientError::ReauthenticationRequired)
    ));
    assert!(matches!(
        client.resolve("files", &created.note.id, &cancel).await,
        Err(ClientError::Api { status: 404, .. })
    ));
    let new_space = client
        .call(
            &PendingCall::<operations::SpacesCreate>::new(&SpacesCreateInput {
                name: "Client fixture".into(),
            })
            .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
    assert_ne!(new_space.space.id, discovery.principal.id);
    client
        .call(
            &PendingCall::<operations::SpacesDelete>::new(&SpaceInput {
                space_id: new_space.space.id,
            })
            .unwrap(),
            &cancel,
        )
        .await
        .unwrap();
}
