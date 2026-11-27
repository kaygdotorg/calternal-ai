//! Disposable passkey enrollment fixture for the Playwright browser client test.

use calternal_auth::{Auth, AuthConfig};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let auth_dir = root.path().join("auth");
    let origin = std::env::args().nth(1).ok_or("expected browser origin")?;
    let config = AuthConfig::new(&origin)?;
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_dir, config)).await??;
    let setup = auth
        .first_owner_token()
        .await?
        .ok_or("expected empty auth directory")?;
    drop(auth);
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut test = Command::new("node")
        .arg(checkout.join("apps/web/e2e/host-chat.mjs"))
        .arg("--fixture")
        .arg(root.path())
        .arg(origin)
        .stdin(Stdio::piped())
        .spawn()?;
    test.stdin
        .take()
        .ok_or("expected private fixture pipe")?
        .write_all(setup.as_bytes())?;
    if !test.wait()?.success() {
        return Err("browser chat fixture failed".into());
    }
    Ok(())
}
