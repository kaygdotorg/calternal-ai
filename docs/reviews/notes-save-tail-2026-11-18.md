# Notes save tail and retirement fence — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Prior delivery trace](perf-followup-2026-11-18.md)

The original 100,000-Note update took 966.13 ms through the HTTP response
body, followed by 44.21 ms to observe file placement. The durable journal
returned before the 17.79 ms outbox retirement. The serial driver had to
complete updates 12 and 13 and observe their placements before update 14
could reach its journal commit. Those two measured intervals total at least
1,873.24 ms. The commits for updates 12 and 14 are only 977.84 ms apart.
At least **895 ms** of update 12 therefore elapsed before its durable
journal commit. The archived capture has no subphases for that interval.

The follow-up used release builds from dev-01 with `nix develop` and ran on
perf-02 under `/usr/local/bin/perf-lock.sh`. Both arms used the same server
with the placement retry fix and disposable tracing. The monotonic stamps
correlate client HTTP, authorization, receipt lookup, slot acquisition,
content commit, journal commit, retirement, and return. A dev-built preload
timed file and SQLite `fsync` and `fdatasync` calls of at least 0.25 ms and
identified their descriptors. Slow placement and search pages were stamped
independently. Each arm scheduled 120 serial writes. Disk-cold refers to
dropping the OS page cache before server launch. Requests and overlapping
background work can warm the files during an arm.

## 1,000-Note tail

| Cache | Complete saves | P95 | P99 | Max | P95 after subtracting retirement |
| --- | ---: | ---: | ---: | ---: | ---: |
| Warm | 720 | 30.28 ms | 42.45 ms | 52.80 ms | 20.39 ms |
| Disk-cold | 240 | 34.19 ms | 49.92 ms | 91.61 ms | 23.76 ms |

Each of the eight 1,000-Note arms completed and its p95 was 27.12–37.07 ms.
The previous budget misses remain in the evidence. The warm p99 save spent
20.87 ms in the content transaction. Two syncs of the WAL and one sync of
the main content DB account for 20.38 ms within that span, consistent with
a checkpoint. It then spent 6.99 ms publishing its journal event and
13.29 ms retiring the intent. The 52.80 ms warm maximum spent 14.67, 20.86
and 15.95 ms in those three phases. Search index syncs overlapped that
maximum, with negligible wait on the space lock. The overlap alone does
not establish contention as a cause. The disk-cold maximum spent 19.84,
11.39 and 24.63 ms in the transactions and another 19.23 ms between
retirement and the client's response headers.

## 100,000-Note tail

| Cache | Complete saves | P95 | P99 | Max | P95 after subtracting retirement |
| --- | ---: | ---: | ---: | ---: | ---: |
| Warm | 720 | 58.29 ms | 98.39 ms | 131.91 ms | 40.21 ms |
| Disk-cold | 480 | 34.04 ms | 53.17 ms | 121.50 ms | 23.80 ms |

All ten arms completed. Warm arm p95 ranged from 35.01 to 70.92 ms. Disk-cold
arm p95 ranged from 29.59 to 35.26 ms. The fixture did not reproduce
the 966 ms tail. The 131.91 ms warm maximum spent 43.48 ms syncing the
content WAL, 51.31 ms syncing the journal WAL, and 34.48 ms across the
retirement WAL and main-DB syncs. The warm p99 save spent 76.45 ms in one
retirement WAL sync. The warm p95 request included a retirement checkpoint,
with 20.46 ms across two WAL syncs and a main-DB sync.

Search checkpoint syncs overlapped those warm samples. The measured wait
to take the content slot was below 0.02 ms in the cited warm and cold
maxima. The search overlap has no causal control in this run. The cold
121.50 ms maximum was the first save in its arm and spent 84.27 ms selecting
its filename, where the inherited name cache fills from the directory.
Subsequent saves reuse the cache. The measured slow samples establish
distinct filesystem causes and do not diagnose the uninstrumented
895 ms before the historic journal commit.

The subtraction is an arithmetic ceiling for removing the retirement span
from each observed request. It does not predict scheduling under a worker
that retires outbox rows concurrently. At 100,000 Notes it leaves a 40.21 ms
warm p95 and an 81.16 ms warm p99. Release traces and hashes are retained
under `~/calternal-ai-perf/profile-52-tail/` on perf-02.

## Candidate for a measured prototype

Keep the content and journal receipts FULL. Let ordinary Notes
acknowledgment follow the durable journal commit and defer the FULL
outbox retirement to the existing per-space placement worker. Use a
bounded published prefix of outbox intents. The worker can retire that
prefix in one transaction before placing its files. Outbox recovery and
non-Notes operations keep the existing ordering. The worker's slot lock can
still shift retirement latency onto the next request if a batch precedes
it. Measure bursts, a stalled disk, and large-cache checkpoints before
approving an implementation. The observed arithmetic gain is an upper
bound.

Use a paired search-ready/rebuild run to test whether index checkpoint I/O
raises the remaining content or journal sync tail. A slower rebuild rate
could trade update latency for longer time to searchable results. Measure
that trade before changing the index worker.

The prune fence must work across process restart. A space's journal prunes
receipts before it admits its first reader today. Before either startup
or periodic pruning can delete a receipt, it must protect every request
still represented by an outbox intent. Read that set through the existing
content store boundary and have that space's journal writer exclude the
keys during its prune transaction. The content view must observe only
retirements past the FULL boundary. Keep the fence active through startup
reconciliation. If the content view is unreadable, skip that space's prune
and surface the storage error. Keep the oldest pending age and bound queued
rows so a failed retirement cannot grow indefinitely. Test erasure and epoch
recovery with the same fence. A new receipt published after the prune
snapshot belongs to the same serialized journal writer and cannot be
deleted by that prune.

| Stop or race boundary | Required outcome |
| --- | --- |
| Before FULL content commit | No acknowledged change or new journal entry. |
| After FULL content commit, before the journal commits | The durable outbox is the recovery source. First access publishes one journal entry and its receipt, then retires the intent. |
| After FULL journal commit, before or after HTTP acknowledgment | The receipt is pinned by the outbox. Retry returns the committed reply. Recovery retires the intent without another event. |
| Interrupted FULL retirement | SQLite recovers either the old outbox row or its durable deletion. A surviving row still pins its receipt. |
| App down or retirement blocked past receipt expiry | Startup or periodic pruning preserves the receipt while its outbox row survives. Once retirement is durable, normal expiry makes a later retry a new request. |
| Two writers replay or retry around the pruning boundary | Immediate retries see the original authorized reply. An expired request can be new only after retirement is durable and pruning has committed. |
| After retirement, before the file completion mark | The durable current head remains pending. First access validates or places the file without replaying the retired event. |
| Unreadable outbox view during pruning | Preserve receipts for the space and retry. A write backlog uses the existing placement backpressure and a fixed row bound. |

## Simplification opportunities

Batch the durable retirement of the published prefix at the placement worker
if measurements show it helps acknowledgment tails. Keep each receipt's
retention rule on the journal writer.
