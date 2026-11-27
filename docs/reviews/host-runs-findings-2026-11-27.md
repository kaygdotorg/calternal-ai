# Host run wiring · 2026-11-27

Lane G, slice 6, [Forgejo #117](https://git.kayg.org/kayg/calternal-ai/issues/117).
The base includes the shared browser shell on `dev` at `b348e053`.

## Findings

- A clean export of `293af65a` exited with code 1 at
  `runs_harness.rs:101`. The test invoked an image copy of the program
  without the bridge module or either host socket. The corrected test
  starts the bundle through the same machine service as the host. The fake
  image supplies the adapter and echo executable.
- Stop previously coupled process termination to the wrapper or Podman
  cleanup. Both could exceed the two-second deadline on the dev VM. The
  runner now waits for the private PID namespace init through a pinned Linux
  descriptor. The filesystem and engine cleanup runs separately.
- A successful start receipt is bound to the originating gesture, the
  resolved harness and the start input. Retrying the gesture gets the same
  run. A changed filler conflicts. Initial adapter input has separate
  stable transport keys so replay also avoids writing a duplicate command
  sequence into the existing process.
- The caller's item-scoped coded operation supplies the sole session
  target. A prompt receipt link resolves within that item's ledger. The
  harness gets the resulting record body. A sandbox callback goes through
  the same route and carries the same restriction through nested coded
  calls. A run gets session content with raw viewer state, computed fields
  and operation conditions omitted.
- A completed reply invokes Chat Send in the run context. The record's
  verified header and ledger history retain the run UUID. If the Chat
  worker is still busy with the original Send, the reply retries admission
  for up to seven seconds under the original reply key.
- Startup sweeps containers for the host's locked run directory. Restart
  stops all prior runs and discards their logs. Persisted start receipts
  report the original ID as stopped within the operator's receipt window.
- The live socket uses an exclusive chunk offset and reports a conflict
  once that offset has fallen out of the bounded replay window. Auth and
  read checks use the existing personal-space rules. The first-owner code
  belongs to the existing private auth enrollment flow.

## Review gate

- Reused `Actor`, `SpaceId`, the shared errors and provider output types.
  The host depends on the lower-level machine crate and uses `rusqlite`.
  Existing filesystem placement, space admission, receipt, handle and
  record primitives remain the authority for their respective operations.
- Run start serialization is per space and spans the receipt and launch.
  No synchronous mutex spans a process launch, file write or await. Space
  storage work runs off the async reactor. Podman and socket I/O are async.
- Per new run, one receipt lookup and one receipt transaction are added.
  A linked prompt reads the target's records and current Markdown file once.
  Reply publication uses the existing warm append path with its item and
  ledger flush. Log replay has zero item or ledger reads or flushes after
  access admission. These are structural counts.
- Installed declarations already compiled by the host supply program and
  policy. Installation reload now follows explicit insertion order so
  Chat's dependency is available before its harness. The transport tests
  use the current required `item` field. These address R1 S8 and S10.
- The host tracks up to 32 runs per open space and the machine admits 32
  active runs. Each run log holds 256 chunks or 1 MiB, and remembers at
  most 128 input keys. Eviction removes completed logs first. Starts use
  ordinary receipt retention, seven days by default. UUID space, item and
  run identities flow through the wired public methods.
- R1's broader engine and host blockers are recorded on the separate
  [engine and host review](review-1-engine-host-2026-11-28.md). This slice
  binds run starts to prevent the changed-filler case of B4. Authenticator
  revocation fencing, general write validation and retention, and the
  unrelated general query budget remain with the host review work.

## Morning security decisions

- **Should a harness query beyond its one session item?** The current
  host admits item-scoped record access and writes, and refuses broad
  query or nested run calls. A harness that needs related notes would
  require an explicit cross-item grant and a way to show it before Send.
  Keep the default single-item rule until that permission exists.
- **What should signing out do to a run already started?** Runs currently
  use the starting account's authority and the fixed target. Losing access
  to the target blocks each subsequent route call and log delivery. A
  browser sign-out closes that browser's live socket. Binding the run to
  the installation would additionally stop a run when that device signs
  out, at the cost of interrupting work launched by the same owner from a
  second device. Resolve that lifecycle alongside R1 B5 before expanding
  beyond personal spaces.
- **Which provider profile and egress may future runs use?** The dev
  runner uses a disposable profile and exact declared CONNECT targets.
  For example a provider asking for a second hostname is refused unless it
  appears in its reviewed manifest. Persistent authenticated profiles
  and image bindings need an explicit approval and artifact-loading
  contract before they become available.

## Dev VM verification

All commands ran on `claude@calternal-dev-01-compute02` under the lane
directory. The merged export used the pinned Nix shell and a lane-specific
fake image tag. Rust artifacts were cached within the lane.

```sh
export CARGO_TARGET_DIR="$HOME/Developer/lanes/runs-harness/target"
export HOST_BIN="$CARGO_TARGET_DIR/debug/calternal-host"
export CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-runs-harness-merged:dev
~/bin/lock-shared nix develop -c npm ci
~/bin/lock-shared nix develop -c npm run build:plugins
~/bin/lock-shared nix develop -c plugins-v2/calternal-chat-claude-code/test/build-echo-image.sh "$CALTERNAL_TEST_PROVIDER_IMAGE"
~/bin/lock-shared nix develop -c cargo fmt --check -p calternal-host -p calternal-machine -p calternal-types -p calternal-plugin-host
~/bin/lock-shared nix develop -c cargo test --locked -p calternal-machine --test egress_proxy --test run_service --test runs_harness
~/bin/lock-shared nix develop -c cargo test --locked -p calternal-host -p calternal-plugin-host --features calternal-plugin-host/operations
~/bin/lock-shared nix develop -c node apps/web/e2e/host-runs.mjs
```

Machine passed 7 tests. Host and plugin transport passed 77 tests. These
include the real Chat and harness bundles producing one run-stamped reply,
an authenticated second person's input refusal, a cross-chat prompt refusal,
sandbox output callbacks exercising cross-chat read and write denial,
undeclared egress refusal, a stable retry and changed-filler conflict, and
Stop under two seconds. Chromium used a cookie-authenticated socket to
confirm chunk offsets, exclusive replay after a reconnect and the same
stopped run ID after a restart.

## Simplification opportunities

- Use the sandbox's program stream for one typed callback and output
  contract. The current provider adapter and run transport have separate
  envelopes.
- Replace the prompt's read-and-verify pass over the whole item with a
  bounded single-record read against the existing ledger proof.
