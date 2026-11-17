# Claude provider lane, 2026-11-17

Issue: [#16](https://git.kayg.org/kayg/calternal-ai/issues/16). Branch:
`lane/provider-claude`. Thread: `thr_y4g4pnn3b5`.

The CLI runs an installed, unmodified Claude Code program through a machine-side
TypeScript adapter, native turn jobs, scoped credentials and the shared output
module. Human output streams text/steps; JSON output is NDJSON. Thread files and
message storage remain build step two. See [protocol](../provider-protocol.md),
[lifecycle](../agent-turns.md) and [adapter](../../plugins/calternal-claude-code/README.md).

## Integration and commits

The integration base is **dev `e7f94de`**, recorded as the second parent of merge
`2abd1a1`. No history was rewritten. Later movement of the shared `origin/dev`
reference does not change this qualification base.

- `2abd1a1`: merge dev, port shared types/Actor, protect agent authority through
  auth recovery, use shared space admission, replace CLI polling with jobs SSE.
- `dc5d9ff`: preserve thread modes across later turns, split machine/stream
  concerns, and keep completed buffered events available to an attaching CLI.
- `9e71441`: copy the VM-formatted turn guard; this is the tested code snapshot.

The earlier provider implementation ends at `eafb1cd`; its prior gate results
were recorded in report-only commits `fd4abcb` / `6aefb5a`. They are historical
results against `f41eb88`, not evidence for this integration. The final commands
below qualify the code recorded by `9e71441`; intermediate commits were not
separately gated. A SHA-256 comparison of all **1034 source files** except this report
found **zero differences** against the VM before cleanup. The final report-only edit is not a new code gate.

## Security and concurrency boundaries

`calternal-types::Actor` is the only attribution type. Access records serialize it;
secret audits consume it directly. Instruction text is context, never authority.
The existing directory handle supplies `Agents` and `Machines`, sharing its auth
worker, connection and recovery sink. No additional installation-wide worker,
mutex or connection was introduced. The inherited auth worker and machine
registry still serialize authority/placement work; this report does not claim
those installation-wide services disappeared.

Token hashes, scopes, permission modes, approval requests/decisions, direct-machine
consent and discard authorization are in **auth.db's closed recovery inventory**.
Their transactions use the existing intent/commit/witness barrier before success.
The recovery regression injects a failed intent for mode/grant/discard changes,
checks unchanged authority, and verifies successful publication and snapshots.
Raw Calternal credentials and provider logins never enter that inventory.

Issuance holds only its space's jobs connection lock across the protected auth
commit, preventing an old issuer from racing stop/resume and replacing a successor.
The comment records the reason and lock order: auth callbacks never acquire job
or content locks. Tokens also require the matching active job/run on every use.
A mode/access change revokes the old hash and interrupts execution; explicit
resume delivers the replacement. Token expiry is ten minutes or work ending.

Mode belongs to the account/space/thread. Later turns retain it, while previous
turn grants and approvals are removed. Scope checks still require the person's
current capabilities. Least, default Approve for me and Full follow the declared
risk matrix; widening always requires a person. Approval requires a fresh passkey
session, and agents cannot approve or change their own mode. The reviewer seam
is limited to irreversible actions and currently uses rules that ask a person.

The per-space content receipt namespace admits manual turns and their space-scoped
controls, including retry-setting changes. The temporary `ManualAdmission` port
is small enough for thread storage to replace. The job admission key remains the
existing atomic job-creation recovery key, not a second externally exposed
receipt store. Account retry defaults are private non-authority settings; machine
settings belong to the machine owner and use protected machine authority.

Jobs/checkpoints remain per-space execution state. Machine discard delivery and
acknowledgement rows remain placement state in machines.db: every delivery must
match an exact lease authorization already protected in auth.db. Those rows
cannot grant deletion themselves. Local machine-owner deletion is filesystem
administration guarded against an active role; it grants no Calternal access.
No token/access/mode/discard grant is excluded from the configured barrier.

Cloud/pool sessions require Podman; only an account machine's enrolled owner may
select direct execution. Direct deliberately has no host isolation. Homes and
container names include the account UUID. Tests exercise separate account homes,
stop/crash, silent machines, explicit resume, acknowledged discard and the
mounted CLI inside a real rootless container with a fake provider.

## Commands and results

Every build/test ran only on `claude@calternal-dev-01-compute02`, under
`~/Developer/lanes/provider-claude`, through `nix develop`; Cargo used `-j6` and
`--locked`. No build or test ran on hd01. Sync:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/provider-claude.tar
ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/provider-claude && cd ~/Developer/lanes/provider-claude && tar xf -' < /tmp/provider-claude.tar
```

Each command below ran through SSH from that lane directory with
`nix develop -c`. Formatting/generated contract files were copied back before
commit. Build results are source-snapshot evidence, not individual commit runs.

| Command after `nix develop -c` | Final result |
| --- | --- |
| `cargo test --locked -j6 --workspace --no-fail-fast` with `CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-test-provider-claude:20261117` | 503 passed, 0 failed; 1 inherited opt-in ignored |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 46 passed, 0 failed (43 dev + 3 provider) |
| `env CALTERNAL_TEST_REAL_CLAUDE=0 npm run test:plugins` | SDK 12, existing worker 2, adapter 6 pass; real-account test skipped |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1` | 1 passed; explicitly runs the inherited workspace opt-in |
| `npm run test:integration` | 1 passed |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Artifacts match Rust models |
| `scripts/check-docs.sh` | Passed: workspace/all-features, standalone conformance, rustdoc and TypeScript reference gates |
| `cargo fmt --all -- --check` | Passed |
| `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed |
| `python contract/verify-cli.py` | Passed: real CLI operations, retries, authority and restart persistence |

Preparation also built the standalone CLI with
`cargo build --locked -j6 -p calternal-cli --bin cl` for the extra CLI smoke.
Its initial attempt lacked that binary; no source fix was needed. Preparation used `npm ci --ignore-scripts --no-audit --no-fund`,
`npm run build:plugins`, and
`plugins/calternal-claude-code/test/build-image.sh localhost/calternal-provider-test-provider-claude:20261117`.
Focused machine/access/recovery tests: **8 passed**. The first merged workspace
run also passed; the final run includes the subsequent mode-retention assertion
and module splits. An initial recovery regression reproduced SQLite upsert/dirty
trigger conflict propagation; explicit update/insert in the protected transaction
fixed it, following existing auth mutation patterns. Intermediate conformance
reported 37 passed / 9 failed (stale worker digest and an extra state frame on
ordinary jobs); both were corrected before the final 46-pass run.

Cleanup after qualification removed only lane-owned processes and the fixture
image, then the VM lane's `target` (19 GiB), `tests/conformance/target` (1.8 GiB),
`reference-docs` (877 MiB) and image-context directory. Those directories are
confirmed absent. Source and dependency caches remain. No other lane/application
process, directory or image was removed.

## Review gate

1. Shared IDs, caller errors, capabilities and clock come from calternal-types;
   Actor/Via are reused for access and secrets. Provider-specific protocol types
   have one Rust declaration generating schemas and SDK types.
2. No new crate or dependency for a helper. Types/fsio sit below auth, content,
   machine and server; the server composes their existing ports.
3. Per-space job/content connections; no new installation-wide lock or worker.
   Existing auth/machine serialization is stated above. Issuance's per-space
   cross-worker lock has a documented fencing reason and one-way lock order.
   Offline placement holds no execution slot; a two-account test checks progress.
4. Files, hashing, SQLite and directory operations run on blocking workers.
   Process and socket work is asynchronous. Test fixtures were ported to the same
   blocking boundary. Sandbox initialization joins before stop/adoption.
5. Each provider line is decoded once. Text deltas do no SQLite write. Session,
   tool-finish, terminal and three-second text checkpoints each cost one per-space
   FULL checkpoint transaction. Each idle/output batch checks one membership and
   two plugin activations; lease/output reads use memory. Token authentication
   reads one indexed auth row and one indexed job. Agent operation input follows
   the shared core validation then handler boundary (two typed decodes); this
   remaining item-5 gap is recorded on #16 for a typed shared-API handoff. HTTP
   delivery repeats current
   credential validation. Issuance additionally reads current job/scope/count and
   performs one protected transaction. Receipt admission reads once and publishes
   one shared receipt; job admission adds its existing FULL transaction. Protected
   nonempty deltas add the existing intent/commit/witness publication; the regression
   checks two recovery file-sync observations, not fabricated physical disk timing.
6. Catalogs, validators and tool index are cached. Provider conversion is streaming
   and byte bounded; the SSE decoder is shared with space live subscriptions.
7. Command 64 KiB; provider event 12 KiB; raw provider line 1 MiB; tool detail 4 KiB;
   checkpoint text/instructions 16 KiB; pending tools 128. Job buffers 256 chunks /
   1 MiB, 64 retained completed buffers for 30 seconds. Final jobs seven days/1024
   per space, active jobs 64/space. Authority and discard authorizations each cap
   at 1024/account, approvals 32/turn; a new turn replaces old turn grants. Authority
   mode remains bounded thread state until superseded/account or space deletion;
   thread deletion is the storage lane's caller. Homes persist as explicitly
   required. Working areas are **bounded by explicit deletion now**, through owner
   discard commands, with **automatic cleanup of preserved work and disk accounting
   against the person's plan** named follow-ups, not exceptions. Delivery rows cap
   at 128 unfinished/1152 total per machine, completed retention seven days/1024;
   64 deletion targets/request. No timed workspace expiry.
8. Manifest v1 and GenUI v2 only. No migration chain, old reader or alias added.
   Current auth schema refuses other versions. Machine protocol advances to two;
   the old process backend is replaced by documented direct execution.
9. Canonical shared space UUID validation, current capabilities and account-qualified
   sandbox paths; no display name or app-specific identifier acts as space authority.
10. Agent operations derive their policy/catalog from one declaration. Manifest
    risks are required and validated by Rust/SDK; provider types generate contracts.
11. Adapter, CLI subscription, admission, tokens, approvals, retry, machine and
    discard paths have runtime callers. Personal instructions remain a comment.
    Thread storage is the named future caller for admission and thread deletion.
12. Reused existing fsio descriptor-relative reads/private placement, protected
    auth commits, shared content receipts, job checkpoints/live streams, machine
    job seams, and fresh-session/token hashing primitives. The reference app's
    auth/fsio ports were considered before provider-specific code.
13. The docs gate includes all affected Rust crates and the TypeScript provider.
    Public comments explain purpose, failure and cost; locks explain why they span
    publication. New modules isolate authority, transport, streams and placement.
14. Protocol, lifecycle, provider/machine/server/CLI docs accompany code; this dated
    report records the integration boundary and limitations.
15. Main machine and jobs modules are below 600 lines after splitting commands,
    owner administration, streams and SQL concerns. Other new server modules remain
    below the guideline. Final format gate result is recorded above.

## Open and unfinished

- Independent token/access/sandbox review is still required before merge into dev.
- Review item 5: consolidate the shared validation/typed-handler decode boundary;
  this lane states both decodes and does not claim it eliminated that API-wide gap.
- No real signed-in Claude test ran. The opt-in test requires
  `CALTERNAL_TEST_REAL_CLAUDE=1`; fake tests establish protocol behavior only.
- Linux direct/Podman were exercised; other Podman hosts require a matching Linux
  CLI in the image. Production HTTPS and a real provider service were not exercised.
- Thread files, personal instructions, content-journal agent attribution, automatic
  preserved-work cleanup and plan disk accounting are named later-lane work.
- Approval UI and durable once/always-in-space grants remain later work. Current
  turn grants already use protected recovery; no broader grant lifetime is assumed.
- A future model reviewer is undecided. Rules-only asks a person without provider
  cost; a model for irreversible actions adds provider cost and audit requirements.
- Cross-machine resume uses bounded checkpoint context; a Claude session ID alone
  does not transport that machine's official session files.

A Codex adapter reuses the contract, account homes, execution/placement, job
streams/checkpoints, admission, access/risk/recovery, instruction assembly, retry
scheduler and CLI unchanged. Only official invocation flags and event conversion
belong in its adapter. No further product decision was made for that provider.

## Questions

No new product decision is required for this integration. The independent review
and later-lane items above remain unfinished; no merge readiness is implied.

## Simplification opportunities

Thread storage can implement the small admission port and own thread-mode deletion.
The shared SSE decoder now serves journals and jobs; account-feed parsing could
reuse it too. A later requests lane can replace the turn-local approval view with
its common trusted request surface without changing provider adapters.
