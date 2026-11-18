# Scoped plugin settings and installed diagnostics

## Scoped settings and slots

- A setting can declare `scope: "personal"`. Its default still lives in
  the manifest. The signed-in member can read and edit their own override
  using `plugins.personal_settings.get` and `plugins.personal_settings.set`.
  The worker uses its existing `read_setting_v1` call.
- The account's `input.db` migration 2 to 3 adds personal overrides and
  receipts alongside its existing input binding. Each account can store
  1,024 overrides. Values are at most 4 KiB. The write transaction checks
  the base revision, co-commits the value and receipt, and invalidates
  the caller's private input feed. It holds at most 4,096 live receipts,
  with the configured receipt window. No auth.db migration is required.
- An owned-slot subscription returns the same capability-filtered
  contributions as a normal listing, with a replacement on activation,
  registry or relevant authority changes. It consumes a lease from the
  existing space journal listener limit and rechecks the live session
  before each response.

## Installed diagnostics

- `plugins.diagnostics` requires `plugins.manage` in the requested space.
  It reports the last observed runtime readiness, up to eight recent
  invocation errors and up to 4 KiB of the last failed worker's stderr.
  The server bounds observations to 64 recently active spaces per
  package revision and exposes observations for one hour. They are
  discarded when the process restarts.
- The sandbox drains each worker's stderr continuously into a bounded
  buffer and sends the tail only on unsuccessful exit. The runtime uses
  the same space binding as the worker. Error metadata stores operation
  and reason codes without request values. Worker-written stderr can
  contain that worker's own space data.
- Successful warm worker calls make no diagnostics reads, writes or
  flushes. A manager read takes one registry snapshot, reads activation
  and copies a bounded observation from memory between authority checks.

## Verification

- On dev-02, focused conformance passed 3 settings cases and 4 owned-slot
  cases. The installed diagnostics case checks the stderr bound, crash
  readiness, reader denial and separation between spaces.
- The full verification gate is pending.

## Simplification opportunities

Share the transport's bounded byte-tail logic with the sandbox pump.
Reuse the browser's existing live resource subscriptions when wiring
owned slots into Claude's renderer work.
