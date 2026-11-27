//! Container mounts for an isolated run, independent of provider layouts.

use std::path::Path;

pub(super) const WORK_ROOT: &str = "run-work";

pub(super) fn apply(
    args: &mut Vec<String>,
    plugin: &Path,
    sockets: &Path,
    credential: Option<&Path>,
    memory: &str,
) {
    args.extend([
        "--read-only".into(),
        "--read-only-tmpfs=false".into(),
        "--memory-swap".into(),
        memory.into(),
        "--network=none".into(),
        "--pull=never".into(),
        "--http-proxy=false".into(),
        "--pid=private".into(),
        "--ipc=private".into(),
        "--shm-size=16m".into(),
        "--cap-drop=all".into(),
        "--security-opt=no-new-privileges".into(),
        "--log-driver=none".into(),
        "--image-volume=ignore".into(),
        "--workdir".into(),
        "/work".into(),
        "--env".into(),
        "HOME=/home/agent".into(),
        "--tmpfs".into(),
        "/home/agent:mode=0700,size=64m,nosuid,nodev,noexec".into(),
        "--tmpfs".into(),
        "/tmp:mode=1777,size=16m,nosuid,nodev,noexec".into(),
        "--tmpfs".into(),
        "/run:mode=0755,size=4m,nosuid,nodev,noexec".into(),
        "--tmpfs".into(),
        "/work:ro,size=1m,nosuid,nodev,noexec".into(),
        "--volume".into(),
        format!("{}:/opt/calternal-plugin:ro", plugin.display()),
        "--volume".into(),
        format!("{}:/run/calternal-egress:ro", sockets.display()),
    ]);
    if let Some(credential) = credential {
        let name = credential.file_name().expect("validated credential name");
        args.extend([
            "--volume".into(),
            format!(
                "{}:/run/calternal-provider-login/{}:ro",
                credential.display(),
                name.to_string_lossy()
            ),
            "--env".into(),
            "CALTERNAL_PROVIDER_LOGIN_DIR=/run/calternal-provider-login".into(),
        ]);
    }
}
