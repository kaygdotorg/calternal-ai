# Recovery review and validation — 2026-11-17

The append-only format replaces the reviewed intent/head/radix-node design.
[PROTOCOL.md](PROTOCOL.md) specifies the safety argument, exact costs, crash
handling and freshness boundary. No deployed format or compatibility is retained.

## Retained audit and regressions

The earlier review found four defects: deletion of newest recovery data restored
old authority; repeated prepare could trust an uncommitted witness; passkey rename
bypassed recovery; and login counters/session creation bypassed recovery. The last
two auth fixes remain intact. Regressions now exercise the log format: newest whole
suffix deletion must fail against the surviving DB or external pin; standalone
hash chains cannot prove an absent suffix ever existed. That limitation is explicit,
not a claim that local hashing can establish freshness.

Protected runtime writes use the existing IMMEDIATE transaction/commit barrier:
account creation, enrollment/recovery-token replacement and consumption, passkey
addition/removal/rename, login/reverification counters and session creation, session
revocation, space lifecycle/membership/owner changes, receipts and fixture seeding.
Session revocation fences still run immediately after SQLite commit, even when
marker publication fails. Setup/session/account checks remain the auth owner's
responsibility. Startup migrations run before admission under root locks. Future
invite/people/group/quota/share mutation APIs must use the barrier when implemented;
external SQL writers are not authorized by merely triggering dirty-key capture.

Excluded writes are hourly session last-seen/rolling-expiry touches, passkey
last-used touches, memory-only rate limits/ceremonies, and sharing journal,
delivery/ack/reset bookkeeping. Rebuild must enforce absolute expiry/revocations
and resync all devices under current authority. No sharing feature is enabled here.

Executable coverage includes primary and replay faults for grants, revocations,
roles, transfers and passkeys; aborted decisions and torn marker prefixes; torn
multi-owner proposals; no partial current state; next-request completion; real
account/space APIs; passkey-label persistence; session-cache revoke fencing;
installation identity, closed inventory and account/space identity separation;
private modes/no-follow descriptors; corruption, gaps, deletion, substitution and
reordering; complete-tree rollback/pins; all 25 ordered pairs of concurrent space
create/remove/role/transfer/delete; bounded segment retention; every compaction
boundary; and counted fsyncs for each kind of change at two inventory sizes.
Atomic snapshot internals rely on fsio tests, not physical power-cut testing.

## Changes and size

Owned paths: `crates/recovery` and the recovery trait/commit hooks in
`crates/auth/src/spaces.rs`. Removed `src/index.rs`, `PLAN.md`, `VALIDATION.md` and
`REVIEW-VALIDATION.md`; the latter reports are consolidated here. No server/core,
UI, auth migration, manifest outside this crate, production data or running server
changes. No agents, external messages, commits or pushes.

Line counts (`wc -l`, Rust source and Markdown; Cargo.toml excluded):

| Scope | Before | After |
| --- | ---: | ---: |
| Production Rust | 1,395 | 1,250 |
| Test Rust | 1,180 | 1,324 |
| All Rust | 2,575 | 2,574 |
| Markdown | 771 | 272 |
| Rust + Markdown | 3,346 | 2,846 |

## Final validation

All commands ran on disposable fixtures with Cargo limited to four jobs.

| Exact command | Result |
| --- | --- |
| `./scripts/with-toolchain.sh cargo test -p calternal-recovery -j4` | Exit 0: 34 passed, 0 failed/ignored; 0 doctests. |
| `./scripts/with-toolchain.sh cargo test -p calternal-auth -j4` | Exit 0: 93 passed, 0 failed/ignored across unit/integration suites. |
| `./scripts/with-toolchain.sh cargo test --workspace -j4` | Exit 0: 368 passed, 0 failed, 3 pre-existing ignores. |
| `scripts/check-docs.sh` | Exit 0: Rust checks/rustdoc and TypeScript documentation gates pass. |
| `./scripts/with-toolchain.sh cargo doc -p calternal-recovery --no-deps -j4` | Exit 0: public reference generated without documentation warnings. |
| `git diff --check` | Exit 0. |

The documentation script does not enumerate recovery, so its separate rustdoc
command enforces this crate's missing-doc/broken-link denial. Workspace warnings
come from existing server documentation/unused-mut code, outside owned paths.
The unchanged ignores cover obsolete cookie compatibility, old-schema upgrades
and unported protected account receipt replay. No ignores were added.
Initial development checks failed on rusqlite's unsupported u64 conversion and
new fixture startup-count/schema/FK mistakes; all were corrected. Final checks
have no failures. Deterministic faults cover 45 primary mutation/boundary cases,
70 committed/aborted replay cases, 17 compaction positions, torn prefixes and
additional regressions; these are not physical power-cut or performance passes.

Steady-state account, space, creation, deletion and combined/receipt changes each
use exactly **2 file + 0 directory fsyncs** (formerly 80/81/150 total recovery
syncs depending on change). No-op/retry/touch changes use **0**. Compaction adds
**2 file + 3 directory**, for **7 total** on a triggering change. Counts exclude
SQLite's inherited FULL/WAL durability; cold/recovery costs are in PROTOCOL.md.

## Remaining limits and questions

The server does not enable this optional sink in this lane. Host root validation,
all-writer configuration, validated DB rebuild, namespace recovery, sandboxing and
external checkpoint storage remain integration work. Linux descriptor paths are
required. Memory holds current protected records; compaction rewrites the current
inventory. An unresolved proposal plus DB loss refuses. A valid old suffix/tree
needs independent freshness evidence. Privileged image forgery is outside scope.
No product or architecture questions remain for this lane.
