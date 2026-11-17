# Claude provider lane, 2026-11-17

Issue: Forgejo #16. Branch: `lane/provider-claude`. Thread: `thr_y4g4pnn3b5`.

The lane runs the installed official Claude Code program through a versioned
machine-side TypeScript adapter, native turn jobs, scoped Calternal credentials
and the shared CLI output module. Conversation files belong to build step two.
The official print-mode stream avoids an SDK dependency and credential handling;
the program owns its login and session files. See [protocol](../provider-protocol.md),
[turn lifecycle](../agent-turns.md) and the [provider README](../../plugins/calternal-claude-code/README.md).

## Commits and qualification boundary

Base integrated: `origin/dev` at `f41eb88` through merge `88d52ed`. The coordinator
explicitly froze this base; no later dev or lane/simplify changes were integrated.

| Commit | Change |
| --- | --- |
| `b2914d5` | Provider v1, SDK contracts, risk metadata, Claude adapter |
| `5fbad35` | Machine stdin, account homes, direct policy, interrupted leases |
| `4169010` | Native turns, scoped tokens, approvals, durable retry |
| `b93a2b8` | Shared human/JSON agent CLI |
| `53161f4` | Protocol and lifecycle documentation |
| `88d52ed` | Merge dev `f41eb88` |
| `0bcfa2a` | Separate streaming output renderer |
| `1981b26` | Visible `AI/Instructions.md` reader and bounded-file tests |
| `d8b546c` | Pending-tool bound and duplicate-start refusal |
| `59d045f` | CLI end-to-end proof and merged-dev fixture corrections |
| `5639dc9` | Explicit owner discard, durable confirmation, canonical UUIDs, provider selection |
| `accb351` | Placement/execution separation, waiting-setting changes, resume fencing |
| `40bd957` | Existing Actor/Via agent provenance in secret audits |
| `eafb1cd` | Real container-layout fake-provider proof and fixture builder |

All builds/tests ran on `claude@calternal-dev-01-compute02`, in
`~/Developer/lanes/provider-claude`, under `nix develop`, with Cargo `-j6`.
No build or test ran on hd01. Tests used copied working-tree snapshots, **not
individual checked-out commits**. After creating `eafb1cd`, a SHA-256 comparison
of every tracked file against the VM copy found **zero differences**. Therefore
the latest completed gate set below qualifies the source tree recorded by
`eafb1cd`. Intermediate commits were not independently gated. The report-only
commit is not separately gated. No qualification has run after rebasing onto
lane/simplify or a later dev; that remains the coordinator's next step.

## Exact commands and results

Source sync from this worktree:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/provider-claude.tar
ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/provider-claude && cd ~/Developer/lanes/provider-claude && tar xf -' < /tmp/provider-claude.tar
```

The commands below were each executed through
`ssh claude@calternal-dev-01-compute02 'cd ~/Developer/lanes/provider-claude && <command>'`.
The latest full chain completed successfully before the coordinator's stop request;
no further tests were started after that request.

| Command | Result |
| --- | --- |
| `nix develop -c cargo run -j6 -p calternal-core --bin export-contract` | Generated artifacts copied back |
| `nix develop -c plugins/calternal-claude-code/test/build-image.sh localhost/calternal-provider-test-provider-claude:20261117` | Built fake provider image; bounded context, empty registry auth file |
| `nix develop -c env CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-test-provider-claude:20261117 cargo test -j6 --workspace --no-fail-fast` | 497 passed, 0 failed, 4 inherited ignored |
| `nix develop -c cargo test -j6 -p calternal-machine --test podman -- --nocapture` | 3 passed; rootless lifecycle/home/discard tests actually ran |
| `nix develop -c cargo build -j6 -p calternal-server --features test-plugins` | Passed |
| `nix develop -c sh -c 'CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1'` | 37 passed, 0 failed |
| `nix develop -c env CALTERNAL_TEST_REAL_CLAUDE=0 npm run test:plugins` | SDK 15, existing plugin 2, adapter 6 passed; real-account test skipped |
| `nix develop -c npm run test:integration` | 1 passed |
| `nix develop -c cargo run -j6 -p calternal-core --bin export-contract -- --check` | Artifacts match Rust models |
| `nix develop -c scripts/check-docs.sh` | Rust and TypeScript reference gates passed |
| `nix develop -c cargo fmt --all -- --check` | Passed |
| `nix develop -c cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed |

Focused proofs also passed on the same final source: provider/access tests 5,
secret/plugin/access provenance tests 25, and the provider-container test 1.
The container proof includes the public streaming CLI, adapter, fake official CLI,
mounted `calternal --help`, and acknowledged disposal retaining the login home.
It does not prove a real Claude account or production HTTPS connectivity.

Four inherited workspace tests did not run:

- `real_server_operations_receipts_and_authority`: its explicit binary environment opt-in was absent.
- `old_name_is_ignored_and_cleared_once_without_touching_host_session`: inherited obsolete-cookie fixture.
- `upgrading_schema_six_removes_cookie_state_and_keeps_passkeys`: inherited old-schema fixture.
- `revoking_another_device_signs_it_out`: inherited protected account mutation replay gap.

No new ignore was added. The inherited compatibility fixtures and account replay
coverage are recorded on #16 for integration review. Initial attempts exposed
stale Cargo metadata after tar overlays; package artifact cleanup corrected that.
New dev fixtures missing required risk/direct-consent fields were corrected.
Only the completed final results above are claimed.

## Handoff and cleanup

Qualification stopped at the coordinator's request. The branch is pushed to both
`origin` and `github`. The lane's VM `target`, `tests/conformance/target` and
`reference-docs` build directories and its private fixture-image context are
removed; only lane-owned test processes/images are cleaned up. No other lane or
existing application's directories or processes are removed.

## Review gate

1. Shared IDs use existing auth UUID wrappers and UUID parsing; provider wire and
   permission types live in core and generate the SDK contract. Existing caller
   errors, token randomness/hashing and canonical request serialization are reused.
2. No new Rust crate or dependency. Machine depends on protocol/core primitives;
   server composes the existing auth, content, plugin and machine ports.
3. Turn/access storage uses existing per-space jobs connections. Account retry
   defaults have one private SQLite file per account. No new global request lock. Offline placement does not consume the shared
   execution budget; a two-account fixture proves ready work can run.
   The inherited machine-service lock remains its documented installation registry
   seam. Activation reads still pass through the inherited `Mutex<Store>`; this
   exception is recorded on #16 because lane/simplify owns that shared refactor.
   No new lock is introduced for turn storage.
4. Directory/auth lookups, descriptor-relative files, hashes and SQLite work run
   on blocking workers. Processes use Tokio or Node asynchronous spawn. Sandbox
   initialization is joined before stop/adoption can race it.
5. Each provider JSON line is parsed once. Text delivery does no SQLite write per
   delta. One FULL checkpoint transaction follows session discovery/tool finish/
   terminal output, or a three-second interval of emitted text. Admission, token
   issuance, state transitions and approvals each use existing FULL transactions.
   Each output batch (a one-second idle wait) rechecks one membership and two plugin
   activations, then makes in-memory lease/output lookups;
   machine/job lookups and current scope checks remain indexed reads.
6. Operation catalogs, schema validators, generated tool index and CLI executable
   discovery are cached. Provider normalization is streaming and byte bounded.
7. Command lines 64 KiB, events 12 KiB, raw CLI lines 1 MiB; tool details 4 KiB,
   checkpoint text 16 KiB, instructions 16 KiB, pending tools 128, approvals 32/turn. Existing active
   jobs 64/space and final retention seven days/1024 records apply. Completed live
   buffers last 30 seconds, at most 64. Persistent account homes are explicitly
   required by the owner and remain until the machine owner removes them. Working
   areas are bounded by explicit deletion through `agent.discard` or the machine
   owner's local `machine discard`, with no timed expiry. This follows accepted
   orchestration defaults. Automatic cleanup of preserved work and disk accounting
   against the person's plan are named follow-ups, not retention exceptions.
   Cleanup records are bounded to 128 unfinished/1152 total per machine, with
   completed retention seven days/1024 during cleanup activity; 64 targets/request.
8. No new migration chain, alias or legacy reader; the old process backend is
   replaced by direct. The four inherited ignored tests above remain; their
   compatibility/replay gaps are recorded on #16 and were not changed in this lane.
   Machine wire version advances to 2 because stop/release and backend selection
   changed. Provider v1 is a separate documented protocol.
9. Space operations validate and canonicalize UUIDs before admission, and recheck
   current capabilities. Account-qualified
   homes/container names prevent reuse of a thread UUID across accounts.
10. Agent operations derive policy/catalog entries from the core declaration;
    plugin risk is declared in manifests and validated by host and SDK. Provider
    frames have one Rust source generating TypeScript/schema outputs.
11. Native operations, adapter, machine transport, access checks, retry recovery
    and CLI are wired together. The personal-instruction seam is deliberately a
    comment; no unused public implementation promises future behavior.
12. Reused existing jobs admission/checkpoints/live buffers, MachineService's five
    job seams, auth token/private-file/fresh-session primitives, content-root
    selection and fsio descriptor-relative reads. The reference app's auth/fsio
    ports were considered before adding provider-specific code.
13. Public types/methods have purpose/cost/failure comments; the docs script covers
    all affected Rust crates and includes the provider in TypeScript reference checks.
14. Provider, machine, server and CLI documentation changes accompany the code.
    The report is dated here; evidence belongs in the issue, not a log in git.
15. New server concerns are split across modules below 600 lines. The machine
    registry is near the guideline (620 lines; 595 at the base), with provider
    implementations in separate modules. Existing
    `api.rs` and `plugins/jobs.rs` already exceed the guideline and remain the
    shared seams; splitting them concurrently would collide with lane/simplify.
    This exception is recorded on #16. Formatting is verified on the dev VM.

## Open and unfinished

- Automatic cleanup after all changes are preserved and disk accounting against
  the person's plan are named follow-ups. Explicit owner discard is implemented.
- Qualification covers Linux direct/Podman execution. Other Podman host platforms
  require a matching Linux Calternal CLI in the image and have not been qualified.
- No real signed-in Claude test was run. The explicit opt-in test is available;
  fake-process tests prove protocol behavior, not provider service availability.
- Content-journal agent/thread provenance is a named follow-up. Secret audits use
  the existing `Actor` with `Via::Agent`; the coordinator will integrate the shared
  Actor from `calternal-types` from lane/simplify. Content-journal wiring is later work.
- Thread files and personal instructions belong to later lanes. This lane reads
  space instructions and does not add instruction management operations.
- Approval UI is deferred. Approval requires a freshly verified person session;
  agent and development bearers cannot approve. Current grants last this turn;
  durable once/always-in-space grants need protected recovery integration.
- The reviewer trait is restricted to irreversible operations. Rules-only review
  asks the person; selecting a model reviewer remains open. Rules-only keeps decisions reproducible
  but requires a person; later model advice adds provider cost and an audit burden.
  No model is selected by this lane.
- Direct execution deliberately has no isolation. Pool execution requires Podman;
  account-owned direct execution requires the owner's stored machine setting.
- A cross-machine resume uses bounded portable checkpoint context, because an
  official Claude session ID alone does not copy its local session files.

A Codex adapter would reuse protocol/types, machine execution and home ownership,
job admission/streams/checkpoints, token scope/risk review, instruction assembly,
retry scheduling and CLI unchanged. The CLI already accepts `--provider` and inherits omitted model choices from
provider metadata. Only official process flags and provider-event normalization
belong to its own adapter.

## Questions

No blocking question remains for this handoff. Selecting a future reviewing model
and designing durable once/always-in-space grants remain Open; no model or broader
grant lifetime is chosen here. Rebase and full integration qualification are pending.

## Simplification opportunities

Use the new live-subscription abstraction for turn events when that path is
integrated, replacing CLI polling. Move manual-turn admission behind `threads.send`
when conversation storage lands. Consolidate existing server job/machine seams in
lane/simplify rather than adding another queue or lock in this lane. Reuse its
shared Actor across caller boundaries and later content-journal attribution.
