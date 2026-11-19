//! Private subscription storage owned by an account and keyed by installation.

use calternal_core::PushSubscribeInput;
use calternal_store::migrations::{Schema, migrate};
use calternal_types::{AccountId, Error, InstallationId, Owner, now_ms};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::{io::ErrorKind, path::Path, time::Duration};
use uuid::Uuid;

const MAX_DEVICES: i64 = 8;
const RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;
const TEST_COOLDOWN_MS: i64 = 60_000;

pub(crate) const SCHEMA: Schema<'static> = Schema::new(
    "web-push.db",
    1,
    &["
        CREATE TABLE subscriptions(
            device TEXT PRIMARY KEY,
            generation TEXT NOT NULL,
            endpoint TEXT NOT NULL UNIQUE,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            expires INTEGER NOT NULL,
            test_after INTEGER NOT NULL DEFAULT 0
        ) STRICT, WITHOUT ROWID;
    "],
    &[],
);

pub(super) struct Subscription {
    pub(super) device: InstallationId,
    pub(super) generation: String,
    pub(super) endpoint: String,
    pub(super) p256dh: String,
    pub(super) auth: String,
}

fn unavailable(_: impl std::fmt::Display) -> Error {
    Error::unavailable("Web push storage is unavailable.")
}

fn open(appdata: &Path, account: AccountId, create: bool) -> Result<Option<Connection>, Error> {
    let folder = appdata
        .join(Owner::Account(account).directory())
        .join(account.to_string());
    let path = folder.join("web-push.db");
    match path.symlink_metadata() {
        Ok(info) if info.is_file() => (),
        Ok(_) => return Err(Error::unavailable("Web push storage is unavailable.")),
        Err(e) if e.kind() == ErrorKind::NotFound && create => {
            calternal_fsio::private::directory(&folder).map_err(unavailable)?;
            calternal_fsio::private::file(&path).map_err(unavailable)?;
        }
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(unavailable(e)),
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(unavailable)?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(unavailable)?;
    db.execute_batch("PRAGMA secure_delete=ON;")
        .map_err(unavailable)?;
    migrate(&db, SCHEMA).map_err(unavailable)?;
    Ok(Some(db))
}

pub(super) fn register(
    appdata: &Path,
    account: AccountId,
    device: InstallationId,
    input: &PushSubscribeInput,
) -> Result<(), Error> {
    let mut db = open(appdata, account, true)?.ok_or_else(|| unavailable("missing store"))?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let now = now_ms();
    tx.execute("DELETE FROM subscriptions WHERE expires <= ?1", [now])
        .map_err(unavailable)?;
    let id = device.to_string();
    let duplicate = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE endpoint=?1 AND device<>?2)",
            params![input.endpoint, id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(unavailable)?;
    if duplicate {
        return Err(Error::conflict(
            "First remove this browser's previous push registration.",
        ));
    }
    let exists = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE device=?1)",
            [&id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(unavailable)?;
    let generation = Uuid::new_v4().to_string();
    let values = params![
        id,
        generation,
        input.endpoint,
        input.p256dh,
        input.auth,
        now + RETENTION_MS,
    ];
    if exists {
        tx.execute(
            "UPDATE subscriptions SET generation=?2,endpoint=?3,p256dh=?4,auth=?5,expires=?6 WHERE device=?1",
            values,
        ).map_err(unavailable)?;
    } else {
        let count: i64 = tx
            .query_row("SELECT count(*) FROM subscriptions", [], |r| r.get(0))
            .map_err(unavailable)?;
        if count >= MAX_DEVICES {
            return Err(Error::conflict(
                "Remove a push device before registering another.",
            ));
        }
        tx.execute(
            "INSERT INTO subscriptions(device,generation,endpoint,p256dh,auth,expires) VALUES(?1,?2,?3,?4,?5,?6)",
            values,
        ).map_err(unavailable)?;
    }
    tx.commit().map_err(unavailable)
}

pub(super) fn devices(appdata: &Path, account: AccountId) -> Result<Vec<(String, i64)>, Error> {
    let Some(db) = open(appdata, account, false)? else {
        return Ok(Vec::new());
    };
    let mut query = db
        .prepare("SELECT device,expires FROM subscriptions WHERE expires>?1 ORDER BY device")
        .map_err(unavailable)?;
    query
        .query_map([now_ms()], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(unavailable)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(unavailable)
}

pub(super) fn subscriptions(
    appdata: &Path,
    account: AccountId,
) -> Result<Vec<Subscription>, Error> {
    let Some(db) = open(appdata, account, false)? else {
        return Ok(Vec::new());
    };
    let mut query = db.prepare(
        "SELECT device,generation,endpoint,p256dh,auth FROM subscriptions WHERE expires>?1 ORDER BY device",
    ).map_err(unavailable)?;
    let rows = query
        .query_map([now_ms()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
            ))
        })
        .map_err(unavailable)?;
    rows.map(|r| {
        let (device, generation, endpoint, p256dh, auth) = r.map_err(unavailable)?;
        let device = InstallationId::parse(&device).map_err(unavailable)?;
        Ok(Subscription {
            device,
            generation,
            endpoint,
            p256dh,
            auth,
        })
    })
    .collect()
}

pub(super) fn remove(
    appdata: &Path,
    account: AccountId,
    device: InstallationId,
    generation: Option<&str>,
) -> Result<bool, Error> {
    let Some(db) = open(appdata, account, false)? else {
        return Ok(false);
    };
    db.execute(
        "DELETE FROM subscriptions WHERE device=?1 AND (?2 IS NULL OR generation=?2)",
        params![device.to_string(), generation],
    )
    .map(|count| count == 1)
    .map_err(unavailable)
}

pub(super) fn admit_test(
    appdata: &Path,
    account: AccountId,
    device: InstallationId,
) -> Result<String, Error> {
    let unregistered = || Error::not_found("Register this browser before sending a test.");
    let Some(mut db) = open(appdata, account, false)? else {
        return Err(unregistered());
    };
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let now = now_ms();
    let device = device.to_string();
    let generation = tx.query_row(
        "UPDATE subscriptions SET test_after=?3 WHERE device=?1 AND expires>?2 AND test_after<=?2 RETURNING generation",
        params![device, now, now + TEST_COOLDOWN_MS],
        |r| r.get(0),
    ).optional().map_err(unavailable)?;
    if let Some(generation) = generation {
        tx.commit().map_err(unavailable)?;
        return Ok(generation);
    }
    let test_after: i64 = tx
        .query_row(
            "SELECT test_after FROM subscriptions WHERE device=?1 AND expires>?2",
            params![device, now],
            |r| r.get(0),
        )
        .optional()
        .map_err(unavailable)?
        .ok_or_else(unregistered)?;
    Err(
        Error::rate_limited("Register this browser or wait a minute before testing again.")
            .with_retry_after(Duration::from_millis((test_after - now) as u64)),
    )
}

pub(super) fn is_current(
    appdata: &Path,
    account: AccountId,
    device: InstallationId,
    generation: &str,
) -> Result<bool, Error> {
    let Some(db) = open(appdata, account, false)? else {
        return Ok(false);
    };
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE device=?1 AND generation=?2 AND expires>?3)",
        params![device.to_string(), generation, now_ms()], |row| row.get(0),
    ).map_err(unavailable)
}

pub(super) fn prune_expired(appdata: &Path, lifecycle: &calternal_types::OwnerLifecycle) {
    let Ok(accounts) = std::fs::read_dir(appdata.join("accounts")) else {
        return;
    };
    let now = now_ms();
    for entry in accounts.flatten() {
        let name = entry.file_name();
        let Some(account) = name.to_str().and_then(|id| AccountId::parse(id).ok()) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(_owner) = lifecycle.step(account.as_uuid()) else {
            continue;
        };
        if let Ok(Some(db)) = open(appdata, account, false) {
            let _ = db.execute("DELETE FROM subscriptions WHERE expires<=?1", [now]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrations_are_private_bounded_and_stale_removal_is_fenced() {
        let root = tempfile::tempdir().unwrap();
        let account = AccountId::parse("01932910-0000-7000-8000-000000000001").unwrap();
        let other = AccountId::parse("01932910-0000-7000-8000-000000000002").unwrap();
        let register_one = |i: u8| {
            let id = InstallationId::parse(&format!("01932910-0000-7000-8000-0000000001{i:02x}"))
                .unwrap();
            let result = register(
                root.path(),
                account,
                id,
                &PushSubscribeInput {
                    endpoint: format!("https://fcm.googleapis.com/{i}"),
                    p256dh: "unused".into(),
                    auth: "unused".into(),
                },
            );
            (id, result)
        };
        let first = register_one(0).0;
        let old = subscriptions(root.path(), account)
            .unwrap()
            .remove(0)
            .generation;
        assert!(admit_test(root.path(), account, first).is_ok());
        assert!(register_one(0).1.is_ok());
        assert!(admit_test(root.path(), account, first).is_err());
        assert!(!remove(root.path(), account, first, Some(&old)).unwrap());
        for i in 1..8 {
            assert!(register_one(i).1.is_ok());
        }
        assert!(register_one(8).1.is_err());
        assert!(devices(root.path(), other).unwrap().is_empty());
        assert!(!remove(root.path(), other, first, None).unwrap());
        let db = open(root.path(), account, false).unwrap().unwrap();
        db.execute(
            "UPDATE subscriptions SET expires=0 WHERE device=?1",
            [first.to_string()],
        )
        .unwrap();
        assert_eq!(devices(root.path(), account).unwrap().len(), 7);
        assert!(register_one(8).1.is_ok());
    }
}
