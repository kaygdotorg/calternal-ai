# Plugin workers through machine sandboxes — 2026-11-17

Thread: `thr_4ien3sf6rv`; branch `lane/plugin-workers`; [issue #29](https://git.kayg.org/kayg/calternal-ai/issues/29).
Integrated `origin/dev` at `25878bd` before qualification. After qualification,
merged `origin/dev` at `aeb7043`: that update contains only the UI lane’s browser
code/tests and UI documentation; no worker, machine or backend source changed.
The backend gates below cover this lane’s final runtime implementation.
Implementation commits: `37372b8` (shared frame bound),
`1abe29a` (machine workers). Only disposable data was used.

## Implementation

The default server worker pool sends verified source to enrolled machines. It
never invokes a container engine. The machine uses its existing `Backend`,
`Sandbox` and streaming `Exec` interfaces, now with bounded interactive stdin.
Worker frames stay v2; machine sessions are v3; disposable machine lease databases
are schema 3, with older versions refused rather than migrated.

The worker profile mounts code read-only and omits account homes, workspaces,
appdata, userdata, the host CLI and Nix store. The root is read-only; networking
is disabled and the bundled Moby-derived seccomp policy refuses socket creation
and io_uring. Limits: 256 MiB, 1 CPU, 64 PIDs and a 128 MiB V8 heap.

Admission has a 10-second deadline for cleanup/capacity. Invocations retain the 3-second
limit including cold launch after capacity reservation (600 seconds for job runs), 60-second idle shutdown and 4096-call rotation.
Global and per-key permits enforce eight workers total and one per (plugin,
space), including cold starts and teardown. Caller failure is delivered before
asynchronous cleanup. Capacity releases only after authenticated cleanup proof;
`stopping` leases survive server restarts, and recovered teardown blocks new
worker admission until confirmed. A machine retains failed stops for retry and
cleans only its own labelled orphan workers at startup.

Internal worker leases and exec streams are hidden from person-facing machine
routes. Worker stderr is discarded on the machine. Current authority is still
checked by `WorkerSession` and `SpaceHost` on every callback and delivery. All
product data continues through those host calls.

Remote placement is restricted to operator pool machines with existing pool
grants until a shared-space trust contract exists. All plugin network remains
disabled: the current manifest has no destination/approval/fetch contract.
Explicit development and test adapters use the same sandbox interface; direct
mode supplies no OS isolation. Local fixture code paths are never serialized.

## Validation

All builds and tests run as `claude@calternal-dev-01-compute02`, under
`~/Developer/lanes/plugin-workers`, through `nix develop`; Cargo uses `-j6`.
No build, test or performance run was made on hd01, a bare-metal node, a perf VM
or production. The independent [development measurements](../measurements/plugin-workers-development-2026-11-17.md)
are informational only: Podman cold median 862.53 ms and warm echo p50/p95
0.401/1.896 ms; full remote brokered-write p50/p95 60.24/74.01 ms (10 calls).
The measurement probes passed (14.60 seconds local, 9.08 seconds remote).
Memory metrics have different accounting and are not directly comparable.

Reproducible complete gate:

```sh
nix develop -c ./scripts/verify-plugin-workers.sh
```

The script completed with exit status 0. It runs these commands in order (the conformance target directory is
shared with the workspace only to reuse build artifacts):

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Pass |
| `cargo fmt --all -- --check` | Pass |
| `rustfmt --edition 2024 --config skip_children=true --check tests/conformance/isolation.rs tests/conformance/worker_cases.rs` | Pass |
| `cargo test --locked --workspace --all-features -j6` | Pass: 532 tests; one existing real-server opt-in test ignored |
| `cargo build --locked -p calternal-server --features test-plugins -j6` | Pass |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | Pass: 49/49 (83.78 seconds) |
| `npm run test:integration` | Pass: 1/1 real-server integration |
| `npm run test:plugins` | Pass: 20 tests; one signed-in provider opt-in skipped |
| `CALTERNAL_WORKER_TEST_BACKEND=podman cargo test --locked -p calternal-server --all-features -j6 --test plugin_host -- --test-threads=1` | Pass: 18/18 unchanged container tests |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Pass: artifacts match Rust models |
| `cargo build --locked -p calternal-cli -j6` | Pass |
| `python3 contract/verify-cli.py` | Pass: describe, CRUD/history, stale conflict, exact retry, authority and restart smoke |
| `CARGO_TARGET_DIR="$PWD/target" scripts/check-docs.sh` | Pass: Rust public docs/links and TypeScript/GenUI reference gates |

The unchanged plugin suite exercises the direct and container adapters. Separate
OS probes use Node without its permission flags to prove the container boundary:
sockets fail, host and foreign-space files are absent, code writes fail, cgroup
values match, and Linux `memory.events` / `pids.events` report actual OOM kills
and refused processes. Pressure probes read the kernel counters independently
of the exec client’s exit notification, then explicitly stop their container.
An enrolled-machine conformance case checks broker authority, hidden internal
leases, container death, a paused worker's deadline, and replacement.
The final PID-pressure run took 122.95 seconds including Podman cleanup;
capacity remained reserved until cleanup succeeded. Unit tests retain teardown
reservations across restart and require both message
acknowledgement and an absent lease report before releasing them.

Earlier qualification found two implementation errors, both corrected before
reporting: waiting for async sandbox filesystem work from the only blocking
worker deadlocked cold starts; awaiting startup asynchronously makes the existing
cancellation regression finish in 0.03 seconds. Admission also incorrectly used
the operation's three-second deadline, although actual Podman cleanup took over
two seconds. Admission now has its own ten-second bound; invocation deadlines
and performance budgets were not changed. A later shared-VM run timed out one
unchanged child-job test during cold launch (17/18); the unchanged container
suite passed on recheck (18/18, 17.57 seconds). Cold launch remains inside the
three-second invocation deadline, so busy machines can return a safe timeout
before a worker is ready. No timeout or regression test was relaxed.

## Hot-path work

Transport only: a warm invocation performs zero file reads, hashes or filesystem
flushes. It queues one stdin frame; each callback adds one worker frame and one
host-reply frame. JSON frames are parsed once at the session boundary. Existing
broker costs remain: current session, capability and activation checks plus the
method's storage work, as documented in `plugins/broker.rs`.

A cold start reads and hashes the bundle once on a blocking task. A machine
exclusively writes one ephemeral code file and writes one static seccomp policy;
these rebuildable artifacts need no durability acknowledgement. Lease admission,
assignment and running confirmation use the existing durable lease updates;
teardown adds a fenced stop and a confirmed end. No lease write occurs per
ordinary worker frame. Schemas remain compiled once per plugin version.

## Review gate

| # | Evidence / disposition |
| --- | --- |
| 1 | Reuses shared typed IDs, caller-safe errors, SHA-256 and `WorkerSession`. The frame bound now has one declaration in `calternal-types`, re-exported by the host and used by machine input admission. |
| 2 | No crate or dependency added. The shared bound moved down; worker execution reuses the machine sandbox interfaces. |
| 3 | Recorded exception on #29: short global worker/admission indexes and the existing machine registry/outbox remain. No pool lock spans I/O. Durable lease admission and teardown confirmation serialize SQLite commits under the existing registry lock to preserve placement/fencing atomicity. This lane does not claim that lock has been removed. |
| 4 | File verification and thread creation use short blocking tasks. Sandbox startup and teardown are awaited asynchronously; broker work stays on dedicated session threads. The single-blocking-worker cancellation regression is retained. |
| 5 | Warm transport: zero file reads/flushes; one parse per frame. Cold code: one read/hash. Existing broker reads/flushes are stated separately above. |
| 6 | Reuses per-version compiled schemas; the seccomp policy is bundled static data. No per-invocation schema/catalog construction. |
| 7 | Eight physical workers, one per key, at most sixteen live admission keys, 32 invocations, bounded frame queues, 1 MiB source, 60-second idle and 4096-call retirement. Pending teardown has eight bounded reservations and no unsafe timed expiry. Ended leases and request receipts keep existing operator retention. |
| 8 | No migration chain, legacy reader, old protocol fallback or ignored regression. Development fixtures use an explicit non-serialized code-directory seam so existing tests run unchanged. |
| 9 | Spaces are shared `SpaceId` UUIDs; account/thread identities remain separate. No names substitute for ownership IDs. |
| 10 | Resource profile and frame size have single declarations. Global/per-key semaphores are authoritative; the redundant cold-start counter was removed. |
| 11 | `WorkerPool::machines` is called at server startup; machine initialization, interactive input and scope filtering are wired. Every added public contract item has a runtime caller. |
| 12 | Considered/reused existing machine backends, fenced exec/output channel, broker/session/schema primitives, shared hash helper and private machine root. Durable userdata stays with existing storage. The reference app's ADR 0062 and ADR 0071/netguard were read with git only; no storage/network contract was redesigned. |
| 13 | Documentation gate covers types, plugin-host, machine and server. Public comments describe limits, failure and cost; final result is in the command table. |
| 14 | Root and crate READMEs, worker documentation, this dated report and morning decisions accompany the code. Measurements are recorded separately as development-host observations. |
| 15 | New/expanded production modules stay around or below 600 lines; admission and worker transport/session concerns are split into modules. New conformance cases live in their own module; the inherited large route inventory was not expanded inline. Formatting is checked on touched Rust. |

## Unfinished contracts and questions

- Shared-space placement: a space-owned operator worker service would separate
  plugin capacity from agent pool grants; participant machines require explicit
  space trust, revocation checks and disclosure rules. Those rules are not implied
  by the current account-owned machine API.
- No runtime on the server host: explicitly enroll an operator machine, or approve
  an automatic installation/enrollment flow. There is no server container path
  or automatic direct fallback.
- Network: define destination syntax, manager approval and brokered fetch together;
  then port/use netguard for the trusted broker. Worker sockets remain disabled.
- A permanently lost or revoked machine needs an approved operator teardown
  attestation/recovery flow. Until then its unconfirmed capacity stays reserved.
- These development probes do not qualify multi-tenant microVM placement or a
  production deployment. No provider credentials or production data were used.

## Simplification opportunities

- Move machine exec mailboxes off the shared registry lock while preserving its
  existing sequence/fence contract; that would address the recorded gate-3 exception.
- Once a fixture host barrier exists, remove readiness-file code mounts from local
  adapters; remote execution already transfers only verified source.
- A space-owned worker placement service can remove first-caller pool-grant and
  participant-host trust ambiguity without creating another container backend.
