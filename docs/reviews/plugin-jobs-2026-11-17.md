# Plugin host follow-ups, jobs and streams (lane report, 2026-11-17)

Lane `lane/plugin-jobs`, issue kayg/calternal-ai#8, thread thr_73gratau79. Built and tested
on `calternal-dev-01-compute02` only. No timing claims.

## What changed

- **Operation targets.** `ManifestOperation.target: { kind, field }`; `field` must be a
  required string property of the input schema. The trusted file's `item_operations`
  is gone. notes-insights takes `{ id }` (bundle re-pinned).
- **Kind extensions.** `ManifestKind.extension`: claimed text is Markdown (`md` or
  absent); any other kind must declare 1-16 lowercase letters/digits and is stored
  unparsed. Enabling a non-text kind works.
- **Sign-in recheck.** The guard records the caller's installation
  (`plugins::Session`); every worker host call, job run and delivery checks it is still
  live. `api::dispatch` takes the session explicitly.
- **Receipts.** A mutating plugin operation's delivered result is receipted under the
  request ID; a retry returns it without running the worker.
- **HTTP reachability (found while testing).** `calternal_core::parse_request` refused
  every operation outside the core catalog, so plugin operations other than
  `notes-insights.summarize` were unreachable over HTTP. Fixed.
- **Jobs.** Per-space `appdata/spaces/<uuid>/jobs.db`; `start_job` / `checkpoint_job`
  host calls; runner; restart recovery; children; cancel; SSE stream; `jobs.get`,
  `jobs.cancel`. See `crates/server/README.md` (Jobs, Job surface, Job bounds).
- **Secrets.** `request_secret_handle` brokers to `crate::secrets::plugin_handle` with
  the space UUID as owner.
- **Test plugin.** `tests/plugins/jobs-demo`, trusted only with the `test-plugins` feature.

## Hot path costs

- Plugin host call: up to 4 authority checks, each = one installation lookup (one
  indexed read on the auth DB worker, uncached), one directory capability read, one
  store lock + indexed activation read. Then the effect (read: 1 catalog lookup + 1
  item read; write: one durable content write).
- `start_job` / `checkpoint_job`: one FULL SQLite transaction (1 fsync) on the space's
  job file. Chunk: one stream lock, no I/O.
- Stream delivery: one indexed job read + one access check per wake-up batch.

## Review gate

1. Shared types: `calternal_auth::{AccountId, SpaceId, InstallationId}` (session, secrets
   owner), `calternal_plugin_host::{AccountId, SpaceId}` for worker authority,
   `HostError` for errors, `calternal_core::JobState`/`JobView` as the wire types. No new
   ID or error type.
2. Dependencies: `futures-util` (already in the tree via axum) for the SSE body stream;
   `private_fs` imported once per file as `use calternal_auth::private_fs;`.
3. Global state: one map lock per lookup (space connection, job route, live stream),
   never held across I/O; each space has its own job connection, held per short
   transaction including its fsync (documented in `jobs.rs`). The run semaphore is a
   deliberate server-wide bound.
4. Blocking: job SQLite runs on blocking tasks or worker threads; session checks from
   the worker thread use `Handle::block_on`, never on a runtime thread.
5. Read/flush counts: above.
6. Constant work: manifests and schemas parsed once at startup (unchanged).
7. Bounds and retention: README "Job bounds"; final jobs 7 days / 1024 per space.
   **Needs sign-off**: these numbers are mine.
8. No compatibility code: `item_operations` removed outright; no shims.
9. Spaces by UUID: job files and secret owners use the space UUID; the content-space
   key is only used where the existing store requires it.
10. One place per fact: job operations are declared once in the core catalog and
    policy table; plugin rows still derive from manifests.
11. Unwired code: every new public item has a non-test caller.
12. Reuse: per-space SQLite layout mirrors `calternal-content`'s `content.db`; receipts
    reuse the content journal (`record_receipt`); durability uses the same WAL+FULL
    settings as `Store`.
13. Docs gate: `scripts/check-docs.sh` passes (it does not yet cover
    `calternal-server`; its modules carry `#![deny(missing_docs)]`).
14. Docs: server, plugin-host and conformance READMEs updated; this report.
15. Formatting: touched files are rustfmt-clean. Pre-existing unformatted files
    (`store.rs`, `bounded.rs`) were left alone to avoid conflicts with other lanes.
    Over 600 lines: `worker.rs` (752, was 650), `jobs.rs` (~650), `api.rs` (pre-existing
    1100+). Splitting `worker.rs` (pool vs. session thread) is the next step.

## Not done / limits

- `claimed_text: false` should place files on userdata-hdd; `calternal-content`
  stores every kind under userdata-ssd today.
- Jobs can only be waited on by children; there is no host "wait for the user" yet.
- No UI.
- Fairness between spaces for the 4 run permits is first come, first served.

## Open questions

- Should a job stop when the starting device signs out (now), or only when the account
  loses access?
- Who may watch a job's stream: any member with `space.read` (now) or only its starter?
- Who may cancel: the starter or `plugins.manage` (now)?
- Retention and bounds above: are 7 days / 1024 final jobs per space acceptable?
