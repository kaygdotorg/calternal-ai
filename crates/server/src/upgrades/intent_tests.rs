//! Fault boundaries of the local deployment intent.
use super::*;
use std::path::Path;

struct TestDeployment(Deployment);

impl TestDeployment {
    fn new(root: &Path) -> Self {
        let mut roots = Roots {
            appdata: root.join("appdata"),
            userdata_ssd: root.join("ssd"),
            userdata_hdd: root.join("hdd"),
        };
        for dir in [
            &mut roots.appdata,
            &mut roots.userdata_ssd,
            &mut roots.userdata_hdd,
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::create_dir_all(roots.appdata.join("system")).unwrap();
        Self(Deployment {
            roots,
            machine: None,
            installation: Uuid::now_v7(),
        })
    }

    fn prepare<const N: usize>(&self, files: [&str; N]) -> io::Result<()> {
        let paths = files
            .into_iter()
            .map(|path| {
                let (role, path) = path.split_once('/').unwrap();
                let role = match role {
                    "appdata" => Root::Appdata,
                    "ssd" => Root::Ssd,
                    _ => unreachable!(),
                };
                (role, path.to_owned())
            })
            .collect();
        self.0.prepare("previous".into(), "candidate".into(), paths)
    }

    fn commit(&self) -> io::Result<()> {
        self.0.commit("candidate")
    }

    fn restore(&self) -> io::Result<Option<String>> {
        self.0.restore()
    }
}

fn put(root: &Path, path: &str, value: &[u8]) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    calternal_fsio::write_atomic_mode(&path, value, 0o600).unwrap();
}

#[test]
fn incomplete_upgrade_restores_exact_authority_and_absent_companions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(root, "appdata/system/auth.db", b"old authority");
    put(
        root,
        "ssd/.calternal/recovery/snapshot.json",
        b"old witness",
    );
    let paths = [
        "appdata/system/auth.db",
        "appdata/system/auth.db-wal",
        "ssd/.calternal/recovery/snapshot.json",
    ];
    let deployment = TestDeployment::new(root);
    deployment.prepare(paths).unwrap();

    put(root, paths[0], b"migrated authority");
    put(root, paths[1], b"new sidecar");
    put(root, paths[2], b"migrated witness");
    deployment.restore().unwrap();
    assert_eq!(
        std::fs::read(root.join(paths[0])).unwrap(),
        b"old authority"
    );
    assert!(!root.join(paths[1]).exists());
    assert_eq!(std::fs::read(root.join(paths[2])).unwrap(), b"old witness");
}

#[test]
fn committed_upgrade_keeps_new_acknowledgements() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let db = "appdata/system/machines.db";
    put(root, db, b"original");
    let deployment = TestDeployment::new(root);
    deployment.prepare([db]).unwrap();
    deployment.commit().unwrap();
    put(root, db, b"acknowledged after cutover");
    assert!(deployment.restore().is_err());
    assert_eq!(
        std::fs::read(root.join(db)).unwrap(),
        b"acknowledged after cutover"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn recovering_an_incomplete_full_copy_keeps_the_live_roots() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let deployment = TestDeployment::new(root);
    put(root, "appdata/system/auth.db", b"live authority");
    put(root, "ssd/work/file", b"live workspace");
    put(root, "ssd/.calternal-upgrade/0", b"partial backup");

    let mut intent = Intent::copying(
        deployment.0.installation,
        "previous".into(),
        "candidate".into(),
    );
    intent.make_full();
    deployment.0.publish(&mut intent).unwrap();

    assert_eq!(deployment.restore().unwrap().as_deref(), Some("previous"));
    assert_eq!(
        std::fs::read(root.join("appdata/system/auth.db")).unwrap(),
        b"live authority"
    );
    assert_eq!(
        std::fs::read(root.join("ssd/work/file")).unwrap(),
        b"live workspace"
    );
    assert!(!root.join("ssd").join(COPY_DIR).exists());
    assert!(deployment.0.read().unwrap().is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn full_restore_preserves_link_bytes_and_never_writes_through_a_link() {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, symlink};

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let deployment = TestDeployment::new(root);
    let ssd = root.join("ssd");
    let outside = root.join("outside");
    put(root, "outside/file", b"outside");
    put(root, "ssd/work/file", b"saved");
    std::fs::create_dir(ssd.join("empty")).unwrap();
    std::fs::hard_link(ssd.join("work/file"), ssd.join("alias")).unwrap();
    let targets = [
        Path::new("/usr/bin/python3"),
        Path::new("../../outside/file"),
        Path::new("cycle"),
        Path::new(std::ffi::OsStr::from_bytes(b"dangling-\xff")),
    ];
    for (name, target) in ["python", "escape", "cycle", "dangling"]
        .into_iter()
        .zip(targets)
    {
        symlink(target, ssd.join(name)).unwrap();
    }
    inventory::Inventory::plan(&deployment.0, true, false)
        .unwrap()
        .apply(&deployment.0, "previous".into(), "candidate".into(), false)
        .unwrap();
    assert_eq!(
        deployment.0.read().unwrap().unwrap().phase,
        Phase::Preparing
    );

    std::fs::remove_file(ssd.join("work/file")).unwrap();
    std::fs::remove_dir(ssd.join("work")).unwrap();
    std::fs::remove_dir(ssd.join("empty")).unwrap();
    symlink(&outside, ssd.join("work")).unwrap();
    assert!(deployment.restore().is_err());
    assert_eq!(std::fs::read(outside.join("file")).unwrap(), b"outside");

    std::fs::remove_file(ssd.join("work")).unwrap();
    std::fs::remove_file(ssd.join("alias")).unwrap();
    symlink(outside.join("file"), ssd.join("alias")).unwrap();
    for name in ["python", "escape", "cycle", "dangling"] {
        std::fs::remove_file(ssd.join(name)).unwrap();
    }
    deployment.restore().unwrap();
    assert_eq!(std::fs::read(ssd.join("work/file")).unwrap(), b"saved");
    assert_eq!(std::fs::read(outside.join("file")).unwrap(), b"outside");
    assert!(ssd.join("empty").is_dir());
    assert_eq!(
        std::fs::metadata(ssd.join("work/file")).unwrap().ino(),
        std::fs::metadata(ssd.join("alias")).unwrap().ino()
    );
    for (name, target) in ["python", "escape", "cycle", "dangling"]
        .into_iter()
        .zip(targets)
    {
        assert_eq!(std::fs::read_link(ssd.join(name)).unwrap(), target);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn full_preflight_rejects_fifo_and_hard_links_outside_the_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let deployment = TestDeployment::new(root);
    let ssd = root.join("ssd");
    put(root, "outside", b"shared externally");
    std::fs::hard_link(root.join("outside"), ssd.join("hard-link")).unwrap();
    let error = inventory::Inventory::plan(&deployment.0, true, true)
        .err()
        .unwrap();
    assert!(error.to_string().contains("hard-link names extend outside"));
    std::fs::remove_file(ssd.join("hard-link")).unwrap();
    let fifo = ssd.join("pipe");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR,
        0,
    )
    .unwrap();
    let error = inventory::Inventory::plan(&deployment.0, true, true)
        .err()
        .unwrap();
    assert!(error.to_string().contains("pipe"));
    assert!(deployment.0.read().unwrap().is_none());
}

#[cfg(target_os = "linux")]
#[test]
fn full_restore_rejects_a_changed_manifest_before_removing_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let deployment = TestDeployment::new(root);
    put(root, "ssd/work/file", b"saved");
    inventory::Inventory::plan(&deployment.0, true, false)
        .unwrap()
        .apply(&deployment.0, "previous".into(), "candidate".into(), false)
        .unwrap();

    put(root, "ssd/work/file", b"live copy");
    let path = root.join("appdata").join(INTENT);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["entries"].as_array_mut().unwrap().clear();
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let error = deployment.restore().unwrap_err();
    assert!(error.to_string().contains("intent failed its checksum"));
    assert_eq!(
        std::fs::read(root.join("ssd/work/file")).unwrap(),
        b"live copy"
    );
}
