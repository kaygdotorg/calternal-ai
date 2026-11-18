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
