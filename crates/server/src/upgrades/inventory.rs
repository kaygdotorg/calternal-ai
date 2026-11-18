//! Resolve the deployment footprint through each writer's frozen schema descriptor.

use std::{collections::BTreeSet, ffi::OsStr, io, path::Path};

use calternal_store::migrations::{Schema, inspect, migrate_release};
use rusqlite::{Connection, OpenFlags};
use uuid::Uuid;

use super::{Deployment, Root};

struct Database {
    relative: String,
    schema: Schema<'static>,
    version: i64,
}

impl Database {
    fn open(&self, root: &Path, flags: OpenFlags) -> io::Result<Connection> {
        connection(&root.join(&self.relative), flags)
    }
}

fn sqlite(error: rusqlite::Error) -> io::Error {
    io::Error::other(error)
}

fn connection(path: &Path, flags: OpenFlags) -> io::Result<Connection> {
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        .map_err(sqlite)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(sqlite)?;
    Ok(conn)
}

type Paths = BTreeSet<(Root, String)>;

pub(super) struct Inventory {
    databases: Vec<Database>,
    paths: Paths,
    full: Option<Vec<super::full::Entry>>,
}

impl Inventory {
    pub(super) fn plan(deploy: &Deployment, incompatible: bool, live: bool) -> io::Result<Self> {
        let appdata = &deploy.roots.appdata;
        let mut databases = databases(appdata)?;
        databases.retain(|database| database.version < database.schema.version());
        let mut paths = Paths::new();
        for database in &databases {
            for ending in ["", "-wal", "-shm", "-journal"] {
                paths.insert((Root::Appdata, format!("{}{ending}", database.relative)));
            }
            if database.relative == "system/auth.db" {
                disk_files(
                    &deploy.roots.userdata_ssd,
                    Path::new(".calternal/recovery"),
                    Root::Ssd,
                    &mut paths,
                )?;
                paths.insert((Root::Appdata, "system/invite.key".into()));
                paths.insert((Root::Appdata, "setup.token".into()));
            } else if let Some(owner) = database
                .relative
                .strip_prefix("spaces/")
                .and_then(|path| path.split_once('/'))
            {
                paths.insert((Root::Appdata, format!("secrets/spaces/{}/head", owner.0)));
            }
        }
        let full = if incompatible {
            Some(super::full::plan(deploy, std::mem::take(&mut paths), live)?)
        } else {
            for (role, path) in &paths {
                super::snapshot::check_source(deploy.root(*role)?, path)?;
            }
            None
        };
        Ok(Self {
            databases,
            paths,
            full,
        })
    }

    // The root locks are still held, and the intent was fsynced before this call.
    pub(super) fn apply(
        self,
        deploy: &Deployment,
        previous: String,
        candidate: String,
        additive: bool,
    ) -> io::Result<usize> {
        let copies = if let Some(entries) = self.full {
            let count = entries.len();
            deploy.prepare_full(previous, candidate, entries)?;
            count
        } else {
            let count = self.paths.len();
            deploy.prepare(previous, candidate, self.paths)?;
            count
        };
        for database in self.databases {
            let conn = database.open(&deploy.roots.appdata, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
                .map_err(sqlite)?;
            migrate_release(&conn, database.schema, additive).map_err(sqlite)?;
            check_integrity(&conn)?;
            if database.relative == "system/auth.db" {
                recovery(deploy)?
                    .refresh_after_migration(&conn)
                    .map_err(io::Error::other)?;
            }
        }
        Ok(copies)
    }
}

fn databases(appdata: &Path) -> io::Result<Vec<Database>> {
    use crate::{account_input, machines, plugins};
    let mut entries = Vec::new();
    let mut inspect_one = |relative: String, schema: Schema<'static>| -> io::Result<()> {
        let path = appdata.join(&relative);
        if !super::snapshot::check_source(appdata, &relative)? {
            return Ok(());
        }
        let conn = connection(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version = inspect(&conn, schema).map_err(sqlite)?;
        entries.push(Database {
            relative,
            schema,
            version,
        });
        Ok(())
    };
    for (relative, schema) in [
        ("system/auth.db", calternal_auth::db::MIGRATIONS),
        ("system/machines.db", machines::db::upgrade_schema()),
        ("system/content-index.db", calternal_content::ROUTE_SCHEMA),
        (
            "installed-plugins.sqlite3",
            plugins::package_store::UPGRADE_SCHEMA,
        ),
    ] {
        inspect_one(relative.into(), schema)?;
    }
    for (scope, schemas) in [
        (
            "spaces",
            [
                Some(("content.db", calternal_content::SPACE_SCHEMA)),
                Some(("index.db", calternal_store::JOURNAL_SCHEMA)),
                Some(("jobs.db", plugins::jobs::upgrade_schema())),
                None,
                Some(("secrets.db", calternal_secrets::OWNER_SCHEMA)),
            ],
        ),
        (
            "accounts",
            [
                Some(("content.db", calternal_content::ACCOUNT_SCHEMA)),
                Some(("input.db", account_input::UPGRADE_SCHEMA)),
                Some(("agent-settings.db", plugins::AGENT_SETTINGS_SCHEMA)),
                Some(("onboarding.db", crate::onboarding::SCHEMA)),
                Some(("secrets.db", calternal_secrets::OWNER_SCHEMA)),
            ],
        ),
    ] {
        for id in owners(appdata, scope)? {
            for (file, schema) in schemas.into_iter().flatten() {
                inspect_one(format!("{scope}/{id}/{file}"), schema)?;
            }
        }
    }
    Ok(entries)
}

fn owners(appdata: &Path, scope: &str) -> io::Result<Vec<Uuid>> {
    let root = super::tree::Tree::open(appdata)?;
    let dir = match root.directory(OsStr::new(scope)) {
        Ok(dir) => dir,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut ids = Vec::new();
    for name in dir.names()? {
        if let Some(id) = name.to_str().and_then(|name| Uuid::parse_str(name).ok()) {
            dir.directory(&name)?;
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Count only regular source files, and use the existing role roots as the
/// only pathname authorities. The private deployment manifest encodes UTF-8.
fn disk_files(root: &Path, prefix: &Path, role: Root, files: &mut Paths) -> io::Result<()> {
    let root = super::tree::Tree::open(root)?;
    let directory = root.directory(prefix.as_os_str())?;
    selective_files(&directory, prefix, role, files)
}

fn selective_files(
    directory: &super::tree::Tree,
    prefix: &Path,
    role: Root,
    files: &mut Paths,
) -> io::Result<()> {
    use rustix::fs::FileType;
    for name in directory.names()? {
        let relative = prefix.join(&name);
        let kind = FileType::from_raw_mode(directory.inspect(&name)?.st_mode);
        match kind {
            FileType::Directory => {
                selective_files(&directory.directory(&name)?, &relative, role, files)?
            }
            FileType::RegularFile => {
                let relative = relative
                    .to_str()
                    .ok_or_else(|| io::Error::other("snapshot path is not UTF-8"))?;
                files.insert((role, relative.to_owned()));
            }
            FileType::Socket => {}
            _ => {
                return Err(io::Error::other(format!(
                    "{role:?} root entry {} cannot be included in a selective restore point",
                    relative.display()
                )));
            }
        }
    }
    Ok(())
}

fn check_integrity(conn: &Connection) -> io::Result<()> {
    let status: String = conn
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(sqlite)?;
    let foreign_key_error: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    if status == "ok" && !foreign_key_error {
        Ok(())
    } else {
        Err(io::Error::other(
            "an upgraded database failed integrity checks",
        ))
    }
}

pub(super) fn verify(deploy: &Deployment) -> io::Result<()> {
    for database in databases(&deploy.roots.appdata)? {
        if database.version < database.schema.version() {
            return Err(io::Error::other(
                "deployment still has pending schema changes",
            ));
        }
    }
    let recovery = recovery(deploy)?;
    let auth = connection(
        &deploy.roots.appdata.join("system/auth.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    recovery
        .validate_against_witness(&auth)
        .map_err(io::Error::other)?;
    Ok(())
}

fn recovery(deploy: &Deployment) -> io::Result<calternal_recovery::Coordinator> {
    let recovery_dir = deploy.roots.userdata_ssd.join(".calternal/recovery");
    if !recovery_dir.join("snapshot.json").is_file() {
        return Err(io::Error::other(
            "the installation has no protected recovery snapshot",
        ));
    }
    calternal_recovery::Coordinator::open(&recovery_dir, deploy.installation)
        .map_err(io::Error::other)
}
