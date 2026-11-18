//! Verify immutable operator-installed guest image files once per role startup.

use std::{fs::File, io::Read, path::Path};

use ring::digest::{Context, SHA256};

use super::{Config, SandboxError, SandboxResult};

pub(super) async fn check(config: &Config) -> SandboxResult<()> {
    let directory = config.image_directory.clone();
    let pinned = config.image_digest.clone();
    tokio::task::spawn_blocking(move || {
        let digest = |name| digest(&directory.join(name)).map_err(SandboxError::new);
        if digest("image.sha256")? != pinned {
            return Err(SandboxError(
                "microVM image manifest does not match the configured digest".into(),
            ));
        }
        let expected = format!(
            "{}  vmlinux\n{}  root.ext4\n",
            digest("vmlinux")?,
            digest("root.ext4")?,
        );
        let manifest = std::fs::read(directory.join("image.sha256")).map_err(SandboxError::new)?;
        if manifest != expected.as_bytes() {
            return Err(SandboxError(
                "microVM image files do not match the verified manifest".into(),
            ));
        }
        Ok(())
    })
    .await
    .map_err(SandboxError::new)?
}

fn digest(path: &Path) -> std::io::Result<String> {
    let mut context = Context::new(&SHA256);
    let mut file = File::open(path)?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for byte in context.finish().as_ref() {
        write!(hex, "{byte:02x}").expect("write to string");
    }
    Ok(hex)
}
