# Shared client and Swift bridge verification — 2026-11-17

The online client and async Swift bridge execute real operations. Full `swift test`
is **unfinished**: the macOS VM has Command Line Tools without XCTest or a full
Xcode installation. The same assertions passed in the Swift executable probe.
No offline cache, journal transport or production-readiness claim is made.

## Changed paths

- `crates/client`: typed operation associations using core DTOs, retained pending
  identities, common bounded transport retry policy, sticky cancellation, explicit
  re-authentication errors, resolver, tests and the unbuilt sync design in `SYNC.md`.
- `crates/bindings`: async native client/request/cancellation objects, safe error
  categories, generated Swift/C artifacts and reproducible generation helper.
- `crates/cli/src/main.rs`: session/development credential wording, bounded retry
  and re-authentication behavior through the shared client. No token argv support.
- `apps/apple/CalternalCore`: SwiftPM library, Task-cancellation adapters, generated
  bridge, shared live-server checks, XCTest adapter and CLI execution probe. No UI.

All edits are within the lane's owned paths. No subagents, threads, commits,
pushes, production data or external messages were created. Other lanes changed
shared files and the branch during this task; no branch was switched by this lane.

## Snapshot and build scope

The first requested whole-checkout VM build failed before compilation: another
lane's new `crates/genui/Cargo.toml` had no target. To avoid changing another lane's
files, successful checks used a disposable **committed-source baseline plus this
lane's owned files**, under `~/Developer/lanes/client-core/verification` on each
VM. The complete 370-test workspace result is for that baseline, rather than all
concurrent lane changes. A later refreshed-shared-snapshot check also passed the
15 lane tests and the real-server integration test, including the then-current
content/server changes. The full combined workspace still needs the parent gate.

The snapshot was created with `git archive HEAD` streamed into that directory,
then an owned-path tar overlay. All builds/tests ran on the authorized VMs, never
on the editing host. Cargo concurrency was at most `-j4`. No server used port
3210; real-server fixtures use port 0 and delete disposable data on exit.

## Exact commands and results

Commands below ran after `cd ~/Developer/lanes/client-core/verification` on
`claude@calternal-dev-01-compute02`:

```sh
nix develop -c scripts/with-toolchain.sh cargo test -j4 -p calternal-client -p calternal-bindings -p calternal-cli --features calternal-bindings/uniffi
nix develop -c scripts/with-toolchain.sh cargo build -j4 -p calternal-server
nix develop -c scripts/with-toolchain.sh sh -c 'CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j4 -p calternal-client --test real_server -- --ignored --nocapture'
nix develop -c scripts/with-toolchain.sh cargo test --locked -j4 --workspace
nix develop -c ./scripts/check-docs.sh
nix develop -c scripts/with-toolchain.sh cargo clippy --locked -j4 -p calternal-client -p calternal-bindings -p calternal-cli --all-targets --all-features -- -D warnings
```

Results in the same order: **15 tests passed** (one separately scheduled real-server
fixture ignored); server build passed; **one real-server test passed**; workspace
**370 passed, zero failed, four ignored**; documentation gate passed; Clippy passed.
The four workspace ignores are three inherited cases and the new separately run
real-server fixture. The docs script already includes all owned crates and needed
no edit. Existing server plugin-documentation warnings remain outside lane scope.
An early Rust compile failure from moving a private field into its module was
fixed; the successful results use the corrected accessor.

Supplementary plugin build, in the same disposable VM snapshot:

```sh
nix develop -c sh -c 'npm ci --ignore-scripts --no-audit --no-fund && npm run build:plugins'
```

It passed. No dependency lockfile was copied back. Rust sources were formatted
with `nix develop -c scripts/with-toolchain.sh rustfmt --edition 2024` and the
explicit owned Rust file list; static `git diff --check` passes.

Commands on `lume@calternal-macos-01-std`, in the same relative directory:

```sh
/nix/var/nix/profiles/default/bin/nix develop -c scripts/with-toolchain.sh cargo build -j4 -p calternal-server -p calternal-bindings --features calternal-bindings/uniffi
python3 crates/bindings/generate-swift.py
python3 apps/apple/CalternalCore/test-server.py --probe
python3 apps/apple/CalternalCore/test-server.py
```

A final documentation-metadata refresh also ran
`/nix/var/nix/profiles/default/bin/nix develop -c scripts/with-toolchain.sh cargo build --locked -j4 -p calternal-bindings --features uniffi`, then generation and the probe again; all passed.

Results: Rust server/library/bindgen build passed; generation passed; **Swift probe
passed**; **SwiftPM XCTest failed** with `no such module 'XCTest'`. The harness
runs `swift run -j 4 CalternalCoreProbe` for the probe and `swift test -j 4` for
XCTest. Both call the same `BridgeChecks.run()`; only the test runner differs.
The probe checks describe, spaces.list, Notes create/replay/get/update/history,
resolve, explicit token cancellation, Swift Task cancellation and a 401. Both
harness modes stop the disposable macOS server even on failure. Notes Insights
is unavailable in this macOS fixture because no worker bundle was built there;
the tested operations do not use that worker.

Exact observed Apple versions:

- macOS **26.5.2**, build **25F84**, arm64.
- Swift **6.3.3**, `swiftlang-6.3.3.1.3`, clang **2100.1.1.101**;
  swift-driver **1.148.6**, target `arm64-apple-macosx26.0`.
- macOS SDK **26.5**, from Command Line Tools.
- **Xcode version unavailable**. `xcodebuild -version` fails because the active
  developer directory is `/Library/Developer/CommandLineTools`, and `/Applications`
  contains no Xcode app. No global developer-directory setting was changed.
- Nix Rust compiler **1.99.1**, `58cf8f9c3` (2026-10-13). The login shell's default
  Cargo was 1.98.1; builds used the pinned Nix toolchain instead.

Logs are retained locally under `crates/client/verification/` (ignored build
artifacts), including `workspace-tests.log`, `real-server.log`, `docs-check.log`,
`clippy.log`, `swift-probe-final.log` and the failing `swift-tests-final.log`.

## Refreshed shared-snapshot check

After the parent explicitly allowed removing unfinished independent crates in a
lane's **VM copy only**, this lane refreshed `~/Developer/lanes/client-core` from
the shared checkout and deleted **only `crates/genui` in that VM copy**. It had no
library target yet and this client's dependency graph does not use it. Nothing
was deleted on hd01. Content and plugin-host source remained in the snapshot.

The following commands ran there (outside the `verification` subdirectory), with
the same authorized dev account:

```sh
rm -rf crates/genui
CARGO_TARGET_DIR="$PWD/verification/target" nix develop -c scripts/with-toolchain.sh cargo test -j4 -p calternal-client -p calternal-bindings -p calternal-cli --features calternal-bindings/uniffi
CARGO_TARGET_DIR="$PWD/verification/target" nix develop -c scripts/with-toolchain.sh cargo build -j4 -p calternal-server
CARGO_TARGET_DIR="$PWD/verification/target" nix develop -c scripts/with-toolchain.sh sh -c 'CALTERNAL_SERVER_BIN="$PWD/verification/target/debug/calternal" cargo test --locked -j4 -p calternal-client --test real_server -- --ignored --nocapture'
```

**All passed:** 15 lane tests, successful server build and one separately run
real-server test. Evidence: `shared-client-tests.log`, `shared-server-build.log`,
`shared-real-server.log` in the local verification artifacts. Existing content and
server warnings remain outside this lane. No source workaround was applied to them.

## Dependency changes

- Client adds normal Tokio dependency, workspace requirement `1`, with `sync`;
  resolved **1.54.1**. Tokio was already a workspace/dev dependency.
- Bindings adds local `calternal-client` **0.1.0**, workspace Tokio **1.54.1** and
  workspace `thiserror` requirement `2`, resolved **2.0.21**.
- Existing optional UniFFI requirement `0.29`, resolved **0.29.5**, gains its
  `tokio` feature. No new version requirement or root manifest edit was made.

The parent must merge Cargo's generated dependency edges into the shared lockfile;
this lane neither hand-edited nor copied back any lockfile.

## Quality

HTTP/protocol checks and one retry policy live in `client/src/lib.rs`; typed
contract associations live in `typed.rs`; cancellation lives in `cancellation.rs`.
`bindings/src/native.rs` adapts those APIs without duplicating transport policy.
The handwritten Swift overloads connect Task cancellation to the explicit Rust
token because generated UniFFI 0.29 async functions do not do so automatically.
The shared `BridgeChecks` fixture prevents probe/XCTest assertion drift.

A call performs at most three HTTP attempts, with 100/200 ms backoff only after
transport failures. Encoding/validation/copying is O(request/result bytes), with
a 128 MiB response cap per attempt. There are no client persistent writes or
fsyncs. Tokens use one synchronized watch value; cancellation wakes current
waiters without growing a queue. No per-request work scans unrelated accounts,
spaces or content. Public API comments describe costs and invariants.

Native editor input, composition, selection, layout, animations and per-frame
rendering stay outside UniFFI. Journal sync, offline data, durable outboxes,
Keychain/passkey UI, session refresh and UI views were deliberately kept out.

## Unfinished and questions for the user

1. Full Apple test qualification needs Xcode/XCTest provisioning outside this
   lane's paths. Option A: provision full Xcode on the authorized VM (installation,
   disk and SDK setup cost), then rerun the unchanged XCTest adapter. Option B:
   retain CLT plus the passing probe (less setup, but `swift test` remains unqualified).
2. Journal reset: atomic snapshot+cursor (snapshot cost) or leased snapshot+replay
   (lease/expiry machinery)? No HTTP journal/reset contract exists yet.
3. Dirty data after revocation/sign-out: encrypted private recovery retention
   (platform key/expiry policy) or authorized export before deletion (interaction
   and storage-pressure costs)? Neither policy was selected.
4. Apple multi-process sync ownership: broker process (IPC/lifecycle cost) or
   fenced transactional leases (concurrency/recovery cost)? Neither was selected.

Questions 2–4 apply only to the designed-but-unbuilt sync work in `SYNC.md`.
No new online-client product decision was needed. The parent still owns the full
combined-workspace gate after all lanes finish.
