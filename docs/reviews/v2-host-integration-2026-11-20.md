# Threads on the v2 host: integration · 2026-11-20

Lane issue [#101](https://git.kayg.org/kayg/calternal-ai/issues/101). Branch
`lane/v2-host-integration`, cut from `lane/v2-host-base` at 3e62ca07 (revision 14.44).

## Merged

| Branch | Tip | Conflicts |
| --- | --- | --- |
| `lane/v2-host-registry` (slice 1, #93) | 21e45ba5 | `live_catalog/tests.rs`: kept the base's refreshed Reading List fixture, restored the `provider()` helper the branch removed. |
| `lane/v2-host-items` (slice 2, #95) | a4893baf | `plugin-host/src/registry_v2.rs`: registry's interface check plus items' cached metadata validators. |
| `lane/v2-host-execution` (slice 3, #94) | 1b76d8a0 | none |
| `lane/v2-host-harness` (slice 4, #96) | 86f2dd0f | none |
| `lane/threads-plugin` (#92) | ae428ee6 | none |

After the merge, the 14.43 fixture refresh gave the Reading List manifest a `jobs`
entry that the slice 2 item engine test drops with its operations. The integration
branch removes that entry in the test, as it already removed `schedule_eligibility`.

## Where the first Send stops

Each slice is library code with its own tests. Nothing yet connects them to a client
route or to a running v2 worker. In the order a first Send meets them:

1. No live server enables a v2 worker for a space. The only carrier profile
   (`ApprovedItemHead`, `with_space_item_heads`) is set only by tests. (#93)
2. The space channel carries one method, `item_head`. Threads also calls
   `item_create`, `item_range`, `item_spans`, `item_append`, `item_view_context`,
   `item_view_publish`, `start_job`, `execution_start`, `contribution_call`,
   `contribution_read` and `viewer_state_*`. (#93, consuming #94 and #95)
3. The Threads bundle exports `handlers` and nothing that speaks the carrier frame
   loop. The SDK's `worker-v2.ts` accepts only v1 host calls. (#78, SDK)
4. `content.db` has no execution tables (#94) or contribution spool tables (#93).
5. No client route exists for `app.open`, `item_view.open`/`follow`,
   `activation.resolve`/`confirm`/`dispatch`, `extension.choices` or
   `execution.control`. (slices 5 and 6, not started)
6. The HTTP binding of renderer events is unpinned. The renderer posts one route per
   event with `X-Step0-View-Lease`, the step 0 fixture's shape. (#78)
7. `contribution_call` has no path to `StartEntry`. Frames have no path to the spool
   or `contribution_read`. The durable launch binding for `ResumeEntry` is missing.
   (#93, #94, #96)
8. The harness package's image is the digest of `fixture-entry.mjs`, and that entry
   throws. A real Podman run needs a scripted, digest-pinned entry image. (#92, #96)

## End-to-end case

`tests/conformance/threads_v2_cases.rs` is behind the `threads-v2` feature. It
installs both built packages through the real installer into a new space, enables
them, and asserts `app.open`, which fails today at item 5. Create, Send, first
output, reload replay, Stop and the restart-during-output gate follow in that order
once the routes answer. Run on dev-01:

```sh
npm run build:plugins && npm run build -w @calternal/threads
cargo build -p calternal-server --features test-plugins
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test \
  --manifest-path tests/conformance/Cargo.toml --target-dir "$PWD/target" \
  --features threads-v2 --test isolation threads_v2 -- --test-threads=1
```

## Simplification opportunities

- One generic carrier method dispatch, from method name to engine call, would cover
  items 1, 2 and 7. Adding a `SpaceMethod` variant per method would not.
- Fold slice 2's migrations 9 and 10, its `with_compatible_creation(8)` witness, and
  the execution and spool schemas into the initial `content.db` schema, under the
  2026-11-20 decision to edit schemas in place. The upgrade test pinned at
  version 8 then goes away.
- Drop the plugin's v1 reader (`legacy.ts`) for the fresh install.
