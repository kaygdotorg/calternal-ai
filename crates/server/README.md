# calternal-server: localhost development-preview host

Development preview with protected account sign-in and stored space authority.
Use disposable data. The preview content store does not provide inherited tiering,
a rebuild from the recovery log, or production readiness. Plugin workers use
the machine role’s confined worker profile; see [worker execution](../../docs/plugin-workers.md).

## Run

```sh
scripts/with-toolchain.sh cargo build -p calternal-server -j4
preview_data=$(mktemp -d /tmp/calternal-preview.XXXXXX)
export CALTERNAL_INSTALLATION_ID=$(cat /proc/sys/kernel/random/uuid)
target/debug/calternal serve --create --port 0 --plugin-root plugins \
  --appdata "$preview_data/appdata" \
  --userdata-ssd "$preview_data/userdata-ssd" \
  --userdata-hdd "$preview_data/userdata-hdd"
```

Startup first validates and locks the three roots (`calternal-roots`). The
installation ID is pinned in server configuration outside every root (ADR 0062):
`CALTERNAL_INSTALLATION_ID` (or `--installation-id`); a missing or malformed pin
refuses to start. Each root's `.calternal-install-id` marker must carry that ID and
the root's role. First setup: generate a UUID, store it in the configuration, and
start once with `--create`, which pairs three empty or missing roots by writing the
markers. Missing,
swapped, nested, occupied-but-unpaired or foreign roots are refused before any
directory, lock or database is created; `--create` only pairs empty or missing
roots. The protected security log (`calternal-recovery`) lives in
`userdata-ssd/.calternal/recovery` and is attached to `auth.db` before any request
or fixture seeding, so every security change is durable there before it is
acknowledged. The operator `machines` commands take the same three roots and installation
ID; they ask a running server over its admin socket, or open a stopped installation
through the same guard and recovery log.

Request receipts (what a retry with the same request ID replays) are kept 7 days by
default; `--receipt-retention-days` / `CALTERNAL_RECEIPT_RETENTION_DAYS` changes it.
A retry after that is treated as a new request.

The listener binds `127.0.0.1`. `appdata/startup.json` (0600) reports `base_url`,
`passkey_origin`, `setup_token_file`, `development_tokens`, `pid`, and token file
paths. The default passkey origin is `http://localhost:<bound-port>`: WebAuthn
needs a hostname. Use the reported URL and preserve the port across restarts to
preserve the passkey origin. `--origin https://example.org` selects an exact
public origin behind a TLS proxy. `--trusted-proxies` accepts explicit CIDRs;
without it, forwarded client addresses never influence rate-limit buckets.
Only the configured public authority and the listener's loopback authorities
are accepted as Host values. TLS is supplied by the deployment proxy.

With no accounts, the privileged local setup token is written to `setup.token`
(mode 0600) and its path appears in startup metadata. Its value is never logged.
Redeem it through setup/start and setup/finish below. Enrollment creates an account,
not a space; use `spaces.create` after signing in. An account's server admin role
grants no content access.

`--development` explicitly enables disposable seeded accounts and development
bearer files; this flag is **off by default**, including after restart. Existing
token files do not grant authority without this flag. Development `base_url`
retains the preview's numeric loopback URL for existing bearer clients; use
`passkey_origin` for browser passkey sign-in. The `owner` fixture owns
two spaces named `personal` and `shared`; `guest` reads only `shared`. Each account
and each space has an independent UUIDv7, and requests name spaces only by UUID:
clients read them from `describe`. Token files are 0600 under a 0700 `dev-tokens`
directory.

## HTTP

Trusted account endpoints use `/api/v1/auth/` and bare JSON results:

| Method/path | Input or result |
|---|---|
| GET status | `{setup, signed_in}`, no directory information |
| POST setup/start | `{token, name?}` -> `{ceremony, options}` |
| POST setup/finish | `{ceremony, credential, device?}` -> user and host cookie |
| POST login/start | empty body or JSON -> discoverable options; no username |
| POST login/finish | `{ceremony, credential, device?}` -> user and host cookie |
| POST reverify/start, reverify/finish | session-account/purpose-bound challenge, same finish shape |
| POST logout | revokes current installation; 204, clears cache; never clears a newer cookie |
| GET me | private profile/server capabilities and installation ID |
| GET installations | current account's devices, with `current` flags |
| POST installations/revoke | `{installation}`; foreign and absent IDs both not found |
| GET passkeys | current account's passkeys |
| POST passkeys/add/start | `{}`, requires recent interactive proof |
| POST passkeys/add/finish | `{ceremony, credential, name?}`, rechecks proof |
| POST passkeys/rename | `{id, name}` |
| POST passkeys/remove | `{id}`, requires recent proof; never removes the last key |

Account POSTs require JSON and the exact configured Origin when present. Bodies
are capped at 64 KiB. Source and target admission, bounded purpose-specific
ceremony pools, account signature-failure backoff, token hashes and current
revocation checks come from the ported auth library. Cookies are
`__Host-calternal_session`, `Path=/; HttpOnly; SameSite=Lax; Secure`, without a
Domain. Duplicate host cookies are refused across the merged router. No legacy cookie is recognized or cleared; delayed logout/refusal never expires
a newer host cookie. Native clients may send a session bearer. DAV/app passwords are not
accepted. Account mutation receipt replay and private live event feeds are not
yet integrated (see docs/reviews/server-porting-2026-11-17.md).

`GET /api/v1/describe` returns `DescribeResult`. `POST /api/v1/operations` takes
`{request_id, operation, input}` and returns the shared operation envelope. A
session supplies only account identity; current Directory capability rows decide
each operation, retry, commit and asynchronous delivery.

`resolve {kind, id}` resolves `/<kind>/<id>` to `{kind,id,space_id,access}`. The
current preview has only the `notes` item kind. Wrong kind, missing item and no
access return identical 404 `not_found`/`item not found`. Resolution currently
probes current membership roots, without enumerating content; the future storage
index can replace this linear-in-memberships lookup without changing clients.
`notes.get`, `notes.update`, `notes.history` and `notes-insights.summarize` accept
just an item ID (plus their own fields). A supplied compatibility `space_id` must
match the resolved owner. Collections and creation still require a space.
The CLI's get/update/history commands omit the hint by default.

Every response retains CSP, nosniff, no-referrer, DENY framing and API no-store.
There are no CORS headers. `--web-dir` serves a plain static directory with no
SPA fallback; unknown paths and API routes return 404.
Core operation bodies remain capped at 2 MiB, Markdown at 1 MiB. The server
supports the settled addresses; the current web router still needs Claude-owned
changes listed in docs/reviews/server-http-validation-2026-11-17.md.

## Durability profile (preview)

Each space's `appdata/spaces/<space-uuid>/content.db` (WAL, `synchronous=FULL`) is authoritative for its revisions and history, and its `index.db` journal for its request receipts; plugin activation is a control value in that space's `content.db`, so no installation-wide database is written for it. Reads come from SQLite. `userdata-ssd/spaces/<space-uuid>/Notes/<title>.md` is a checked, portable copy of each note's head revision, named from its title, with the item ID in its frontmatter.

A write proceeds as follows:

1. **Commit.** One transaction commits the full revision, the receipt `(principal, request_id) → operation, SHA-256 fingerprint of canonical input, result` and `mat_state=pending`.
2. **Check the existing file.** It must be missing on a first write, or match the SHA-256 recorded at the last placement. Symlinks and non-regular files never pass.
3. **Place the new bytes.** Write a temp file, fsync it, rename it over the target, then fsync the directory. Newly created directory ancestry is also fsynced.
4. **Mark and acknowledge.** Set `mat_state=materialized` with the new hash, then return the acknowledgement.

Recovery and retries:

- **Startup.** The server takes an exclusive lock on the data directory (a second instance exits with code 2). It removes only its own `.<uuid>.<12hex>.tmp` files, then replays pending notes before serving.
- **Crash after rename.** If replay finds the new bytes already in place, it fsyncs the file and the directory before marking.
- **Retry with the same input.** It returns the original result, finishing any pending placement. It is reauthorized against current grants and does not re-run the old base-revision check.
- **Retry with different input.** It returns 409 `request_id_reused`.
- **Plugin enable/disable.** These are receipted in the same way, so retrying an old enable cannot undo a later disable.
- **Mutating plugin operations.** Receipted after delivery: a retry with the same request ID and input returns the original result without running the worker; different input is 409. Each brokered `write_item` also carries its own receipt (`plugin/<plugin>/<request>/<n>`), so a retry after an undelivered result replays those writes instead of duplicating them.

**External changes.** A changed, deleted or replaced note file is never overwritten. The note is blocked: updates return 409 with `details.reason = "external_change"`, the external bytes are preserved, and reads still serve the SQLite revision. There is no import and no unblock command tonight. The hash check detects changes that happened before a write. It does not lock out an external writer that races the check and the rename. Concurrent external editing is out of scope.

**Crash tests.** In debug builds only, `CALTERNAL_FAULT=after_commit|before_placement|after_placement|before_reply` aborts the process at that point. `tests/http.rs` kills the server at each point, restarts it, and checks the placed bytes, the idempotent retry, that history has no duplicates and that no temp files are left.

## Plugins

Code: `src/plugins/` (registry, `broker.rs`, `worker.rs`) over the `calternal-plugin-host` library.

- **Registry.** Only entries in the compiled-in `trusted-plugins.json` load. Each manifest (v1) is read once at startup, parsed and validated by the host library without running plugin code, checked against the entry's id and version, and registered atomically: kind, operation, collection and visible-folder collisions are refused. A plugin operation that would shadow a host operation, or that needs an account-level capability, keeps the plugin out.
- **Policy.** Plugin operation rows are derived from the manifests (`required_capability`), never hand-written. Host rows (`authz::POLICIES`) and plugin rows go through the same `Registry::policy` lookup and the same `Policy::authorize` path. An operation with no row answers like a hidden target (`item not found` / `space not found`), so registration is not observable to non-members.
- **Operation targets.** A manifest operation may declare `target: { kind, field }`: `field` is a required string property of its input schema holding the item ID. The existence-hiding resolver finds the item's space from `(kind, id)` and the caller is authorized there; a missing, foreign or wrong-kind ID answers the same not-found. The plugin receives its own input (the optional `space_id` hint is removed). Operations without a target are space-scoped: the wire input carries `space_id`, which is removed before the rest is passed to the plugin.
- **Digest.** Each worker bundle's SHA-256 is pinned and verified at startup and before every worker start.
- **Workers (protocol v2).** One long-lived Node process per (plugin, space), driven by the library's `WorkerSession` over enrolled machines and their existing sandbox/exec channel. The server starts no worker process. [Worker execution](../../docs/plugin-workers.md) specifies the profile and lifecycle:
  - read-only code/root, no network or socket creation, no appdata/userdata mounts, 256 MiB, one CPU, 64 PIDs and 128 MiB V8 heap
  - eight physical workers including startup and teardown, 32 in-flight invocations, 256 KiB frames, three-second operations (including cold launch after admission), 600-second job runs, 4096-call retirement and 60-second idle shutdown
  - a protocol violation, output overflow or missed deadline fails its callers; replacement waits for authenticated teardown proof
- **Host calls.** Only manifest-declared calls are accepted; each is brokered with the invocation's authority rechecked (current capability, plugin still enabled at the admitted generation) before and after the effect and before delivery. `read_item` reads any kind in the invocation's space only; `write_item` writes only the plugin's own kinds and needs `content.write`. Both go through `crates/content`. `start_job` and `checkpoint_job` go to the job service and `request_secret_handle` to `calternal-secrets` (owner: the space UUID). The other declared calls are refused until their host services exist.
- **Enable.** `plugins.enable` registers each declared kind (visible folder or `.calternal/<plugin>/<kind>/`; claimed text as Markdown; any other kind stored unparsed under its declared `extension`) and creates each standard collection with a stable ID, before activation commits.
- **Delivery.** After the worker answers, the host checks the plugin is still enabled at the same generation (409 `plugin_generation_changed`, `tests/race.rs`) and rechecks the caller's space access; the HTTP guard rechecks the session. A result's GenUI view must pass `validate_extension_view`.
- **Preview.** Runs the plugin's first targeted operation against a fixed fixture item served by a broker that refuses everything else; works while the plugin is disabled and never activates it.
- **Jobs.** Code: `src/plugins/{jobs,job_sql,live,runner,job_api}.rs`. A worker's `start_job { operation, input, parent_id? }` durably admits a job in the space's own `appdata/spaces/<space-uuid>/jobs.db` before the ID is returned; the admission is keyed by request (retries return the same job). The runner invokes the declared run operation with `{ job_id, input, checkpoint, children }`; it answers `{status:"done", result}` or `{status:"waiting"}` (until its children report). `checkpoint_job { job_id, checkpoint }` is durable; chunks are not: each run has an epoch and a bounded buffer (256 chunks / 1 MiB, 16 KiB per chunk), with sequence numbers continuing from the checkpoint. States: admitted, running, waiting, done, failed, cancelled, outcome_unknown. On restart, running jobs resume from their checkpoint, or end `outcome_unknown` when their run operation is `idempotency: never`. Each child reports exactly once to its parent. Write receipts inside a run are scoped to (job, checkpoint count), so a resumed run replays its writes rather than repeating them.
- **Job surface.** `jobs.get` / `jobs.cancel { job_id }` (cancel: the starter or `plugins.manage`; cascades to active children; later output is discarded) and `GET /api/v1/jobs/<id>/events` (server-sent events: `snapshot`, `chunk`, `reset`, `end`; resume with `?after=<epoch>:<seq>` or `Last-Event-ID`). SSE because the stream is one-way and needs no upgrade support in the server's axum feature set. Subscribing and every delivery recheck sign-in, `space.read` and enablement at the job's generation; a job outside the caller's spaces is the same 404 as a missing one.
- **Job bounds.** 64 active jobs per space, 32 children per parent, 64 KiB checkpoints/inputs/results, 64 runs per job, 4 concurrent runs server-wide (10 min deadline each), 8 subscribers per job and 256 in total, 64 open job files; final jobs are kept 7 days and at most 1024 per space.
- **Sign-in.** Every host call, job run and delivery rechecks that the caller's installation is still live (not signed out, revoked or expired); a job is bound to the installation that started it.
- **Test plugins.** `tests/plugins/jobs-demo` is trusted only by a build with the `test-plugins` feature and `--test-plugin-root tests/plugins`.
- **Boundary.** Container probes establish this development profile’s OS restrictions; they do not qualify multi-tenant microVM placement or production readiness. Explicit direct development mode has no OS isolation. Workers retain in-memory state within their (plugin, space).

## Machines

Agent threads run on enrolled machines that connect out to this server; the server
never starts containers or VMs (`src/machines.rs`; machine side in `crates/machine`).
Machine authority (enrollment code hashes, machines with their public keys and
revocation, pool grants) lives in `auth.db` and is published to the recovery log
before it is acknowledged (`calternal_auth::machines`). Placement state (leases,
draining) lives in `system/machines.db` (0600, WAL, `synchronous=FULL`). Exec output
is kept in memory only. `serve` takes `--machine-lease-ttl-ms` (default 90000) and
`--machine-poll-hold-ms` (default 20000).

| Path | Who | Purpose |
|---|---|---|
| `/api/v1/machine/{enroll,session/start,session/finish,sync,output}` | the machine (code, key, then bearer) | protocol v1, see `calternal_machine::protocol` |
| `POST /api/v1/machines/codes`, `GET /api/v1/machines`, `POST /api/v1/machines/{id}/revoke`, `POST /api/v1/machines/{id}/drain` | signed-in owner | enroll and manage your machines |
| `POST /api/v1/machines/runs`, `GET …/runs/{lease}`, `POST …/runs/{lease}/stop`, `POST …/runs/{lease}/execs`, `GET /api/v1/machines/execs/{exec}/output?after&wait_ms` | account with `plugins.use` in the space | place work, run commands, read output |

Operator commands name the installation (three roots and installation ID). A running
server receives them over its owner-only admin socket and applies them at once; with
no server running they open the installation through the storage guard:
`calternal machines code --pool|--account <uuid>`, `machines list`,
`machines revoke <machine>`, `machines pool-grant <account> [--remove]`.
Every command prints a readable table or block, or one JSON document with `--json`:
see [`docs/cli.md`](../../docs/cli.md).

## Tests

`scripts/with-toolchain.sh cargo test -p calternal-server -j4` runs these suites. They need Node 22.13 or later on PATH.

- **`tests/plugin_host.rs`:** manifest-derived policy rows, enable provisioning of kinds and collections, an undeclared host call refused before any effect, one long-lived worker per (plugin, space), cross-space reads, cleared environment and filesystem confinement, deadline/overflow/malformed/schema-violating output, pool capacity, digest tampering.
- **`tests/http.rs` (black-box tests of the real binary):** two principals, stale writes and history, retries, crash points, external edits, the lock, Host/Origin/bearer/CSP checks, static fallback, plugin receipts, temp-file cleanup.
- **`tests/race.rs`:** disabling a plugin while a call is in flight.

Timings from these tests are development-host smoke numbers, not performance gates.

Manual provider jobs and their CLI, scoped credentials, risk review, machine homes
and durable retry waits are documented in [manual turns](../../docs/agent-turns.md).
The provider test suite uses an account-free fake official CLI.

`GET /api/v1/live?subscriptions=<JSON array>` follows 1–16 distinct space UUIDs.
Each array element is `{ "space_id": "<uuid>", "bookmark": null | "<opaque>" }`.
The authenticated feed emits SSE `data` containing core `LiveFrame` values:
`entry`, `ready` and explicit `reset`. SSE matches the existing jobs stream.
Journal broadcast receivers are registered before replay; database pages are the
source of every delivery, including catch-up after lag. The stream checks current
session and space grants for every event and uses the selected per-space database for current item
binding. Unknown and inaccessible spaces have identical 404 responses. In-process
auth changes wake idle streams; a one-second fallback checks expiry and directory
changes through separately opened handles. This is a development preview; no
cross-process instantaneous revocation guarantee is asserted. Each space has a
128-subscriber safety ceiling and each connection buffers one 64-entry page.
Account-private drafts, pending submissions and `/api/v1/account/live` are
implemented in separate owner-local modules; see [account input](../../docs/account-input.md).
The live space/subscriber defaults above are operator-configurable through
`CALTERNAL_LIVE_SPACES` and `CALTERNAL_LIVE_LISTENERS`.

Saved conversations are described in [threads](../../docs/threads.md). The orchestrator manifest owns their operations; the host reuses generic content storage and the existing turn engine. Active-turn steering and large-thread indexing remain named gaps there.
