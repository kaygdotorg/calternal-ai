# Machine role lane report, 2026-11-17

Lane `lane/machine`, issue kayg/calternal-ai#10. Development preview: this is not
proof of hostile-workload isolation (the process backend isolates nothing; the
Podman backend is rootless containers, not microVMs).

## What exists

- `crates/machine` (`calternal-machine`): protocol v1, machine key and credentials,
  enrolment, the reconnecting long-poll loop with local lease expiry, the runner, and
  the sandbox trait with rootless Podman and process backends. `calternal machine enrol
  | run | status`.
- `crates/server/src/machines.rs` plus `machines/{state,sessions,db,http,admin}.rs`:
  code issuance and redemption, signed sessions, placement by capacity, fenced leases,
  revocation, draining, the sweeper, and exec output buffers. `calternal machines code
  | list | revoke | pool-grant` for the operator.
- `scripts/install-machine.sh`: copies the binary, enrols, and installs a systemd user
  unit (Linux, tested) or a launchd agent (macOS, written but not run).

## Decisions taken inside the lane (documents did not settle them; reversible)

- Transport: long-poll (reasons in `crates/machine/src/protocol.rs`).
- Machine authentication: an Ed25519 key made on the machine, with a challenge-signed
  session. `ring` was already in the lock file through rustls.
- Enrolment codes reuse the auth setup-token format and hashing.
- Runs need `plugins.use` in the space.
- An expired lease is placed again (fenced), not ended. Revocation ends leases.

## Review gate

1. Shared types: `calternal_auth::{AccountId, SpaceId}` for accounts and spaces,
   `HostError` for every caller-safe error, `calternal_auth::token::{random32, hash32,
   new_setup_token, normalize_setup_token}` for secrets. Two exceptions, both open
   below: `db::now_ms` (auth and store keep private copies; there is no shared one yet),
   and `identity::create_private_dir` in the machine crate (a 0700 mkdir), because
   depending on `calternal-auth` for `private_fs` would pull the whole auth crate onto
   machines. The machine crate's own `Error` is a client-side error, not a caller-safe
   server error.
2. Dependencies point downward: server → machine (protocol types and the subcommand) →
   fsio. New dependencies are `ring` and `base64` 0.22, both already in the lock.
   The server imports `private_fs` with a single `use calternal_auth::private_fs;`
   line, so the coming move to fsio changes one line.
3. One registry mutex: machines are installation-wide, not per-space. It is held for
   microseconds, or across one fsync on enrolment, placement, lease change and
   revocation. This is stated in the module docs.
4. No blocking on async threads: every registry or SQLite call from HTTP goes through
   `spawn_blocking`. `sync` and `read_output` only await a `Notify`. The poll guard
   releases on a blocking task. On the machine, file I/O happens at start and enrolment
   only.
5. Hot paths: a quiet `sync` makes 0 file reads and 0 flushes. An `output` upload makes
   0 SQLite calls. Reading output makes 0. The sweeper does one `PRAGMA data_version`
   read per second. Placement and lease changes cost one commit (fsync) each.
6. Nothing constant is rebuilt: SQL is static text and there is no schema validator.
7. Bounds and retention: challenges at most 1024 for 60 s; one session per machine;
   exec output at most 1 MiB unread per exec, dropped 10 minutes after exit; enrolment
   codes deleted a day after expiry; ended leases deleted after 7 days. Machine rows
   (revoked ones included) are kept indefinitely: this needs the owner's decision (Open).
8. No compatibility code: schema version 1, with no migrations.
9. Spaces by UUID only: `space_id` is parsed as a `SpaceId`, so aliases such as
   `personal` are rejected.
10. One place per fact: the protocol types are defined once, in `calternal_machine::protocol`.
    The capability check exists once (`http::authorize`).
11. Every public item has a non-test caller, except `MachineService::{run_space,
    exec_space}`, which HTTP calls. The jobs lane will call `request_run`, `exec`,
    `read_output`, `run` and `stop_run` in place of the HTTP endpoints.
12. Reuse: `calternal_fsio::write_atomic_mode` for credentials,
    `calternal_auth::private_fs` for the database, auth token primitives, `HostError`
    and `api::failure`/`json_response` (made `pub(crate)`), and the guard's existing
    exemption pattern for account routes.
13. The docs gate (`scripts/check-docs.sh`) now covers `calternal-machine` and
    `calternal-server`. `cargo doc -D warnings` is clean for both.
14. Docs: `crates/machine/README.md`, the server README's Machines section, and this
    report.
15. `rustfmt --check` is clean on every file this lane added, and on `main.rs`. `api.rs`
    already had rustfmt differences outside this lane's two edits, and I left them alone.
    No file exceeds 600 lines (the largest, `machines.rs`, is 587).

## Verification (calternal-dev-01-compute02)

- `cargo test -p calternal-machine`: 5 unit tests, plus 2 Podman tests. Podman 5.8.9
  and `alpine:3` were present, so the Podman tests ran rather than skipped.
- `cargo test -p calternal-server --test machines --test machines_service`: 9 tests.
  They cover enrolment replay and expiry, other accounts, the pool grant, revocation
  mid-run, fencing after a partition with reassignment, kill and restart of both roles,
  stale fence, wrong machine, a forged signature, and back-pressure.
- `scripts/install-machine.sh` against a throwaway server: enrolled, the unit was
  active, the machine was online, and a re-run was idempotent. Everything was removed
  afterwards.

## Secrets and conformance (after dev merged lane/secrets)

- Secrets seam, documented but not wired: exec `env` is plain configuration only. Secret
  values are to reach a sandbox only through `SecretStore::use_value` with
  `Consumer::SandboxEnv { thread, variable }`, released right before the `Exec` message
  is queued for the machine (`MachineService::exec` docs, `protocol::Down::Exec`). Wiring
  it is one exec-input field plus a store reference, once the secrets lane says how the
  server holds its `SecretStore`.
- `tests/conformance/isolation.rs::machines_and_runs_are_owner_private` covers every
  machine route, with a source inventory guard over `machines/http.rs`. It checks that a
  foreign id gives the same answer as a missing one for accounts 0, 2 and 3; that
  anonymous callers get 401; that a code works once; that a forged session signature is
  refused; that a bad bearer gets 401; that runs need `plugins.use`; that another
  account's machine is not used; and that revocation is immediate. The suite now depends
  on `calternal-machine` for the Ed25519 key, which adds rustls and ring to its lock file.
  The full suite passes: 29 of 29.

## Open

- Who may use pool machines: implemented as explicit per-account operator grants (fail
  closed). Alternatives are every account, or an entitlement from the coming paid tier.
- How leases expire: implemented as a 90 s TTL from the last sync, after which the lease
  is fenced and placed again with same-machine affinity. Alternatives are ending the run
  instead, or waiting for that one machine indefinitely, which keeps its files.
- What a user sees while waiting: the `waiting` state with a queue position, an empty
  reason, or "the machine stopped responding". There is no ETA and no notification yet.
- Machine row retention after revocation.
- Shared `now_ms` and `private_fs` in fsio, to remove the two local copies.
