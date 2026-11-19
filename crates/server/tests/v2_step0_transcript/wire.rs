//! The disposable transcript service over loopback TCP.
use super::fixture;
use calternal_core::SharePermission;
use calternal_server::{api::Shared, auth::Directory, resolve::authorize_item_access};
use calternal_store::{Commit, IdemWrite, SpaceStore, StampedEvent};
use calternal_types::{AccountId, SpaceId};
use ring::hmac;
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io,
    os::fd::AsRawFd,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use uuid::Uuid;

pub(super) const EVENTS: usize = 96;
pub(super) const CUT: i32 = 84;
pub(super) const MAX_PAGE: usize = 128 * 1024;
const KIND: &str = "turns.transcript.committed";
const PAGE: &str = "SELECT seq,data FROM journal
  WHERE kind='turns.transcript.committed' AND item=?1 AND seq>?2 ORDER BY seq LIMIT ?3";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub(super) root: PathBuf,
    pub(super) space: SpaceId,
    pub(super) thread: Uuid,
    pub(super) thread_ref: Uuid,
    pub(super) key: [u8; 32],
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) token: String,
    pub(super) route: String,
    pub(super) body: Value,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) stall: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageRequest {
    thread: Uuid,
    after: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Append {
    batch: usize,
    cut: bool,
}

struct Service {
    state: Shared,
    config: Config,
    log: SpaceStore,
    db_path: PathBuf,
    key: hmac::Key,
    blocked: AtomicUsize,
    writer_pending: AtomicUsize,
    owner: AccountId,
    guest: AccountId,
}

impl Service {
    async fn current(&self, actor: AccountId) -> bool {
        authorize_item_access(
            &self.state,
            self.config.space,
            "threads",
            self.config.thread,
            actor,
            SharePermission::View,
        )
        .await
        .is_ok()
    }

    fn cursor(&self, actor: AccountId, token: &str) -> String {
        let material = format!("{actor}/{}/{token}", self.config.thread_ref);
        let tag = hmac::sign(&self.key, material.as_bytes());
        format!("{token}.{}", hex::encode(&tag.as_ref()[..16]))
    }

    async fn page(&self, actor: AccountId, value: Value) -> Option<Value> {
        let input: PageRequest = serde_json::from_value(value).ok()?;
        let limit = input.limit.unwrap_or(128);
        if input.thread != self.config.thread_ref
            || !(1..=128).contains(&limit)
            || !self.current(actor).await
        {
            return None;
        }
        let after = match input.after.as_deref() {
            None => 0,
            Some(cursor) => {
                let (token, _) = cursor.rsplit_once('.')?;
                let expected = self.cursor(actor, token);
                use subtle::ConstantTimeEq;
                if !bool::from(expected.as_bytes().ct_eq(cursor.as_bytes())) {
                    return None;
                }
                self.log.position(token).await.ok()?
            }
        };
        let path = self.db_path.clone();
        let thread = self.config.thread.to_string();
        let (events, last) = tokio::task::spawn_blocking(move || {
            let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            conn.busy_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            let mut query = conn.prepare(PAGE).unwrap();
            let mut rows = query.query(params![thread, after, limit as i64]).unwrap();
            let mut events = Vec::new();
            let mut encoded = 256; // envelope, cursor, separators
            let mut last = after;
            while let Some(row) = rows.next().unwrap() {
                let data: String = row.get(1).unwrap();
                if encoded + data.len() + 1 > MAX_PAGE {
                    break;
                }
                encoded += data.len() + 1;
                last = row.get(0).unwrap();
                events.push(serde_json::from_str::<Value>(&data).unwrap());
            }
            (events, last)
        })
        .await
        .unwrap();
        let token = self.log.token_at(last).await.ok()?;
        if !self.current(actor).await {
            return None;
        }
        let cursor = self.cursor(actor, &token);
        assert!(cursor.len() <= 128);
        Some(json!({"events":events,"committed_cursor":cursor}))
    }

    async fn append(&self, actor: AccountId, body: Value) -> Option<Value> {
        let input: Append = serde_json::from_value(body).ok()?;
        let key = format!("append-{}", input.batch);
        let previous = self.log.idempotency(actor, &key).await.unwrap();
        if previous.is_some() {
            let receipt = previous.unwrap();
            return Some(json!({"count":receipt.result["count"],"seq":receipt.seq,"replay":true}));
        }
        let events = (0..EVENTS)
            .map(|i| StampedEvent {
                author: actor,
                kind: KIND.into(),
                item: self.config.thread.to_string(),
                data: json!({"event_id": format!("{}-{i}",input.batch), "kind":"text_delta",
                "data":{"text":"append JSON escape: \\ \" 🎵\n".repeat(32)}}),
                installation: None,
                request_id: Some(key.clone()),
            })
            .collect();
        let rows = self
            .log
            .commit(Commit {
                events,
                idempotency: Some(IdemWrite {
                    account: actor,
                    key: key.clone(),
                    action: "transcript.append".into(),
                    payload_hash: key,
                    result: json!({"count":EVENTS}),
                }),
                durable: true,
            })
            .await
            .unwrap();
        if input.cut {
            std::process::exit(CUT);
        }
        Some(json!({"count":rows.len(),"seq":rows.last().unwrap().seq,"replay":false}))
    }

    async fn reply(self: &Arc<Self>, mut stream: TcpStream) -> io::Result<()> {
        let mut body = String::new();
        let mut reader = BufReader::new(&mut stream);
        (&mut reader).read_line(&mut body).await?;
        if body.is_empty() {
            return Ok(());
        }
        let input: Request = serde_json::from_str(&body).map_err(io::Error::other)?;
        let identity = self.state.directory.authenticate(input.token.as_bytes());
        let mut result = None;
        let admin = self.state.directory.protected_spaces().unwrap();
        if let Some(who) = identity {
            result = match &*input.route {
                "transcript.page" => self.page(who.id, input.body).await,
                "fixture.append" if who.id == self.owner => self.append(who.id, input.body).await,
                "fixture.probe" if who.id == self.owner => {
                    Some(json!({"blocked":self.blocked.load(Ordering::Acquire),
                    "pending":self.writer_pending.load(Ordering::Acquire)}))
                }
                "fixture.revoke" if who.id == self.owner => {
                    let space = self.config.space;
                    tokio::task::block_in_place(|| admin.remove(who.id, space, self.guest))
                        .unwrap();
                    Some(json!({"ok":true}))
                }
                "fixture.restore" if who.id == self.owner => {
                    let root = self.config.root.clone();
                    let space = self.config.space;
                    let guest = self.guest;
                    tokio::task::spawn_blocking(move || {
                        let db = Connection::open(root.join("appdata/system/auth.db")).unwrap();
                        db.execute(
                            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
                            params![space.to_string(), guest.to_string()],
                        )
                        .unwrap();
                    })
                    .await
                    .unwrap();
                    Some(json!({"ok":true}))
                }
                "threads.get" => fixture::run(
                    &self.state,
                    who,
                    "threads.get",
                    json!({"id":self.config.thread}),
                )
                .await
                .ok()
                .map(|v| json!({"revision":v["thread"]["revision"]})),
                _ => None,
            };
        }
        let mut bytes =
            serde_json::to_vec(&result.unwrap_or_else(|| json!({"error":"not_found"}))).unwrap();
        bytes.push(b'\n');
        assert!(bytes.len() <= MAX_PAGE);
        if input.stall {
            set_buffer(&stream, libc::SO_SNDBUF);
            let _pending = Pending::new(&self.writer_pending);
            let mut offset = 0;
            loop {
                match stream.try_write(&bytes[offset..]) {
                    Ok(n) => {
                        if n == 0 {
                            return Err(io::Error::from(io::ErrorKind::WriteZero));
                        }
                        offset += n;
                        if offset == bytes.len() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if offset > 0 {
                            self.blocked.fetch_add(1, Ordering::AcqRel);
                        }
                        stream.writable().await?;
                    }
                    Err(e) => return Err(e),
                }
            }
        } else {
            stream.write_all(&bytes).await?;
        }
        Ok(())
    }
}

struct Pending<'a>(&'a AtomicUsize);
impl<'a> Pending<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::AcqRel);
        Self(count)
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) fn set_buffer(socket: &impl AsRawFd, option: libc::c_int) {
    let size: libc::c_int = 4096;
    let result = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            option,
            (&raw const size).cast(),
            std::mem::size_of_val(&size) as libc::socklen_t,
        )
    };
    assert_eq!(result, 0);
}

pub(super) async fn serve(config: Config) {
    let roots = fixture::roots(&config.root);
    let packages = fixture::packages();
    let (state, ids) = tokio::task::block_in_place(|| fixture::open(&roots, &packages));
    let log =
        tokio::task::block_in_place(|| state.store.content().live_journal(config.space).unwrap());
    let db_path = roots
        .appdata
        .join(format!("spaces/{}/index.db", config.space));
    let index = Connection::open(&db_path).unwrap();
    index
        .execute(
            "CREATE INDEX IF NOT EXISTS spike_transcript_scope ON journal(item,seq)
                   WHERE kind='turns.transcript.committed'",
            [],
        )
        .unwrap();
    let plan: String = index
        .query_row(
            &format!("EXPLAIN QUERY PLAN {PAGE}"),
            params![config.thread.to_string(), 0, 128],
            |r| r.get(3),
        )
        .unwrap();
    assert!(plan.contains("spike_transcript_scope"), "{plan}");
    drop(index);
    let (owner, guest) = (ids.owner, ids.guest);
    let key = hmac::Key::new(hmac::HMAC_SHA256, &config.key);
    let service = Arc::new(Service {
        state,
        config,
        log,
        db_path,
        key,
        owner,
        guest,
        blocked: AtomicUsize::new(0),
        writer_pending: AtomicUsize::new(0),
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("STEP0_LISTEN {}", listener.local_addr().unwrap());
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        stream.set_nodelay(true).unwrap();
        let service = service.clone();
        tokio::spawn(async move {
            let _ = service.reply(stream).await;
        });
    }
}
