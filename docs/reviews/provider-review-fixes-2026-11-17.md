# Provider review fixes, 2026-11-17

Lane `lane/provider-claude`, issue [#16](https://git.kayg.org/kayg/calternal-ai/issues/16),
thread `thr_y4g4pnn3b5`. Review: [#27](provider-independent-review-2026-11-17.md).
This report supersedes the earlier qualification for the changed security paths.
The final gates qualified code commit `9163f93`; this report is a subsequent
metadata-only commit. A SHA-256 comparison matched all 1,041 source files between
the worktree and VM, excluding this subsequently written report.

## Commits and scope

The branch retains dev integration base `e7f94de` and provider head `5121423`.
Review commits `9fead83`, `b9f2403` and `5744963` were cherry-picked as `c7247d9`,
`8a26e0f` and `b899c62`, preserving the review and its failure evidence.

- `b7c7d5b`: name-only Podman environment arguments; direct/configured/enrollment audit.
- `9163f93`: credential redaction, risk/source checks, admitting-session checks,
  narrower modes, normalized grants, shared scope helpers and startup diagnostics.

No review Open decision was selected. Account-home placement, token transport,
wider next-turn mode policy, grant lifetimes, retained-thread deletion and queued
mode changes remain with the owner. A wider requested mode still keeps the saved
mode; a narrower request now applies.

## Findings and verification

1. **Credential publication/storage.** The server redacts its issued credential
   before folding provider events into checkpoints or publishing them. Every string
   field is covered by an exhaustive event match; checkpoints are scrubbed again
   before durable saves. Text buffering retains only a possible credential prefix,
   so splitting the value across deltas cannot reconstruct it in output. Flushes
   preserve ordered event numbering, UTF-8 and the 4 KiB text bound. The original
   real-server/machine/adapter regression passes, as do all text split positions
   and a maximum-size Unicode delta case. This is exact issued-value redaction;
   the review's separate token-transport Open item is unchanged.
2. **Risk and originating app.** Irreversible, external and widening operations
   require an exact action grant in every mode. An app grant or originating app
   does not authorize these effects. Least access is never wider than the default.
   Origin prefixes must match an enabled registry plugin identity/kind/operation
   prefix or a built-in operation group. A conformance case checks unknown,
   disabled, enabled and built-in origins.
5. **Admitting sign-in.** Thread authentication and worker callbacks require the
   original admitting installation to be non-revoked and unexpired, through one
   indexed query on the existing auth handle. Placement, execution and retry waits
   share that check. The review test additionally proves a live installation works,
   and both token entry points reject it immediately after revocation or expiry.
   Static development/in-process callers keep their existing explicit behavior.
3. **Host argv.** Podman receives `--env NAME`; the value is set through the Podman
   child environment. Direct execution already uses the process environment, and
   configured execution delegates to those implementations. Both reject names
   that could carry an inline value. Enrollment sends the code through HTTP; the
   spawned test CLI now obtains it through `CALTERNAL_ENROLL_CODE`. The independent
   recording-engine regression, an additional multiline/invalid-name case, and
   actual rootless container environment delivery pass. No credential value is
   constructed into host argv by these paths.
4. **Narrower next-turn mode.** A new turn takes the narrower of its requested and
   saved modes. Narrowing and the previously tested refusal to widen both pass.
6. **Grant shape and rotation.** One `Grant` contains app, read/write permission
   and optional action hash. Denial-generated approvals now apply in Least access;
   app grants cover ordinary app operations while sensitive effects still require
   the digest. The review fixture/assertion was adapted to the typed grant API;
   its behavioral expectation is unchanged. Scope preparation/issuance share one
   bounds/lookup helper; mode and approval changes share one full-identity scope
   replacement/token-revocation statement. These changes remain behind auth's
   recovery barrier. The single current auth schema is version five; older
   disposable schemas are refused, with no migration or legacy grant reader.

**Missing build.** Each original provider-turn test checks for both compiled
adapter entry files before creating fixtures, with the sentence:
“Claude Code adapter build is missing; run npm run build:plugins.” The sandbox
launcher emits that same permanent setup error when compiled files are absent;
a missing executable or Node runtime also has a bounded provider error. The job
ends failed, retains the setup message in its checkpoint, and does not retry.
With the build directory temporarily hidden, the three original tests fail at
entry with that sentence, while the intentional missing-adapter regression
passes. Restoring the build makes all four pass.

## Exact commands and results

All builds/tests ran only on `claude@calternal-dev-01-compute02` in
`~/Developer/lanes/provider-claude`, through `nix develop -c`; Cargo uses `--locked`
and `-j6`. No build/test ran on hd01. Source sync:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/provider-claude.tar
ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/provider-claude && cd ~/Developer/lanes/provider-claude && tar xf -' < /tmp/provider-claude.tar
```

Plugin builds precede Cargo tests. Commands below run inside that VM lane with
`nix develop -c`; the fake fixture image is
`localhost/calternal-provider-test-provider-claude:review20261117`.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before tests |
| `plugins/calternal-claude-code/test/build-image.sh localhost/calternal-provider-test-provider-claude:review20261117` | Passed; rootless fixture image built |
| `CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-test-provider-claude:review20261117 cargo test --locked -j6 --workspace --no-fail-fast` | 513 passed, 0 failed, 1 opt-in ignored |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed with test-plugins |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 47 passed |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1` | 1 passed (explicit real-server test) |
| `CALTERNAL_TEST_REAL_CLAUDE=0 npm run test:plugins` | SDK 12, notes 2, adapter 6 passed; real-account test skipped |
| `npm run test:integration` | 1 passed |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed; generated artifacts match |
| `scripts/check-docs.sh` | Passed: all-target/all-feature checks and Rust/TypeScript docs |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Both passed |
| `cargo build --locked -j6 -p calternal-cli --bin cl`, then `python contract/verify-cli.py` | Passed; real-server smoke covers lifecycle, receipts, authority and restart |

Focused checks before full qualification:

- `cargo test --locked -j6 -p calternal-machine --lib --test podman_exec_env`:
  12 unit tests and the independent argv test pass.
- `cargo test --locked -j6 -p calternal-machine --test podman -- --nocapture`:
  three pass with Podman actually available; includes environment delivery.
- `cargo test --locked -j6 -p calternal-server --lib --test provider_review --test provider_review_stream --test provider_access --test provider_security --test provider_turns --no-fail-fast`:
  five unit tests, four policy/session review tests, one disclosure review test,
  two access tests, one recovery test and four turn tests pass.
- Negative build preflight: temporarily move only this lane's adapter `dist`, run
  the provider-turn suite, expect exit 101 with three exact startup messages and
  one passing intentional setup-failure case, and restore `dist` with an EXIT trap.

The review Open items remain unimplemented. No real Claude account, macOS sandbox
or production HTTPS test is claimed.

## VM cleanup

After the successful gate chain, this lane's `target` (18 GiB), standalone
conformance `target` (1.8 GiB) and `reference-docs` (878 MiB) were removed with
`rm -rf target tests/conformance/target reference-docs .provider-image-context`;
absence was verified. The lane-specific Podman fixture image above was removed.
No retained user workspace, account home or another lane's build was deleted.
A second SHA-256 comparison after all gates again matched the 1,041 source files.

## Fifteen-item gate delta

1. Shared Actor, IDs, caller errors, capabilities, clock/hash primitives reused;
   one domain grant type replaces the two grant shapes.
2. No new crate or dependency. Existing auth/content/machine primitives are reused.
3. No new global lock, worker or connection; auth proof reads use the existing
   guarded handle, and execution still uses per-space job storage.
4. Auth, filesystem and SQLite calls stay on blocking workers; process/network
   execution remains asynchronous. Redaction is bounded in-memory work.
5. Provider frames parse once. Redaction adds bounded string scans and at most
   credential-length pending text; authentication adds one indexed admitting-proof
   read to the existing access-row/job checks. Scope bounds/read now have one
   implementation, as does rotation. Checkpoint transactions remain at session,
   tool completion, terminal and three-second text boundaries. The shared core
   validation/typed-handler double decode remains the previously recorded #16
   follow-up; this change does not claim to remove that API-wide gap.
6. Registry, schemas and catalogs remain cached; no schema recompilation per event.
7. Existing event/line/checkpoint/job bounds remain. Pending redaction is shorter
   than the issued credential; extra text frames obey 4 KiB. Action digests have
   a 64-character bound, grants remain 32/turn, and authority remains 1024/account.
   Workspace retention is unchanged: explicit deletion now, preserved-work cleanup
   and plan disk accounting as named follow-ups, no timed expiry.
8. One current schema; no compatibility reader or migration chain added. Review
   tests keep their behavioral assertions while adopting the typed grant fixture.
9. Space and actor identity remain shared UUIDs; app labels are validated and never
   replace a space or supply capabilities.
10. One grant representation, one scope lookup/bound and one rotation statement.
11. New auth/session/redaction/launch items have production callers. The reviewer
   trait remains the owner's future irreversible-review seam, not a grant bypass.
12. Existing auth recovery, receipt, exec, streaming and hash primitives considered
   and reused; no second grant database or queue introduced.
13. New public comments state purpose, failure and cost; docs gate covers the crates.
14. Lifecycle, machine/provider documentation, generated contract and this dated
   report accompany the code. The independent report stays historical evidence.
15. Changed production modules and provider-turn tests remain below about 600 lines;
   the format gates cover workspace and standalone conformance code.

## Simplification opportunities

The three review simplifications are implemented: normalized grants, a shared
scope lookup/bound, and one rotation statement. Thread storage and the future
trusted requests surface can use those ports directly. No additional queue,
compatibility layer or choice from the review Open list was added.
