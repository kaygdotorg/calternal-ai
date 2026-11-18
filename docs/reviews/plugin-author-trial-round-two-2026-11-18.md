# Plugin author trial, round two

The coordinator ran GPT-6 Luna at medium with the three original requests,
the packaged guide and the refreshed source-free kit. All three plugins
passed verification. Each run still had failed commands.

| Request | Verifier | Elapsed | Failed commands |
| --- | --- | --- | --- |
| Reading list | 6/6 | About 5 min | 13 of 35 |
| Checklist | 7/7 | About 2 min | 4 of 17 |
| Composer chip | 6/6 | About 2 min | 3 of 12 |

The records are `/tmp/plugin-trial-hd/r2-{1,2,3}/events.json` on hd01.
None of the sessions read product source.

| Defect | Evidence and change |
| --- | --- |
| A caller received an error after a committed write | The reading-list schema declared an integer revision and fixture replies supplied integers. The real broker returns an opaque string revision. After failed output validation, the session tried fresh request IDs, creating more items. The host recovers the first committed item acknowledgment from its existing per-space content journal and replays it under the outer request ID. The guide specifies the revision's type and the acknowledgment form. |
| An invalid manifest gave no useful location | Both attempts replaced the generated slot target's output schema and removed its required `view` property. The CLI now reports `/slots/0/renderer` with the generated renderer rule. The validator uses the same semantic checks for CLI and registration. |
| A package with no kinds still needed a `kinds` array | Rust and TypeScript normalize omission to an empty list. The generated wire schema reflects that default. |
| A successful activation appeared to have failed | The next command was a separate `invoke`. Expansion on the wrong side of SSH produced an empty `space_id`. Development output includes the selected UUID and its source. For a failed lookup the invocation error identifies the submitted space and explains where the shell must expand `SPACE_ID`. |
| The session guessed `calternal jobs` | The guide gives the exact `calternal invoke jobs.get` form and documents taking `JOB_ID` from the start result. |
| TypeScript errors ended in a duplicate Node stack | The generated build script now forwards TypeScript's output and process status. Spawn and signal failures have their own short message. The compiler diagnostic retains its source location. |

The operations still use the existing content and request journals. A fresh
interactive content mutation adds one space-local recovery receipt lookup
after a parent receipt miss and one receipt check when finalizing so that
concurrent finalizers return the same published result. Durable write
publication and authorized reads retain their existing fsyncs. Failed
validation reuses the child write receipt. Existing
client retry policy and retention apply.

## Backend gate

The lane merged `origin/dev` at `270c1e3`. The gate ran from
`~/Developer/lanes/plugin-dx` on `ubuntu@calternal-dev-02-std` under
`nix develop`, with four Cargo jobs. The Rust build used
`CARGO_TARGET_DIR=$PWD/target`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_INCREMENTAL=0`.
Conformance used `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Rust tests. Rebuilt the guide and Notes Insights pins. |
| `cargo build --locked -p calternal-server --features test-plugins -j4` | Passed for the normal conformance server. |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -p calternal-conformance -j4 -- --test-threads=1` | 128 passed. Included replay after invalid output and after terminating the server before the worker completion. The latter ran with no eligible worker machine after restart. |
| `npm run test:integration` and `npm run test:plugins` | Passed. The signed-in provider case is opt-in. |
| `calternal plugin new r2-cli --sdk-root "$repo" --json`, `npm install --offline --no-audit --no-fund --silent`, `calternal plugin check . --json`, `calternal plugin test . --json` | Passed for a disposable starter. A malformed generated renderer reported `/slots/0/renderer`. A forced `TS2322` error retained its source location and omitted the Node child-process wrapper. |

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | The host uses the existing caller error, UUIDs, digests and journal receipts. |
| 2. Dependencies | No new dependency or crate. |
| 3. Lock scope | Durable work uses the existing space lock. |
| 4. Async and blocking | Receipt work runs on the request blocking lane or the broker thread. |
| 5. Reads and flushes | Read-only operations add no receipt reads or flushes. The extra admission and finalization lookups on a content mutation are recorded on #50. Recovery distinguishes a committed effect from an absent response. |
| 6. Constant work | Invocation validates with the revision's existing compiled schemas. Error formatting runs on a failure. |
| 7. Bounds | Existing receipt retention applies. Nothing new is stored. |
| 8. Compatibility | The same manifest rules apply throughout registration. Omitted kinds normalize directly to the empty list. The worker adapter uses one receive path. |
| 9. Space identity | The receipt uses the space UUID. The CLI reports the selected UUID and its source. |
| 10. Source of truth | Manifest checks share the registration validator. Write recovery uses the original broker receipt. |
| 11. Callers | The normal invocation route and development CLI exercise both changes. |
| 12. Reuse | Uses the content outbox, per-space journal, cached JSON Schema validators and private catalog socket. |
| 13. Reference | Public receipt and diagnostic-formatting functions document purpose, errors or bounds, and cost. |
| 14. Documentation | The API reference, authoring skill and generated manifest contracts changed with the code. |
| 15. Formatting and modules | Receipt recovery and content adapter methods live in their own modules. |

## Simplification opportunities

Compile operation schemas as part of manifest validation and hand the checked
validators to registration. The current paths still compile the same
declarations separately when registering a package. A journal primitive that
returns the durable value on publication could also consolidate the
finalization reads while preserving first-published semantics.
