# Retrying acknowledged file placement — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Prior placement trace](perf-followup-2026-11-18.md#file-placement-failures)

The placement worker schedules a short retry when work fails. Previously, if
that retry could not acquire a space slot, it silently skipped the slot and
could sleep for up to 30 seconds. A slot holder that does not use the ordinary
content admission path can release the lock without waking the worker. The
search worker's journal-head check is one such holder. The retry lapse was in
the deferred placement worker before the WAL visibility reader was added.

The historical 100,000-Note sample acknowledged in 52.36 ms and timed out
1,000.48 ms later. Its latest bytes were present, the outbox was empty and
the item still had a pending mark when the fixture was stopped. The trace
records successful placement completions and does not name the first delayed
boundary in that sample. Both a directory flush still in progress and a
failed bookkeeping attempt can leave this observable state.

The store regression drives the real placement worker step. A temporary
SQLite trigger rejects its completion mark after fsio has published the file.
It verifies the acknowledged revision, latest bytes, empty outbox and
pending mark. It then removes the failure, holds the slot through the retry
and releases it without sending a wake. The original worker loses the short
retry at this point. The fixed worker schedules the short retry and marks
the same revision materialized, preserving its file bytes.

| Verification on dev-01 | Result |
| --- | --- |
| Regression with the old retry policy | Failed at the busy-slot retry assertion |
| Content suite with the fix | 17 passed |
| All-feature workspace and opt-in real-server check | 602 passed and one opt-in pass |
| Standalone conformance with `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal` | 117 passed |

The workspace crash test passed at `after_placement` and the other durable
boundaries. Integration, plugins, contract export, docs and both formatting
checks passed. The docs gate passed after regenerating the storage reference
from its updated source.

The worker still attempts each cached slot without blocking unrelated
spaces. On contention it keeps the 50 ms retry instead of assuming the
slot has no work. There is no request-path change, extra flush or new
worker. The worker may make a short pass over cached slots while a lock is
held, even if the contended slot turns out to have no pending work.

A person can read the saved revision in the app and see the newer bytes
in the file. Later saves can be refused in the space once the oldest
pending placement reaches one second. Before the fix, a new wake or the
idle sweep could finish placement. After a restart, first access opens the
space and recovers pending durable heads before exposing them. If the
matching bytes are already there, recovery validates and re-syncs them
before marking placement complete. The original journal receipt and
revision are reused.

## Simplification opportunities

Keep placement timing at its own post-acknowledgment durability boundary.
Record whether a delay happened before file publication, during a sync or
while waiting to mark completion.
