# Plugin author readiness, logs and timing

2026-11-18 · [Lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

## What an author sees

`calternal plugin diagnostics <package-id> <operation>` on a development
server shows, for the caller's own Manager session in the draft's space:

- the state (`ready`, or `failed` before any revision has loaded) and the
  loaded revision;
- validation, handshake and publication time of that revision;
- the last five errors for this caller and operation: the reason code
  (`input_schema`, `output_schema`, `view_schema`, `worker_busy`,
  `worker_machine_required`, `worker_capacity` or `worker_failed`), the schema
  field path for the three schema reasons, and elapsed time;
- the last candidate that failed before publication, with its phase
  (`validation`, `handshake` or `publication`), a message cut to 300 characters
  and elapsed time. A failed candidate never replaces the ready revision.

Output errors name the declared output schema path, never a value the worker
returned. The same command works with `--json`.

## Review of the parked checkpoint

The checkpoint from the previous agent was sound and was reused: the output
path helper, the typed `ReceiveError::Output`, the per-revision bounded error
history, the private-socket diagnostics request with a Manager check at
admission and again before reply, and the revision-identity check at delivery.
It did not report a candidate that failed, so an author whose edit was rejected
saw the previous ready revision with no explanation. This change adds that
(`FailedCandidates` on the registry, phase tracked through publish) and the
state `failed` when no revision has loaded.

## Bounds, retention and migration

| Kept | Bound | Gone when |
| --- | --- | --- |
| Recent errors | 5 per caller and operation, up to 64 pairs per revision | The revision is replaced, or the server restarts |
| Phase timings | One record per revision | The revision is replaced |
| Failed candidate | One per package and space, up to 16 packages, message 300 characters | The next successful publish of that package, or a restart |

All of it is in memory on a development-draft build, outside any database or
file. No stored schema or owned file format changed, so no migration is needed,
and nothing assumes a fresh database. A production build has no diagnostics
view and none of this state.

## Measurement

Edit to ready stays under one second on perf-01 under the lock, 829 to 925 ms
p95 across four runs with diagnostics against 833 to 867 ms across three
without. See [the measurement](../measurements/plugin-readiness-2026-11-18.md).

## Not built

- **Production viewers.** The plan called for scoped reason codes and timings
  for installed packages. That needs a new authorized operation, its contract
  and a retention decision, so it is left for the next piece rather than added
  to the development channel.
- **A revoke race test.** Revocation is covered by the second Manager check
  before the reply, and by the refused non-Manager and foreign-space callers in
  the conformance case. A deterministic test that revokes between the two
  checks was not written.
- **Busy and capacity errors** share the recording path with the covered
  schema errors but have no conformance case of their own. Handshake failure
  (a crashing worker), input and output schema errors, the five-error bound and
  isolation are covered.
- **Raw worker stderr** is still not captured by the host, so a crashing
  worker's own output does not appear. The plan's local-only raw channel needs
  that capture first.

## Simplification opportunities

- `Plugin::record_draft_error` and the `invocation_started` timer in
  `invoke_plugin` exist only for development builds. A single recording hook in
  the worker completion path would remove the cfg in the API handler.
- `DraftDiagnostics` and `FailedCandidates` repeat the same bounded
  most-recent-first list. One small generic bounded list would serve both.
- The publish phases (`validation`, `handshake`, `publication`) are set by hand
  in `drafts.rs`; a typed `Phase` enum returned in the error would remove the
  shared mutable label.
