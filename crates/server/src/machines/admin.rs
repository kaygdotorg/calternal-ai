//! Private operator socket for machine administration, first-owner setup and
//! receipt-pruning diagnostics.
//!
//! Machine authority commits through the recovery barrier, and one installation's
//! recovery log has one writer: the process holding the root locks. So an
//! operator command never writes behind a running server. It asks the server
//! over its admin socket, `<appdata>/system/admin.sock` (mode 0600 in a 0700
//! directory; the server also checks that the peer runs as its own Unix user),
//! or, when no server runs, opens the installation itself through the root guard
//! with the pinned installation ID and all three roots, attaching the recovery log
//! before any change. Both paths run the same [`execute`] on a
//! [`MachineService`], so a running server applies a change at once.
//!
//! One request per connection, one JSON line each way: a [`Request`] envelope in,
//! `{"ok":true,"data":<Response>}` or `{"ok":false,"error":<Error>}` out.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use calternal_content::{Content, ReceiptPruningIssue};
use calternal_machine::protocol::Owner;
use calternal_types::{AccountId, Error, SpaceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{MachineService, Settings, authority_error};
use crate::store::Store;

/// The longest request or response line accepted (64 KiB).
const MAX_LINE: u64 = 64 * 1024;
/// How long either side waits for the other's line.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// One operator action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Rotate the first-owner code through the running server.
    SetupCode,
    /// Issue an account replacement grant through privileged local transport.
    AccountRecover {
        /// Immutable account identity, never a display name.
        account: AccountId,
        /// Out-of-band verification reason from the operator.
        reason: String,
    },
    /// Inspect receipt pruning in a single space through the running server.
    ReceiptStatus {
        /// Space UUID to inspect, including dormant spaces.
        space: SpaceId,
    },
    /// Issue a one-time enrolment code for `owner`, valid for `minutes`.
    Code {
        /// Who the enrolled machine will belong to.
        owner: Owner,
        /// Validity in minutes; 0 issues an already expired code.
        minutes: u64,
    },
    /// List every enrolled machine, revoked ones included.
    List,
    /// Revoke one machine.
    Revoke {
        /// Machine UUID.
        machine: Uuid,
    },
    /// Grant or withdraw an account's use of pool machines.
    PoolGrant {
        /// Account UUID.
        account: AccountId,
        /// True grants, false withdraws.
        granted: bool,
    },
}

/// One enrolled machine as the operator sees it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MachineSummary {
    /// Machine UUID.
    pub id: Uuid,
    /// Owner.
    pub owner: Owner,
    /// Label.
    pub name: String,
    /// Operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Revoked machines never authenticate again.
    pub revoked: bool,
    /// Draining machines receive no new placements.
    pub draining: bool,
}

/// Pending erasures for one retained or revoked machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineErasureSummary {
    /// Enrolled machine UUID, retained even when authority is retired.
    pub machine_id: Uuid,
    /// Instructions awaiting successful receipt.
    pub pending: u64,
    /// The machine cannot authenticate and the operator must wipe it.
    pub operator_wipe: bool,
}

/// The result of one [`Request`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    /// A fresh one-time code, its expiry and the public setup address.
    SetupCode {
        /// Display this code only to the operator.
        code: String,
        /// Expiry in Unix milliseconds.
        expires_ms: i64,
        /// Public origin for registration.
        url: String,
    },
    /// A one-time, enrollment-only grant shown privately to the operator.
    AccountRecovery {
        /// Secret to present at the account recovery enrollment screen.
        enrollment_grant: String,
        /// Public origin at which this grant can be used.
        url: String,
    },
    /// Registration has already been completed.
    SetupComplete {
        /// Public origin for signing in.
        url: String,
    },
    /// Receipt-pruning health for the requested space.
    ReceiptStatus {
        /// Reason for impaired pruning, if observed.
        issue: Option<ReceiptPruningIssue>,
    },
    /// A new code and its expiry in Unix milliseconds.
    Code {
        /// The one-time code; shown once.
        code: String,
        /// Expiry in Unix milliseconds.
        expires_ms: i64,
    },
    /// Every enrolled machine.
    List {
        /// Machines, ordered by UUID.
        machines: Vec<MachineSummary>,
        /// Outstanding machine-side erasures, ordered by machine UUID.
        erasures: Vec<MachineErasureSummary>,
    },
    /// Whether the machine existed (and is now revoked).
    Revoke {
        /// False when no such machine exists.
        revoked: bool,
    },
    /// The grant now matches the request.
    PoolGrant,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    installation: Uuid,
    request: Request,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Response>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Error>,
}

/// Run one request on a machine service. Blocks: at most one protected authority
/// commit; call on a blocking thread.
pub fn execute(svc: &MachineService, request: Request) -> Result<Response, Error> {
    Ok(match request {
        Request::SetupCode | Request::AccountRecover { .. } => {
            return Err(Error::unavailable(
                "start this server before requesting a setup code",
            ));
        }
        Request::ReceiptStatus { .. } => {
            return Err(Error::unavailable(
                "start this server before inspecting receipt pruning",
            ));
        }
        Request::Code { owner, minutes } => {
            let ttl = Duration::from_secs(minutes.saturating_mul(60));
            let (code, expires_ms) = svc.issue_code_for(owner, ttl)?;
            Response::Code { code, expires_ms }
        }
        Request::List => Response::List {
            machines: svc
                .all_machines()?
                .into_iter()
                .map(|m| MachineSummary {
                    id: m.id,
                    owner: m.owner,
                    name: m.name,
                    os: m.os,
                    arch: m.arch,
                    revoked: m.revoked,
                    draining: m.draining,
                })
                .collect(),
            erasures: svc.pending_erasures()?,
        },
        Request::Revoke { machine } => Response::Revoke {
            revoked: svc.revoke_any(machine)?,
        },
        Request::PoolGrant { account, granted } => {
            svc.set_pool_grant(account, granted)?;
            Response::PoolGrant
        }
    })
}

/// Unix socket paths are limited to about 108 bytes.
const MAX_SOCKET_PATH: usize = 100;

/// The admin socket of an appdata root: `<appdata>/system/admin.sock`, or, when
/// that path is too long for a Unix socket, `calternal-admin-<uid>/<hash>.sock` in
/// the temp directory, named from the canonical appdata path. The server and the
/// command compute it the same way. Cost: one canonicalize and, rarely, a hash.
pub fn socket_path(appdata: &Path) -> PathBuf {
    let appdata = appdata
        .canonicalize()
        .unwrap_or_else(|_| appdata.to_owned());
    let near = appdata.join("system").join("admin.sock");
    if near.as_os_str().len() <= MAX_SOCKET_PATH {
        return near;
    }
    let hash = crate::store::sha256_hex(appdata.as_os_str().as_encoded_bytes());
    std::env::temp_dir()
        .join(format!(
            "calternal-admin-{}",
            rustix::process::getuid().as_raw()
        ))
        .join(format!("{}.sock", &hash[..32]))
}

/// Open an installation whose server is not running, for operator commands:
/// the root guard first (pinned `installation`, all three roots, nothing created),
/// then the recovery log, then `auth.db` with recovery attached, then the machine
/// service. The returned [`Store`] holds the root locks; keep it alive while the
/// service is used. Blocks: startup-sized I/O.
///
/// # Errors
/// Unavailable when a root is refused or locked (a server is starting), or when
/// the recovery log cannot be opened; nothing is changed then.
pub fn open_offline(
    roots: &calternal_roots::Roots,
    installation: Uuid,
) -> Result<(Store, Arc<MachineService>), Error> {
    let store = Store::open(
        roots,
        installation,
        false,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )?;
    let svc = service_for(&store)?;
    Ok((store, svc))
}

/// The machine service of an opened (guarded) store, with recovery attached to
/// its authority before any change. Blocks: recovery and database opens.
pub fn service_for(store: &Store) -> Result<Arc<MachineService>, Error> {
    let recovery = store.open_recovery(false)?;
    let spaces = calternal_auth::spaces::Spaces::open(&store.appdata().join("system/auth.db"))
        .map_err(authority_error)?;
    spaces
        .configure_recovery(Arc::new(recovery))
        .map_err(authority_error)?;
    MachineService::open(store.appdata(), spaces.machines(), Settings::default())
}

/// Send one request to the running server of `appdata`. `Ok(None)` when no
/// server listens there (no socket, or a stale one). Blocks: one connection and
/// two bounded lines.
pub fn ask_server(
    appdata: &Path,
    installation: Uuid,
    request: Request,
) -> Result<Option<Response>, Error> {
    use std::os::unix::net::UnixStream;
    let mut stream = match UnixStream::connect(socket_path(appdata)) {
        Ok(s) => s,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(Error::unavailable(format!("admin socket: {e}"))),
    };
    let io = |e: std::io::Error| Error::unavailable(format!("admin socket: {e}"));
    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(io)?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).map_err(io)?;
    let mut line = serde_json::to_vec(&Envelope {
        installation,
        request,
    })
    .map_err(|_| Error::internal("encode"))?;
    line.push(b'\n');
    stream.write_all(&line).map_err(io)?;
    let mut answer = Vec::new();
    BufReader::new(std::io::Read::take(&stream, MAX_LINE))
        .read_until(b'\n', &mut answer)
        .map_err(io)?;
    let reply: Reply = serde_json::from_slice(&answer)
        .map_err(|_| Error::unavailable("the server's admin socket gave no answer"))?;
    match (reply.ok, reply.data, reply.error) {
        (true, Some(data), _) => Ok(Some(data)),
        (false, _, Some(error)) => Err(error),
        _ => Err(Error::unavailable(
            "the server's admin socket gave no answer",
        )),
    }
}

/// Listen on the appdata's [`socket_path`] for operator requests until the
/// returned task is aborted. Called once by the server after the root guard and
/// recovery are set up; a stale socket file is replaced (the guard proves no other
/// server owns this appdata). Each connection: peer UID check, one bounded line
/// read with a timeout, [`execute`] on a blocking task, one line written.
/// Receipt status uses the live content service for a read-only inspection.
///
/// # Errors
/// I/O errors binding the socket or setting its mode.
pub fn serve_socket(
    appdata: &Path,
    installation: Uuid,
    svc: Arc<MachineService>,
    owner_setup: Option<Arc<crate::owner_setup::OwnerSetup>>,
    content: Arc<Content>,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    use std::os::unix::fs::PermissionsExt;
    let path = socket_path(appdata);
    if let Some(dir) = path.parent() {
        // Owner-only (0700); refuses a directory another user planted.
        calternal_fsio::private::directory(dir)?;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = tokio::net::UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let uid = rustix::process::getuid().as_raw();
    Ok(tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            if stream.peer_cred().map(|c| c.uid()).ok() != Some(uid) {
                continue;
            }
            let svc = svc.clone();
            let owner_setup = owner_setup.clone();
            let content = content.clone();
            tokio::spawn(async move {
                if let Err(e) = answer(stream, installation, svc, owner_setup, content).await {
                    tracing::warn!(error = %e, "admin socket request failed");
                }
            });
        }
    }))
}

async fn answer(
    stream: tokio::net::UnixStream,
    installation: Uuid,
    svc: Arc<MachineService>,
    owner_setup: Option<Arc<crate::owner_setup::OwnerSetup>>,
    content: Arc<Content>,
) -> std::io::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
    let (read, mut write) = stream.into_split();
    let mut line = Vec::new();
    tokio::time::timeout(
        IO_TIMEOUT,
        tokio::io::BufReader::new(read.take(MAX_LINE)).read_until(b'\n', &mut line),
    )
    .await
    .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))??;
    let result = match serde_json::from_slice::<Envelope>(&line) {
        Err(_) => Err(Error::invalid_input("malformed admin request")),
        Ok(e) if e.installation != installation => Err(Error::invalid_input(
            "this server runs a different installation; check --installation-id",
        )),
        Ok(e) if matches!(e.request, Request::SetupCode) => match owner_setup {
            Some(setup) => setup.issue(true).await.map(|grant| {
                let url = setup.url().to_owned();
                match grant {
                    Some(grant) => Response::SetupCode {
                        code: grant.code,
                        expires_ms: grant.expires_ms,
                        url,
                    },
                    None => Response::SetupComplete { url },
                }
            }),
            None => Err(Error::unavailable(
                "passkey onboarding is unavailable on this server",
            )),
        },
        Ok(Envelope {
            request: Request::AccountRecover { account, reason },
            ..
        }) => match owner_setup {
            Some(setup) => setup
                .recover(account, &reason)
                .await
                .map(|enrollment_grant| Response::AccountRecovery {
                    enrollment_grant,
                    url: setup.url().to_owned(),
                }),
            None => Err(Error::unavailable("account recovery is unavailable")),
        },
        Ok(e) => crate::blocking::spawn(move || match e.request {
            Request::ReceiptStatus { space } => content
                .receipt_pruning_issue(space)
                .map(|issue| Response::ReceiptStatus { issue }),
            request => execute(&svc, request),
        })
        .await
        .unwrap_or_else(|_| Err(Error::internal("admin task failed"))),
    };
    let reply = match result {
        Ok(data) => Reply {
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => Reply {
            ok: false,
            data: None,
            error: Some(error),
        },
    };
    let mut out = serde_json::to_vec(&reply).map_err(std::io::Error::other)?;
    out.push(b'\n');
    tokio::time::timeout(IO_TIMEOUT, write.write_all(&out))
        .await
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))?
}
