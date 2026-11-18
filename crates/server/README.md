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

Notes, Tags, Trash and directory continuations bind to the account, operation and
filters. Cursors carry a signed position and expire on restart. Approval entries
in the private request inbox and feed carry the lifetimes the host accepts for
that action.

The listener defaults to `127.0.0.1`. Use `--bind-ip` to configure an isolated container listener. `appdata/startup.json` (0600) reports `base_url`,
`passkey_origin`, `setup_token_file`, `development_tokens`, `pid`, and token file
paths. The default passkey origin is `http://localhost:<bound-port>`: WebAuthn
needs a hostname. Use the reported URL and preserve the port across restarts to
preserve the passkey origin. `--origin https://example.org` (or `CALTERNAL_ORIGIN`) selects an exact
public origin behind a TLS proxy. `--trusted-proxies` (or `CALTERNAL_TRUSTED_PROXIES`) accepts explicit CIDRs;
without it, forwarded client addresses never influence rate-limit buckets.
Only the configured public authority and the listener's loopback authorities
are accepted as Host values. TLS is supplied by the deployment proxy.

`--machine-sandbox-cpu-millis` sets a per-agent CPU ceiling on machines
that enforce sandbox CPU limits. The default leaves CPU uncapped. See the
[deployment runbook](../../docs/deploy.md) for the packaged limits.

With no accounts, the privileged local setup token is written to `setup.token`
(mode 0600) and its path appears in startup metadata. Its value is never logged.
Redeem it through setup/start and setup/finish below. Enrollment creates an account,
not a space; use `spaces.create` after signing in. An account's server admin role
grants no content access.

`--development` explicitly enables disposable seeded accounts and development
bearer files; this flag is **off by default**, including after restart. Existing
token files do not grant authority without this flag. Development `base_url`
retains the preview's numeric loopback URL for existing bearer clients; use
`passkey_origin` for browser passkey sign-in. Each fixture account owns its
personal space. The `owner` also owns `shared`, where `guest` is a Reader. All
account and space UUIDv7s are independent. Clients identify the personal space
by the caller-relative `personal` flag on a `spaces.list` row. Token files are
0600 under a 0700 `dev-tokens` directory.

First-owner enrollment and privileged account creation insert the account,
personal space and Owner membership in one protected authority transaction.
Passkey recovery attaches a credential to the existing account and space.
The preview has no invite-redemption route.

Account and space erasure use the same protected authority cutover and
owner-local lifecycle. The storage and HTTP contract is in
[Owner erasure](../../docs/owner-erasure.md).

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
There are no CORS headers. `--web-dir` serves existing static files first.
A missing GET or HEAD on a
[Calternal address](../../docs/addresses.md) serves the app shell. Missing
assets and API routes still return 404.
Core operation bodies remain capped at 2 MiB, Markdown at 1 MiB. The server
supports the settled addresses; the current web router still needs Claude-owned
changes listed in docs/reviews/server-http-validation-2026-11-17.md.

## Durability profile (preview)

Each space's `appdata/spaces/<space-uuid>/content.db` (WAL, `synchronous=FULL`) is authoritative for its revisions and history, and its `index.db` journal for its request receipts; plugin activation is a control value in that space's `content.db`, so no installation-wide database is written for it. Reads come from SQLite. `userdata-ssd/spaces/<space-uuid>/Notes/<title>.md` is a checked, portable copy of each note's head revision, named from its title, with the item ID in its frontmatter.

A write proceeds as follows:

1. **Commit.** One transaction commits the full revision, the receipt `(principal, request_id) → operation, SHA-256 fingerprint of canonical input, result` and `mat_state=pending`.
2. **Check the existing file.** It must be missing on a first write, or match the BLAKE3 recorded at the last placement. Symlinks and non-regular files never pass.
3. **Acknowledge durable SSD state.** Content/history, journal/receipt and outbox retirement are durable before the response. Retirement stays FULL so a power loss cannot resurrect an intent after its receipt expires.
4. **Place and mark within one second.** The existing content lifecycle worker wakes after commit, writes a temp file, fsyncs it, renames it over the target, and fsyncs the directory. Newly created ancestry is also fsynced. It marks `mat_state=materialized` with the new hash using NORMAL, replayable bookkeeping. The oldest pending age never resets; overdue placement blocks further writes until the disk recovers. Pending slots pin the existing 64-space cache, and the worker pins installation root locks during I/O.

Recovery and retries:

- **Startup.** The server locks installation roots and completes authority/recovery reconciliation before serving. Content stores open lazily; before first use of each space they remove only owned `.<uuid>.<12hex>.tmp` files, publish pending intents, then recover pending placement. Inactive spaces do not delay readiness.
- **Crash after rename.** If replay finds the new bytes already in place, it fsyncs the file and the directory before marking.
- **Retry with the same input.** It returns the original durable SSD result; a later userdata placement failure cannot change that result. It is reauthorized against current grants and does not re-run the old base-revision check.
- **Retry with different input.** It returns 409 `request_id_reused`.
- **Plugin enable/disable.** These are receipted in the same way, so retrying an old enable cannot undo a later disable.
- **Mutating plugin operations.** Receipted after delivery: a retry with the same request ID and input returns the original result without running the worker; different input is 409. Each brokered `write_item` also carries its own receipt (`plugin/<plugin>/<request>/<n>`), so a retry after an undelivered result replays those writes instead of duplicating them.

**External changes.** A changed, deleted or replaced note file is never overwritten. The note is blocked: updates return 409 with `details.reason = "external_change"`, the external bytes are preserved, and reads still serve the SQLite revision. There is no import and no unblock command tonight. The hash check detects changes that happened before a write. It does not lock out an external writer that races the check and the rename. Concurrent external editing is out of scope.

**Crash tests.** In debug builds only, `CALTERNAL_FAULT=after_commit|before_placement|after_placement|before_reply` aborts the process at that point. `tests/http.rs` kills the server at each point, restarts it, and checks the placed bytes, the idempotent retry, that history has no duplicates and that no temp files are left.

## Plugins

Code: `src/plugins/` (registry, `broker.rs`, `worker.rs`) over the `calternal-plugin-host` library.

- **Registry.** The distribution seed populates the installation-local package catalog at first start. An operator can review, install, update and remove packages while the server runs. The private operator socket publishes an immutable catalog snapshot after validation, sandbox readiness and a durable catalog commit. Kind, operation, collection and visible-folder collisions are refused. A plugin operation cannot shadow a host operation or request an account-level capability.
- **Policy.** Plugin operation rows are derived from the manifests (`required_capability`), never hand-written. Host rows (`authz::POLICIES`) and plugin rows go through the same `Catalog::policy` lookup and the same `Policy::authorize` path. An operation with no row answers like a hidden target (`item not found` / `space not found`), so registration is not observable to non-members.
- **Operation targets.** A manifest operation may declare `target: { kind, field }`: `field` is a required string property of its input schema holding the item ID. The existence-hiding resolver finds the item's space from `(kind, id)` and the caller is authorized there; a missing, foreign or wrong-kind ID answers the same not-found. The plugin receives its own input (the optional `space_id` hint is removed). Operations without a target are space-scoped: the wire input carries `space_id`, which is removed before the rest is passed to the plugin.
- **Digest.** The operator approves the package archive checksum. The stored bundle is verified at startup and before each worker starts. Drafts require the development-only `dev-plugin-drafts` build and development mode. They use the same sandbox and are visible in one development space.
- **Workers (protocol v2).** One long-lived Node process per (plugin, space, revision), driven by the library's `WorkerSession` over enrolled machines and their existing sandbox/exec channel. The server starts no worker process. [Worker execution](../../docs/plugin-workers.md) specifies the profile and lifecycle:
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

## Saved turns

`threads.models` reads the calling space's enabled provider manifests from
the startup registry and its indexed activation records. `threads.send`
validates the selected model and reasoning level before writing the message.
The existing job stream carries phase changes and redacted reasoning events.
Provider provenance and timing travel in Markdown through both full and delta
reads. See [saved threads](../../docs/threads.md) for the wire shapes and
checkpoint costs.

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
The [readiness operation](../../docs/agent-turns.md#machine-and-provider-readiness)
reads private machine and provider observations. Saved turn reasons travel in
the ordinary space journal, with owner-only details filtered on delivery.

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

The private account feed subscribes before replay and wakes only after a committed
input change. Each admitted stream retains its existing SQLite connection with a
64 KiB page-cache setting; no SQL transaction remains open between wakes. This
avoids last-connection WAL checkpoint/delete/reopen between saves. Disconnect
closes SQLite on the host blocking pool, retaining the admission lease and root
guards until close finishes. Active plus closing readers therefore share the
existing per-account listener ceiling. There is no second account cache or new
background thread. The reference app's TCP_NODELAY listener policy is also retained
for small live frames; changing that option alone did not remove the database
connection costs in the measured control.
Ordinary commit wakes replay bookmarks without repeating the mutation's cleanup
scan. Maintenance still runs on authority changes and an absolute one-second
heartbeat, including during continuous writes; actual content operations always
recheck current authority. An idle-feed test advances the seven-day confirmation
state without issuing another operation.

The orchestrator manifest owns saved-thread operations. The host reuses generic
content storage and the turn engine for checkpoint ranges, live tool steps and
saved Markdown. See [saved threads](../../docs/threads.md).

Canonical request fingerprinting for Notes, plugin activation and plugin calls
uses the existing blocking pool. The parsed input moves into that task and returns
unchanged with its digest; handoff adds no body clone and the canonical digest is
computed once. The existing canonical encoder still materializes sorted JSON
trees. Synchronous callers retain the same canonical helper. CLI argument
definitions live in `src/cli.rs`; `main.rs` keeps startup and dispatch.

## People over HTTP

See the [people and access reference](../../docs/people.md) for account profiles,
space grants, groups and single-use passkey invitations. The people operations use
the same protected authority and recovery publication as existing space changes.

The Notes retention task probes one closed space at a time with a read-only
SQLite connection, opening its content/journal lifecycle only for expired work.
Purge eligibility is rechecked under that space's lock. This preserves startup
and hourly expiration without filling the lazy space cache during installations
with no expired Trash. Such probes can warm SQLite metadata; cold-space latency
measurements retain that qualification. A matching HTTP receipt replay checks
permanent-deletion state without forcing file placement: purge suppresses prior
body receipts, while later outside-file conflicts do not revoke an acknowledged
SSD result. The shared content receipt type carries the existing item UUID.

Native operation admission and delivery checks await the existing auth worker directly.
Item resolution offloads only its catalog lookup before awaiting the same current-grant
query. There is no cross-request capability cache: each check observes the existing
auth-worker publication barrier, including removals, role edits and account disable.
Synchronous test directories use the blocking adapter.

Foreground operation callbacks use the operation catalog/manifest's `mutates`
flag to select scheduling. Separate lazy current-thread Tokio drivers supply two
blocking readers and up to 32 blocking writers. Each lane admits at most 64
running/queued callbacks; further callers await permits on the async executor.
The listener's two unscoped blocking slots remain separate for authentication
adapters, startup and background tasks. Thus foreground reads never join the
writer or background queue. There can be 36 blocking threads across these three
executors, versus the previous eight shared foreground/background slots; all
new lane threads retire after ten idle seconds. Performance ceilings, auth
batch width, the 64-space cache and retention are unchanged.

A caller that disconnects before admission drops its waiter; a started durable
callback finishes as before. No per-space state is stored in the scheduler.
SSE bodies retain read scheduling on every body poll after the HTTP handler
returns, using one pinned stream wrapper per connection and the existing
listener/page limits. Async spawned background tasks do not inherit request
scope. Non-operation HTTP
mutations also use writer scheduling; operation requests derive their class
from their declaration after parsing. Authentication runs before that scope.
These scheduling reservations do not bypass a space's locks or the auth
worker's mandatory publication barrier. A read blocked on those resources can
still delay other reads within the bounded reader lane.

The optional Unsplash integration serves the eight built-in wallpapers through
the same origin. See the [wallpaper contract and environment setting](../../docs/wallpapers.md).
