# Warm 100,000-Note HTTP save gate

[Forgejo #89](https://git.kayg.org/kayg/calternal-ai/issues/89)

## Historical evidence

The product metric is the per-arm p95 of `notes.update`, with a 40 ms
ceiling. The driver creates one account with a software passkey and uses its
personal space. It provisions 100,000 Notes through the real API with seed
20261117, stops the server, and copies the three roots for each arm. Warm
arms sync the copy, keep the OS page cache, open the space through Notes
list, and subscribe to the space and account SSE feeds. Each sample edits
the same note, using its latest returned revision and the body
`seed 20261117 save {i}`. Its title stays fixed.

The client starts timing just before sending the built loopback HTTP
request and stops after reading the complete response body. Client-side
JSON encoding and decoding, SSE observation, file placement and receipt
replay have separate timing. The driver waits for placement and the
correlated event before advancing the serial save loop. The acknowledgment
includes the FULL content/outbox commit, FULL journal event and receipt
commit, and FULL outbox retirement. The window-engine lane used the same
HTTP timer, with one provisioning change to use the baseline server's
schema for the shared seed. Its Step 0 and 8 KiB engine probes time
`Content::write` directly. [Product driver](../../crates/perf/src/notes_scenario.rs) · [HTTP timer](../../crates/perf/src/http.rs) · [Window lane](https://git.kayg.org/kayg/calternal-ai/src/commit/ef82bc31/docs/reviews/window-engine-2026-11-19.md)

| Source and VM | Warm arms × saves | Arm p50 range (ms) | Arm p95 range (ms) | Arms within 40 ms |
| --- | ---: | ---: | ---: | ---: |
| `40a3848d`, perf-01 integrated release matrix | 5 × 100 | 21.54–22.66 | 29.23–31.86 | 5/5 |
| `2a08bb89`, perf-01 archived plain A/B with `f0d485d` | 3 × 100 | 22.71–23.38 | 30.90–36.87 | 3/3 |
| Package-store search repeat, perf-01, same server SHA in both arms | 6 × 100 | 22.40–23.17 | 29.37–36.06 | 6/6 |
| `039c0a65` source, perf-02 plain repeat | 3 × 100 | 22.19–30.20 | 33.98–60.26 | 1/3 |
| `039c0a65` source, perf-02 timing repeat | 3 × 100 | 21.71–22.41 | 31.40–38.42 | 3/3 |
| `039c0a65` source, perf-02 later syscall trace, plain variant | 3 × 150 | 23.43–27.89 | 31.43–57.71 | 2/3 |
| `039c0a65` source, perf-02 later syscall trace, timing variant | 3 × 150 | 22.05–22.68 | 30.01–31.72 | 3/3 |
| `85add29a`, perf-01 window-engine control | 3 × 100 | 37.18–46.68 | 55.88–81.65 | 0/3 |

The November 18 stage artifact's run metadata names
`b876bf2+pruning-fence+stage-timing`. Its source is represented by
`039c0a65`, the timing commit on top of `e1cbdac3`. The final qualifying
instrumented arms use that source with `perf-timings` enabled. A release
with the same source and the feature off missed in two of the 100-save
arms and one of the 150-save arms. The plain package-store repeat used
a search-ready prelude, varied search on and off, and recorded binary
digests with arm labels instead of an exact source commit. The pinned
`2a08bb89` plain control also missed in one of ten arms in a longer
same-binary run. [Stage report](notes-save-stages-2026-11-18.md) · [Search report](notes-search-worker-2026-11-18.md) · [Integrated release](perf-fixes-2026-11-17.md)

The 50 ms save-to-subscriber and 10 ms commit-to-client gates are separate
observations. Both have passed on runs in which acknowledgment has missed.
The 75 ms working expectation for perf-02 reflects historical storage
tail measurements. The product gate remains 40 ms. [Delivery profile](perf-followup-2026-11-18.md) · [Storage profile](perf-compaction-and-storage-2026-11-18.md)

## Locked perf-01 rerun

The original harness and plain historical binary were copied with the
checksums in the archived run metadata. The plain current server was
built on dev-01 from `f41c3429` with `nix develop` and copied with its
checksum. `85add29a` and `f41c3429` have identical build inputs outside
documentation. `origin/dev` moved to `0466f80c` during the run. That
commit changes the v2 contract and documentation and leaves the measured
Notes runtime and bundle inputs identical.

The preflight tried one shared seed from the historical release. The
current server refused its older installation root marker. The harness
therefore provisions a fresh installation per release with the same
seed, corpus, request order, timer, and plugin build procedure. Each arm
opens an independent copy of its release's stopped seed. The fixture
contains the storage format and derived state that release creates.

Both identical-binary runs completed six arms of 100 saves. All
file-placement checks passed. Current dev missed every arm. The historical
plain release passed three arms and had a lower median in every arm than
the lowest current median.

| Pair | Historical baseline p50 / p95 (ms) | Historical head p50 / p95 (ms) | Dev baseline p50 / p95 (ms) | Dev head p50 / p95 (ms) |
| --- | ---: | ---: | ---: | ---: |
| 0 | 22.187 / 33.454 | 21.809 / 36.143 | 36.746 / 49.905 | 42.497 / 58.138 |
| 1 | 22.418 / 43.007 | 26.094 / 43.128 | 36.060 / 50.061 | 36.541 / 50.304 |
| 2 | 25.200 / 34.539 | 23.032 / 59.194 | 36.277 / 51.399 | 35.581 / 50.505 |

Historical placement observations occurred about 14–16 ms after the
acknowledgment at p50. Current placement observations were about 0.07 ms
after it. Save-to-subscriber p50 remained approximately 16–19 ms on both
releases. This points to work between journal delivery and the HTTP
response.

A second driver build changed provisioning to use a fresh seed from each
release. It kept the HTTP client and Notes timing scenario and alternated
the arms under one lock. All six 100-save arms and placement checks completed.

| Pair | `039c0a65` plain p50 / p95 (ms) | `f41c3429` plain p50 / p95 (ms) |
| --- | ---: | ---: |
| 0 | 21.579 / 29.292 | 43.748 / 53.031 |
| 1 | 22.196 / 37.759 | 36.125 / 51.987 |
| 2 | 22.031 / 49.679 | 36.848 / 46.922 |

## Regression commit

The adjacent source bracket used the same separate-seed driver on locked
perf-01. The parent `0d66ebad` and item-sharing commit `dd949cec` each ran
three 100-save arms with the matching bundles. All six completed and
passed placement. The steady-state median rose by 13.95–16.23 ms. The
parent had one 0.033 ms p95 miss, and every sharing arm missed by at least
12.61 ms.

| Pair | Parent p50 / p95 (ms) | Item sharing p50 / p95 (ms) |
| --- | ---: | ---: |
| 0 | 22.622 / 36.853 | 36.576 / 55.163 |
| 1 | 22.089 / 36.065 | 38.320 / 52.611 |
| 2 | 22.105 / 40.033 | 36.549 / 56.563 |

Acknowledgment-to-placement p50 moved from 14.15–14.20 ms to
0.07–0.08 ms. The adjacent bracket, live trace and delivery-only fix
identify the sharing delivery wait as the sustained regression.

## Stage attribution

Six 100-save diagnostic arms ran on locked perf-01 at `f41c3429` with the
release timing feature and a syscall preload. The two trace arms included
a control for live-room work. The fixture uses native Notes operations
and SSE, with zero plugin activation or room join requests. Both arms
used the real owner session and recorded the same placement flush counts.
Timing ranges are across arms with separate percentiles for each span.

| Stage | Arm p50 range (ms) | Arm p95 range (ms) |
| --- | ---: | ---: |
| HTTP acknowledgment | 36.26–46.63 | 44.64–63.96 |
| Parse request envelope | 0.006–0.006 | 0.007–0.010 |
| Resolve and authorize | 0.259–0.278 | 0.297–0.372 |
| FULL content and outbox commit | 6.86–9.12 | 9.05–14.86 |
| Journal publication including FULL commit | 6.96–8.61 | 9.90–13.26 |
| FULL outbox retirement | 6.68–8.00 | 8.90–13.33 |
| Retirement return to HTTP response serialization start | 13.93–18.37 | 17.51–23.57 |
| Placement syncs inside that interval | 13.19–17.52 | 16.55–22.08 |
| Response serialization | 0.008–0.009 | 0.011–0.012 |

Every observed save had both placement flushes between retirement and
serialization. In each 100-save arm, the content commit issued 97 single
syncs, one double and two triples. Journal publication issued 97 singles,
two doubles and one triple. Retirement issued 98 singles and two doubles.
The three FULL SQLite boundaries remain independent of the file and
directory syncs, with extra syncs on checkpoint-crossing saves.
The global item route costs a lookup on update. No search-ready marker or
search-worker sync occurred in this fixture. Search indexing adds zero
foreground reads or flushes, and this run provides no search-ready latency.

`dd949cec` changes the post-save capability check to
`resolve::item_capabilities`. Its indexed reverse-membership query
acquires the space write slot. After retirement, the write releases that
slot and wakes the placement worker. When placement acquires the slot first,
delivery waits until the worker has replaced the file, synced the file
and directory, and released the slot. The requester pays for that wait
even when its space role alone authorizes delivery.

Logical read counts below describe a warm, healthy owner save with a
fixed title, current base, empty prior outbox and warm session cache.
They count SQL statements and guarded file reads, with checkpoint and
SQLite page I/O reported separately.

| Read or flush | Historical | Current | Candidate |
| --- | ---: | ---: | ---: |
| Global route, resolution and publication | 2 | 2 | 2 |
| Protected authority statements | 2 | 3 | 3 |
| Reverse collection membership in the write slot | 0 | 2 | 1 |
| Room-kind probes | 0 | 2 | 2 |
| Foreground content reads for metadata, header and outbox | 6 | 6 | 6 |
| Journal receipt probes | 3 | 3 | 3 |
| Guarded file reads, foreground and placement | 1 + 1 | 1 + 1 | 1 + 1 |
| FULL content, event and retirement transactions | 3 | 3 | 3 |
| Placement file and directory flushes, total | 2 | 2 | 2 |

The row-level receipt delta commit `8c28dfb6` is outside the measured
dev ancestry. The thread append barriers serve a different operation.
Upgrade handling explains the fresh-seed preflight result. The observed
extra wait comes after the Notes FULL commits.

## Fix and verification

Delivery now checks the current space role first. When that role satisfies
the operation policy, delivery can proceed without taking the content
writer slot. A caller whose role is insufficient still receives the
current item and collection grant check. That fallback pays one additional
protected role statement. The save path continues to check item-only
editor authority inside the content lock before committing.

The plain candidate `c1422cad` and control shared a stopped seed on locked perf-01.
Both releases ran all three 100-save arms and passed all file-placement
checks. Each candidate arm met the 40 ms p95 save target.

| Pair | `f41c3429` control p50 / p95 (ms) | Delivery fix p50 / p95 (ms) |
| --- | ---: | ---: |
| 0 | 36.665 / 48.984 | 22.196 / 33.025 |
| 1 | 36.379 / 50.716 | 22.289 / 34.551 |
| 2 | 37.225 / 51.536 | 22.798 / 29.355 |

With the fix, acknowledgment-to-placement p50 was 14.04–14.27 ms and
maximum was 43.09 ms across the three arms. Save-to-subscriber p95 was
23.23–25.40 ms. The harness returned its ordinary partial-qualification
status for the plain matrix. It reported zero failed arms and zero budget
breaches in the candidate arms. This qualifies the warm owner HTTP
workload. Item-grant-only save latency and production storage latency
remain unmeasured.

On dev-01, formatting, checked release builds and `scripts/check-docs.sh`
passed. The three authorization unit tests and nine resolver tests passed.
The new unit test holds the
space writer slot while checking the owner's delivery authority. It times
out under the old delivery path and completes while the writer remains
held with the fix. The resolver suite covers item and collection grant
fallback, group edits and revocation.

## Simplification opportunities

Record an exact server source commit, feature list, fixture provisioning
revision, and checksums for the server and harness for every gate run.
Retain raw distributions so an identical-binary repeat can quantify
host variation before a bisect.
