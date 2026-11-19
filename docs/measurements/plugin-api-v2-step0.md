# Plugin API v2 step 0

Base `d2ebcbc8` (revision 5). Disposable integration fixtures exercised the
query, staged save and carrier gates from §8. Binaries were built on
`calternal-dev-01-compute02` with `nix develop -c cargo test --locked --release …`.
Measurements below are from `calternal-perf-01-backup01` under
`/usr/local/bin/perf-lock.sh` on ext4. The filesystem cache could retain
pages between repetitions. Cold query here means a fresh read-only SQLite
connection and an asleep plugin runtime. All p95s use nearest rank.

## Cold indexed page

100,000 synthetic note heads in the real `calternal-content` SQLite
schema, with its discovery indexes, collection membership triggers and
FTS5 schema. The fixture changes grants, indexed labels and search content
between requests. The manual collection contains 10,000 notes (the existing
engine ceiling). The dynamic collection is a dense indexed-label predicate.
Both use a mutable per-item visibility table in the fixture. The search and
collection search rows below are candidate SQL over the existing FTS schema.
The production search engine uses a title/body tier rather than the candidate
BM25 score. The unscoped member control receives a known key from the fixture.

Offsets and ordered triples in the table are **0 / 10,000 / 90,000**.
The window is 200 rows. Candidate visits count rows reaching the instrumented
predicate. They exclude nested index probes and FTS internal posting reads.
SQLite VM steps capture additional work. Each timed cell has 100 repetitions.

| Injection | Pass condition | Measured on locked perf-01 | Verdict |
| --- | --- | --- | --- |
| Manual collection + ACL + newest, current indexes | Ordered seek, offset-independent work, ≤100 / 500 / 500 ms p95 | 26.385 / 28.509 / 29.039 ms p95. 10,000 / 10,000 / 10,000 visits. 200 / 0 / 0 rows. Membership scan with two keyed lookups and a temporary sort. | Miss. The collection limit explains the empty deep windows. |
| Manual collection + ACL + newest, scoped index | Ordered seek, bounded visits even for empty deep windows | 0.169 / 0.089 / 0.081 ms p95. 201 / 0 / 0 data and locator rows visited. Deep offsets are outside the collection. | Pass for reads. |
| Dynamic collection + ACL + newest, current indexes | Ordered seek, offset-independent work, ≤100 / 500 / 500 ms p95 | 1.116 / 11.777 / 92.368 ms p95. 210 / 10,742 / 94,999 visits. 5,825 / 267,207 / 2,358,245 VM steps. Walks the update index and probes label and ACL for each candidate. | Miss. Linear offset scan. |
| Dynamic collection + ACL + newest, scoped index | Ordered seek and ≤201 visits at each offset | 0.296 / 0.182 / 0.181 ms p95. 201 visits and 200 output rows at every offset. | Pass for reads. |
| Ranked search + ACL | Search scoped corpus, bound work, meet first and deep latency | 124.780 / 124.592 / 151.611 ms p95. 100,000 visits per offset. FTS postings walk and temporary sort. One private-only body edit changed the score of an unchanged readable row in the shared-corpus BM25 candidate. | Miss. First-window time, work bound and corpus isolation. |
| Manual collection + ranked search + ACL | Apply corpus scope before ranking and bound visits | 74.698 / 72.867 / 73.143 ms p95. 100,000 visits per offset. FTS postings walk, membership and ACL probes, temporary sort. | Miss. Reordering the predicates in SQL still leaves a shared FTS walk. |
| Member keyset control | Indexed range with supplied anchor | 0.858 / 1.062 / 0.911 ms p95. 200 visits and 1,812 VM steps per offset. | Pass for keyset. Arbitrary rank resolution is outside this control. |
| Covering jumps with maintained scope count | Low single-digit milliseconds at offset 50,000, capped walk | Dynamic offsets 0 / 10k / 50k / 90k p95 0.154 / 0.676 / 2.189 / 0.407 ms and 200 / 10,200 / 44,948 / 4,948 visits. | Pass with the covering-walk amendment. |
| Content save with projection in the same transaction | ≤40 ms p95 through the qualified scope ceiling | 27.792 / 24.775 / 29.261 ms p95 at 1 / 4 / 16 scopes. Extended arms measured 31.299 ms at 32, 38.092 ms at 64 and 47.064 ms at 256. | Pass at the 32-scope contract ceiling. The 256-scope arm missed. |
| Filtered exact count under grant/content churn | Meter separately and refuse unbounded counts | 98.914 ms p95, 98,113 visits p95 and 2,155,344 VM steps p95. Term index range with item and ACL probes. The fixture withholds the unqualified count. | Miss for general exact count. |
| Revoke after the first row of an open SQLite snapshot | Zero rows and counts released, zero runtime wakes | 0 / 0 / 0 in 100 injections. The fixture buffered the old results and compared the live grant revision before delivery. | Pass for the fixture delivery check. |
| Burst invalidations across tabs | Coalesce fan-out and count work | 80 simulated invalidations over 4 ticks. 128 fixture subscriptions admitted, 52 refused. 512 pushes total, 4 per admitted subscriber. 4 count starts and zero stale count deliveries. | Pass for the fixture coalescer. Live server wiring remains to be exercised. |

The current-index rows and count each use one SQLite statement, zero write
commits and zero flushes. The churn injection commits separately. The
scoped design adds one audience/collection version read, one rank point
lookup and one ordered covering range query. Its ACL decision is applied
when maintaining the projection, so the ordered seek skips no hidden rows.
SQLite plans are `SEARCH scoped_rank USING PRIMARY KEY
(collection=? AND audience=? AND rank=?)` and `SEARCH scoped_membership
USING PRIMARY KEY (collection=? AND audience=? AND (newest,id)>(?,?))`.
Neither plan uses a temporary sort. The read visit totals omit the one
fixed metadata row and internal B-tree node accesses.

The scoped indexes occupy **15,974,400 bytes (15.23 MiB)** for 100,000
source items, 104,625 authorized membership keys, two overlapping collections
and one audience. Each mutation below is one isolated SQLite FULL WAL
commit, with one normal WAL sync. Timings include index maintenance and
commit, in addition to the existing engine's write cost.

- Item save moving one row in both collections changed two membership keys
  and removed and reinserted 104,625 rank rows. 234.806 ms p95 (100 runs).
- One dynamic membership change changed one key and removed and reinserted
  approximately 94,948 rank rows. 217.288 ms p95 (100 runs).
- One grant-row change changed one key and removed and reinserted approximately
  94,948 rank rows. 261.200 ms p95 (100 runs). The harness batches no grants.

### Covering index and maintained count

The same fixture has 94,948 authorized dynamic members and 9,677 manual
members. This variant stores just the covering membership B-tree and one
transactionally maintained exact count per scope. It uses **9,904,128 bytes
(9.45 MiB)** per 100,000 source heads for the two collections and one audience.
Only sort key and ID are read from the B-tree during a jump. Content metadata
can be fetched for the returned rows separately. Jumps use the count and
covering seek within one read snapshot, with zero write commits or flushes.

Dynamic jump measurements by offset (p50 / p95 in milliseconds):

- 0, from newest — 0.138 / 0.154 ms, 200 index visits.
- 10,000, from newest — 0.605 / 0.676 ms, 10,200 index visits.
- 50,000, from oldest — 2.032 / 2.189 ms, 44,948 index visits.
- 90,000, from oldest — 0.350 / 0.407 ms, 4,948 index visits.

For the manual collection, offset 0 was 0.140 / 0.167 ms and 200 visits.
Offsets 10,000, 50,000 and 90,000 were empty from the exact count and
took 0.062 / 0.075, 0.062 / 0.077 and 0.062 / 0.080 ms respectively,
with zero membership visits. The follow-up measured the manual collection
through its last supported range within the 10,000-member engine cap.
In this ACL projection, the last full page at offset 9,477 took 0.143 /
0.158 ms p50 / p95 and 200 visits. The last visible row at offset 9,676
took 0.085 / 0.101 ms with one visit, and offset 9,677 took 0.061 /
0.079 ms with zero visits (100 each). Populated deep offsets use the dense
dynamic collection.

Each cell is 100 fresh read-only connections on locked perf-01. The
covering plans all use `SEARCH scoped_membership USING PRIMARY KEY
(collection=? AND audience=?)`, with no temporary sort or lookups per skipped
record. The 50,000 jump used 135,658 VM steps. Reverse reads are reversed
again before delivery into ascending key order. The largest skip equals
half the authorized scope before the window width is included.

Home and End each visited 200 membership keys at 0.177 ms p95
(p50 0.149 / 0.150 ms). The second page, using a tuple cursor in each direction,
took 0.195 / 0.178 ms p95 (p50 0.161 / 0.161 ms), again 200 visits.
An exact count took one point lookup, 0.072 ms p50 and 0.087 ms p95.
Each measurement had 100 repetitions.

For the standalone maintenance comparison, a sort-only save across two
collections replaces two keys (two removals, two insertions) and updates no
counts. It measured 6.803 / 9.007 ms p50 / p95. One membership removal
deleted one key and decremented one count row at 6.784 / 9.195 ms p50 / p95.
Each measurement used 100 FULL transactions. Both include the fixture's
result-version write. These are independent commits, so their p95 cannot
be added to the content save p95.

A sort-only save touching `k` distinct effective scopes replaces `k` keys
and touches `2k` old/new B-tree entries. If `k` is the number of audiences
*per collection* and the item is listed in `c` such collections, the upper
bound is `2kc` entries. Membership changes also touch the corresponding
counts. A space-member projection plus 100 named and 32 link grants gives
at most 133 direct audience identifiers when all are independently indexed.
The existing limits do not bound the union of inherited scopes across
collections. Qualifying live plans requires a bound on the total
deduplicated effective scopes per item. Reuse audience sets across viewers
with identical rights.

### Combined content save

The fixture extends the same 100,000-head content database with covering
membership and count triggers. It calls the existing `Content::write`
for a warm note with the short, varying body used by the Notes performance
scenario. The timed interval ends at engine acknowledgment. The test completes
userdata placement between samples. Each nonzero arm moves one grant-backed
membership from one scope to another, so the transaction decrements one
scope count and increments another. All remaining memberships replace their
old sort keys. The fixture verifies the committed keys and counts after
every acknowledgment.

In locked perf-01 runs of 100 saves per arm, the zero-scope baseline was
21.864 / 27.077 ms p50 / p95. One effective scope took 21.405 / 27.792 ms,
four took 21.608 / 24.775 ms, and sixteen took 22.117 / 29.261 ms.
These are engine measurements. The HTTP route and live subscriber are
outside the timed interval.

The content update reads one head and its current placement guard, checks the
request receipt, and commits the revision and outbox intent. The triggers
maintain membership and counts in that FULL transaction, with 2 / 8 / 32
old/new keys and two count rows touched at each arm. The existing indexed
intent, routing and receipt reads, journal FULL commit and outbox retirement
FULL commit follow. The projection adds no additional commit or flush.
The extended run kept the original content heads and added 6,000 existing
memberships to each additional audience, up to 257 audience IDs. The timed
item still belongs to exactly the stated number of effective scopes.
With 100 saves per arm, the zero-scope baseline was 20.805 / 23.621 ms
p50 / p95. Confirmatory p95s at 1, 4 and 16 scopes were 28.412, 25.142
and 30.514 ms. At 64 scopes the save touched 128 old/new keys and took
22.270 / 38.092 ms. At 256 it touched 512 keys and took 28.039 /
47.064 ms, crossing the 40 ms p95 gate between those measured widths.
The count changes still touch two rows. The 64-scope arm leaves 1.908 ms
of margin in this engine-only run. The follow-up 100-save arm at 32
scopes used the same extended fixture and touched 64 old/new keys and
two count rows. It took **22.211 / 31.299 ms p50 / p95**, leaving
8.701 ms of headroom against 40 ms.

### What the specification must change

Keep collection + ACL + newest in the required Notes surface. Use
audience-scoped sorted membership and maintained counts. Use tuple seeks for
sequential navigation and a bounded, nearer-end covering walk for arbitrary
jumps. Amend §4.1 to permit this index-only offset walk within a qualified
scope ceiling. Cap the first contract at 32 deduplicated effective
scopes per item, including inherited scopes. Eager ranks rewrote nearly the whole scope on a single
membership change and exceeded the save budget. Until a plan demonstrates
bounded work and corpus isolation, return `422 unsupported_query_plan` for
ranked search and collection + ranked search. Constrain exact counts to
bounded partitions or counts maintained alongside the authorized projection.
A shared FTS corpus with a visibility predicate can expose private statistics. The scoped follow-up below qualifies alternatives to those baseline plans.

## Atomic large save

The fixture uses the content engine's real FULL revision + outbox commit,
store journal publication and receipt, filesystem placement, purge, and
indexed revision slices. It stages provisional chunks in a private
directory using `calternal-fsio`. The adapter checks the exact byte length
and BLAKE3 digest and hands an immutable byte buffer to the engine. JSON
base64 frames use the workspace encoder. Each timed repetition carries
2,097,152 decoded bytes in 32 × 64 KiB chunks. The body is composed entirely
of double quotes, backslashes, tabs and newlines. Direct JSON string
serialization measures 4,194,306 bytes.

| Injection | Pass condition | Measured on locked perf-01 | Verdict |
| --- | --- | --- | --- |
| Stage the 2 MiB body, commit, advance the head and read the saved revision by range | Reconstruct every byte, p95 ≤5 s | 143.243 ms p50, 168.451 ms p95, 199.913 ms max across 100 saves. All 100 reconstructed bodies matched. | Pass |
| 60 s range pin with advanced head, revoke, expiry and purge | Allow the original revision while the pin and grant are live. Release zero bytes after invalidation | 100 successful old-head reconstructions. Zero range releases after injected expiry, revoke or purge. All purges finished inside the original pin lifetime and removed the retained revision rows. | Pass |
| Duplicate begin, changed begin, lost chunk ACK and conflicting bytes | One reservation, matching replay, zero conflicting replacements | 100 concurrent identical begins reused their reservation. 100 changed begins refused. 100 matching chunk replays accepted and 100 conflicting bytes refused. | Pass |
| Cancel races a chunk and stale cleanup races restage | No chunks accepted after cancel. New handle survives old cleanup | Zero accepted post-cancel chunks and zero new-stage losses in 100 of each race. | Pass |
| Revoke, cancel, epoch loss, enrollment removal or run replacement on either side of commit | Invalidation first leaves old head and no new receipt. Commit first keeps one revision/receipt and refuses stale delivery | 10 repetitions per cause and ordering, 100 total. All preserved the expected journal length and receipt state. Zero unauthorized deliveries or extra events. | Pass for the fixture authority fence |
| Process exit with an orphan stage or before content commit | Restart removes the orphan. Zero visible new revisions or success receipts | 10 exits per point, no partial edit or success receipt. Restart swept each stage before admitting another attempt. | Pass |
| Process exit after FULL commit before publication, or after publication before reply | Recovery retains one revision and matching receipt. Replaying by request identity leaves one event | 10 exits per point, 20 recoveries. One new revision and one receipt each. Zero duplicate events, pending intents or foreign receipts. | Pass |

Timed interval covers private stage creation, 32 encoded chunk writes, 32
seal reads, the durable save, a second save that advances the head, and
32 range calls. The stage directory creation syncs its parent once.
Chunk writes have zero synchronous body flushes. Uncommitted uploads may
restage after restart. Each range makes one indexed lookup and one
incremental SQLite BLOB read. No range makes a write commit or touches
the placed file.

For one existing-item save, content checks an indexed request receipt,
loads one item header, reads the placement guard, and commits revision
plus outbox in one FULL transaction. Existing pending placement may need
finishing first. Outbox publication follows with indexed intent, routing
and receipt reads, one FULL journal commit and one FULL outbox retirement.
That is three durability commits on the normal acknowledged update path.
The journal witness and file/directory flushes follow on their existing
schedules. The timed interval includes two updates and can include
placement of the first before the second update.

### What the specification must change

Keep the content FULL transaction as the recovery-visible commit point.
The fixture took an authority fence before writing and released it at the
existing `AfterCommit` hook. Product wiring needs a production fence
at that boundary and release on rollback, with delivery rechecking live
authority. A preflight authorization callback alone leaves a gap before
commit. Keep provisional chunks restageable so per-chunk durability adds
no flush to acknowledgment. Bind the range handle to its exact saved
revision and adjacent base. A 60-second pin is a read authorization
lifetime and purge or revoke ends it sooner. It does not postpone erasure.

## One enrolled channel

One authenticated loopback TCP connection per awake space, opened on a
shared listener. Space A multiplexes two real Node 22.23.3 package
processes. Space B has its own connection and processes. The harness
uses the real machine enrollment, Ed25519 session challenge and bearer
validation from `calternal-machine` and `MachineService`, plus the live
space capability lookup. It uses the existing `plugin-host` child frame
codec, `core` broker method types and the content/store receipt writer.
A space-bound handshake agrees the major, issues the epoch and sets
credits. The test holds one callback for two seconds, gives control
priority over an unsent invocation, and stalls the receiving socket
with a near-cap frame. It counts physical bytes accepted by the writer.

The test ran 100 sequential repetitions on locked perf-01. The stream
used a 300 ms write watchdog and a four-second remote lease. The
startup TCP/session handshake ran on loopback, outside the deployed
HTTPS route and production sandbox scheduler.

| Injection | Pass condition | Measured on locked perf-01 | Verdict |
| --- | --- | --- | --- |
| Two package processes on A. Hold one callback for 2 s and enqueue control behind unsent data | Control and other callback each ≤500 ms p95 while A is writable | Control 0.078 ms p95 (0.782 ms max). Other callback 7.947 ms p95 (13.119 ms max). Control overtook unsent data in all 100. | Pass |
| Stop A's receiver during a physical write | Writer stops mid-frame and watchdog closes A ≤2 s p95 | The 261,330-byte frame wrote 69,619 bytes before stalling in every run. Close was 301.794 ms p95, 301.977 ms max. The first 4,083 bytes were written explicitly before the watchdog test. | Pass |
| Inject local fencing while A's receiver is still blocked | Fence ≤500 ms p95 even with no delivered cancel. Zero old-lease effects | Fence was 51.655 ms p95 from the start of the stall, including the injected 50 ms wait. The local state change was below 1 µs timer resolution. Zero old-epoch or old-ticket effects. | Pass |
| Progress a callback through B while A's physical writer is stalled | B ≤2 s p95 and independent of A | 0.744 ms p95, 0.858 ms max. B completed before A's watchdog close in every run. | Pass |
| Leave A's receiver unread, including cancel and EOF after its last authenticated renewal | Both remote package processes stop within 5 s on every run | 4.004673 s p95 and 4.006205 s max to stop and reap both package processes. 100 out of 100 within 5 s. | Pass |
| Reconnect A, issue a fresh ticket, replay, then send an old-epoch frame with the expected next sequence | Reject old epoch. Authorized replay leaves one durable effect | Handshake 0.472 ms p95 and 0.648 ms max. Reconnect through completion of replay 32.462 ms p95 and 36.122 ms max. 100 old-epoch frames rejected, one stored event per run and zero extra commits. | Pass |

The unladen channel cost used one cohort of 128 idle spaces on the same
locked machine. One TCP connection and two endpoint objects per space.
The Rust objects total 672 bytes per space and the readers reserve
8,192 bytes of buffer capacity. Process RSS rose 556 KiB for the cohort
(4.34 KiB per space across both endpoints). RSS reflects resident pages
at the time of sampling. Linux reported 8,192 bytes of configured
send capacity and 8,192 bytes of receive capacity per endpoint. Configured
capacities describe buffer limits. Package processes and deployed HTTPS
transport are outside the idle-channel measurement.

On an open channel, a control or callback exchange uses bounded socket and
pipe reads/writes with zero storage commits or flushes. The handshake
reuses the bearer lookup and one space-capability check. The fixture's
first effect checks the indexed receipt and appends one receipt/event in
one FULL journal commit with one ordinary WAL sync. Replay reads the
receipt after fresh authorization with zero further commits or flushes.

### What the specification must change

The measured back-pressure, independent callback progress, fencing and
replay gates support the per-space duplex design. Keep machine sync for
enrollment lifecycle, capacity, placement and assignment. Reuse its
authenticated session and revocation data when opening the stream. Keep the
existing per-worker exec mailbox path available during rollout. The default
machine-control lease is 90 seconds and its long-poll hold is 20 seconds.
Use the separate short, renewable space lease for this protocol's five-second
remote-stop gate. Give transport watchdog, local authority fencing and
remote expiry independent execution paths. Hosted HTTPS, real sandbox
placement and production teardown still need an integration run before
replacing the mailbox data path.

## Follow-up — bounded Notes search

The spike reuses the existing [Notes FTS5 and folded-title schema](../../crates/content/src/search/schema.sql), the message index's term splitting and approach to bounded ordered postings, and the [search lane's background worker](../reviews/notes-search-worker-2026-11-18.md). The message index's compressed blocks and message byte locators stay on Threads. Notes searches a scope-local FTS5 corpus and uses the same folded-title B-tree for literal title prefixes. The worker publishes the scoped term dictionary and document frequencies with the projection. Each text result sorts by the stable scope-local order for that generation. The fixture builds that order from UUIDs **after** applying authority. FTS5 supplies the order without a temporary sort or shared relevance score.

The 100,000-note source has 96,774 readable space notes and 9,677 readable notes in a manual collection. Measurements use a fresh SQLite read-only connection, a single snapshot per window, a warm filesystem cache, and 100 repetitions per cell on locked perf-01. Windows have 200 rows. At depths 0, 200 and 2,000, each window traversed 200, 400 and 2,200 matched entries. Full-text windows make 200 additional indexed document-ID lookups. Title-prefix windows are covering and make zero document lookups. No window writes or flushes SQLite. The separate postings column shows the full-scope input bound, not the number of MATCH hits traversed.

| Scope | Query | Postings upper bound | Depth 0 p50 / p95 (ms) | Depth 200 p50 / p95 (ms) | Depth 2,000 p50 / p95 (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| Space | 1 term | 53,226 | 0.444 / 0.516 | 0.450 / 0.581 | 0.517 / 0.530 |
| Space | 2 AND terms | 101,613 | 0.457 / 0.470 | 0.476 / 0.508 | 0.627 / 0.698 |
| Space | 3 AND terms | 149,999 | 0.481 / 0.499 | 0.511 / 0.565 | 0.712 / 0.733 |
| Space | Title prefix | 0 | 0.276 / 0.300 | 0.290 / 0.327 | 0.389 / 0.410 |
| Space | Token prefix | 133,548 | 5.634 / 5.903 | 5.689 / 5.893 | 5.718 / 6.140 |
| Collection | 1 term | 5,323 | 0.432 / 0.454 | 0.439 / 0.460 | 0.509 / 0.529 |
| Collection | 2 AND terms | 10,161 | 0.458 / 0.494 | 0.476 / 0.510 | 0.623 / 0.655 |
| Collection | 3 AND terms | 14,999 | 0.477 / 0.509 | 0.511 / 0.548 | 0.718 / 0.753 |
| Collection | Title prefix | 0 | 0.280 / 0.308 | 0.286 / 0.338 | 0.391 / 0.418 |
| Collection | Token prefix | 13,354 | 0.968 / 1.030 | 0.976 / 1.028 | 1.040 / 1.106 |

Exact count probes were timed separately, again with 100 runs per cell. An exact term reads one scoped dictionary row. Other counts traverse the bounded MATCH or title range.

| Scope | Query | Matching entries | Entries walked for count | Count p50 / p95 (ms) |
| --- | --- | ---: | ---: | ---: |
| Space | 1 term | 53,226 | 0 | 0.211 / 0.231 |
| Space | 2 AND terms | 43,548 | 43,548 | 4.033 / 4.151 |
| Space | 3 AND terms | 33,870 | 33,870 | 4.449 / 4.549 |
| Space | Title prefix | 36,774 | 36,774 | 2.417 / 2.595 |
| Space | Token prefix | 96,774 | 96,774 | 9.589 / 9.742 |
| Collection | 1 term | 5,323 | 0 | 0.213 / 0.237 |
| Collection | 2 AND terms | 4,355 | 4,355 | 0.690 / 0.839 |
| Collection | 3 AND terms | 3,387 | 3,387 | 0.745 / 0.948 |
| Collection | Title prefix | 3,677 | 3,677 | 0.447 / 0.494 |
| Collection | Token prefix | 9,677 | 9,677 | 1.249 / 1.350 |

Default to count mode `none`. Exact counts are available only on count-authorized ready scopes. Keep the existing separate 100,000-work count budget and four count starts per second per space. In this fixture, the tested space-wide two- and three-term counts and the space-wide token-prefix count exceed the conservative input-posting ceiling for counts. The collection variants, single literal term and title-prefix count fit. No estimate was qualified.

### Separate posting budget and refinement

Cap a ready search window at **160,000 scoped input term–document postings**, independent of the 4,096 result/index-row budget. Admit using the sum of published per-term document frequencies. Exact words take up to three point reads, and a token prefix scans at most 33 term-stat rows to check a 32-expansion ceiling. FTS5's live vocabulary cursor computes frequencies by enumerating postings, so the worker persists them at publication time. The estimate is of the available posting work, with merge and prefix overhead captured by the measured query. A settled compacted corpus and the same-generation dictionary are required.

| Warm full-consumption calibration | Input postings or upper bound | Raw p50 / p95 (ms) | Normalized p50 / p95 per 100,000 input postings (ms) |
| --- | ---: | ---: | ---: |
| Dense single term | 96,774 actual | 4.249 / 4.461 | 4.390 / 4.609 |
| Three-term intersection | 149,999 upper bound | 4.069 / 4.290 | 2.712 / 2.860 |
| Two-expansion prefix | 133,548 upper bound | 9.251 / 9.489 | 6.927 / 7.105 |

Each calibration had 100 runs on locked perf-01. Normalization uses posting input size, not measured cursor op counts for intersection and prefix. The slowest tested shape scaled to the 160,000 ceiling is about 11.4 ms p95. The slowest measured fresh window was 6.140 ms p95. The probe did not measure fragmented production index generations.

For broad AND, FTS5 applies an ordered rowid interval constraint over the scoped corpus. The bounded leading interval holds at most `floor(160000 / term_count)` scope-local ordinals. A broad three-word query returned 200 authorized provisional rows at depth 2,000 in 0.748 / 0.782 ms p50 / p95. A broad token prefix avoids the expansion and returns `refine_query` in 0.220 / 0.244 ms, with zero provisional rows in the uncached fixture. At most an already-issued window for that same authorized query, audience and generation may be retained in that case. Both refinements omit the count and carry an explicit state. Each was measured 100 times.

The initial probe qualified one to three literal ASCII AND terms, one ASCII token prefix with at most 32 expansions, and literal folded-title prefix for ready space or manual-collection scopes with up to 100,000 authorized notes. The Notes probe tested 200-row windows through depth 2,000. The shared guarantee below applies to both kinds. Use the contract's `422 unsupported_query_plan` for unqualified shapes, including raw Boolean/phrase grammar, fuzzy or shared-corpus relevance ranking, mixed word/prefix expressions, unqualified tokenization, deeper offsets, foreign projections and exact counts above the count budget when required. Admitted shapes that exceed the separate postings budget use refinement.

**Proposed specification sentence**

> A ready authority-versioned Notes search Bind admits one to three literal AND tokens or one bounded token prefix from the generation’s qualified FTS5 `unicode61` tokenizer, or a literal folded-title prefix, with a separate cap of 160,000 scoped term–document postings per window and the existing 4,096-row cap, and returns a typed `refine_query` window with only bounded authorized results for that query and generation and count mode `none` on posting-budget exhaustion.

The catalogue must add `refine_query` to the result states and allow authorized provisional rows for that state. The ordinary `loading` state already accepts an empty row array, null result version, and absent count and activation contexts.

### Save cost, storage and publication

The scoped index occupied **68,595,712 bytes** for the space projection and **8,568,832 bytes** for the manual collection, including authorized FTS text, title indexes and term dictionary. Those projections contained 11,177,397 and 1,117,695 bytes of synthetic note bodies, respectively. Size scales with the number and size of physically distinct scoped corpora. Share a projection only across equivalent authority sets.

An 8 KiB save with 32 effective scopes took **22.028 / 30.685 ms p50 / p95** with covering membership and counts alone. Maintaining the FTS and folded-title copies in the same content transaction took **45.170 / 66.775 ms**. The repeat measured 29.538 ms and 67.882 ms p95, respectively. Each arm used 100 real content saves and the original 40 ms gate. Steady-state synchronous maintenance replaces 32 document and FTS rows and runs the corresponding title-index updates. There is no new foreground commit, but the existing FULL transaction covers the added WAL work. The synchronous variant already missed before adding term-dictionary maintenance. Keep content admission and save acknowledgment independent of search maintenance through the existing background journal worker.

Save-to-search starts when the real content save acknowledges. The target's existing search worker begins with a ready 100,000-note index. The test waits on its real journal-backed search query, then a disposable adapter publishes the changed row, term frequencies and generation into a full-size space FTS corpus and manual-collection corpus. Those two background commits use SQLite NORMAL and do not block save acknowledgment. Observation uses a two-millisecond polling interval, plus scheduling delay. Each successful arm uses 100 acknowledged saves and checks that the new body token becomes searchable. The busy arm has six competing space writers, each finishing file placement before submitting the next save.

| Load on the existing worker | Competing saves | Existing worker p50 / p95 (ms) | Ready in both scoped fixture corpora p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Idle | 0 | 16.016 / 24.042 | 17.392 / 26.468 |
| Busy | 516 | 16.421 / 34.967 | 18.100 / 54.222 |

An initial unpaced busy attempt hit the content engine's one-second pending-file placement backpressure and ended before a busy percentile could be collected. The table's busy result describes the placement-paced workload. It does not establish publication lag for 32 physical scope copies or a cold rebuild.

Before evaluating a Bind, resolve the currently authorized audience, corpus and searchable fields. Compare the projection's authority revision and content watermark with the live source. While the authorized projection is behind, return `loading` with empty `rows`, null `result_version`, and no count or activation contexts so the renderer can show that search is updating. A lost scope returns the ordinary denial. Once a revoke commits, fence the old grant version immediately, discard cached rows and counts for it, and recheck that version again at delivery to drop racing windows. Scope-local statistics cannot be reused after revocation. Never filter a stale ranked window after it was evaluated as a substitute. In the fixture, each of 200 prepublication attempts returned `loading` and each of 200 stale-authority delivery attempts returned zero rows or counts. Rebuilding after an excluded row changed title and body left every sampled rank and count unchanged in both scopes. A private-only term matched zero authorized notes.

The targeted release test `recovery::tests::maintenance_and_retry_serialize_with_retirement` also fails on `origin/dev` at `e51899fa`. It times out waiting for `AfterJournalCommit`, which is compiled only with debug assertions.

Reproduce the follow-up with the page executable below and the test name
filter `scoped_search`. The tests can be run separately with
`--exact scoped_search_windows`, `--exact scoped_search_combined_save`, or
`--exact scoped_search_worker_lag`. The lag fixture defaults to 100 samples.

### Unicode qualification

The existing Notes FTS5 index uses `unicode61 remove_diacritics 2` and prefix indexes at lengths 2 and 3. This probe compiled each settled literal using that SQLite tokenizer. It read native tokens from a scratch FTS5 vocabulary instance, then used the scoped generation's matching term dictionary to enforce the 160,000-posting ceiling. Settled-query timings include opening a fresh SQLite read connection, checking the posting budget and materializing up to 200 IDs. Tokenizer setup is outside that interval. They used the same 100,000-note mixed-visibility source, with 96,774 authorized notes across the space and 9,677 in the manual collection. Every returned window matched independently selected authorized source IDs in order.

- **Accented Latin qualifies as-is.** `resume` matched `RÉSUMÉ` and the combining-acute spelling. `RESU*` matched `résumé`. The separate folded-title B-tree lowers case and preserves accents, so a literal title prefix `resu` misses `résumé`. The title-prefix arm uses `RÉSU`.
- **Cyrillic qualifies as-is.** `ёлка` matched `ЁЛКА`. `елка` remained distinct. The three-term probe used `ёлка москва библиотека`.
- **Devanagari full text needs the tokenizer option `categories 'L* N* Co M*'`.** By default, `किताब` yielded `क`, `त`, `ब` and matched `कुताब` in a controlled false-positive check. The three-term space arm exceeded the cap. With marks included, `किताब`, `दिल्ली` and `भारत` each produced one token. The false match disappeared and the prefix `कि` kept its vowel sign. The mark-aware tokenizer also passed the Latin and Cyrillic correctness checks. Qualify Devanagari full text only after rebuilding the scoped corpus and its term frequencies with matching tokenizer options. Literal folded-title prefix already preserves these marks.
- **CJK qualifies for literal token boundaries, whole runs and leading prefixes.** The space-delimited Han control matched all three words. With no separators, `资料会议记录`, `資料かいぎ記録` and `자료회의기록` each produced a single token. Whole-run exact queries and prefixes at the beginning matched. Three interior words supplied separately matched zero such rows. Implicit CJK word segmentation and interior substring matching are `unsupported_query_plan` in the first contract. They require a separately qualified segmentation plan.

Each cell below has 100 repetitions on locked perf-01. The three p95 values are for depths 0 / 200 / 2,000. Posting estimates are per scope before MATCH. Folded-title prefixes use the covering B-tree and consume zero FTS postings. Default Devanagari text rows are diagnostics, including admission-only time for the refused space AND arm.

| Corpus | Query | Postings, space / collection | Space p95 (ms) | Collection p95 (ms) |
| --- | --- | ---: | ---: | ---: |
| Accented Latin | Three terms | 149,999 / 14,999 | 0.389 / 0.375 / 0.673 | 0.344 / 0.372 / 0.593 |
| Accented Latin | Token prefix | 53,226 / 5,323 | 1.825 / 1.780 / 1.845 | 0.442 / 0.451 / 0.569 |
| Accented Latin | Title prefix | 0 / 0 | 0.174 / 0.181 / 0.275 | 0.173 / 0.177 / 0.269 |
| Cyrillic | Three terms | 149,999 / 14,999 | 0.349 / 0.369 / 0.588 | 0.362 / 0.376 / 0.773 |
| Cyrillic | Token prefix | 53,226 / 5,323 | 0.319 / 0.316 / 0.509 | 0.316 / 0.324 / 0.414 |
| Cyrillic | Title prefix | 0 / 0 | 0.186 / 0.179 / 0.278 | 0.197 / 0.192 / 0.284 |
| Devanagari, default | Three terms | 401,611 / 40,159 | 0.133 / 0.143 / 0.106 (refusal) | 0.481 / 0.557 / 1.443 |
| Devanagari, default | Token prefix | 53,226 / 5,323 | 1.708 / 1.655 / 1.756 | 0.455 / 0.465 / 0.545 |
| Devanagari, default | Title prefix | 0 / 0 | 0.182 / 0.180 / 0.281 | 0.172 / 0.175 / 0.287 |
| Devanagari, marks | Three terms | 149,999 / 14,999 | 0.365 / 0.423 / 0.630 | 0.396 / 0.414 / 0.609 |
| Devanagari, marks | Token prefix | 53,226 / 5,323 | 0.318 / 0.419 / 0.500 | 0.416 / 0.327 / 0.507 |
| Devanagari, marks | Title prefix | 0 / 0 | 0.176 / 0.197 / 0.297 | 0.190 / 0.182 / 0.292 |
| Han, spaced | Three terms | 149,999 / 14,999 | 0.392 / 0.402 / 0.604 | 0.352 / 0.369 / 0.598 |
| Han, spaced | Token prefix | 53,226 / 5,323 | 0.330 / 0.318 / 0.407 | 0.311 / 0.318 / 0.394 |
| Han, spaced | Title prefix | 0 / 0 | 0.211 / 0.183 / 0.279 | 0.172 / 0.177 / 0.271 |
| Han, unspaced | Three terms | 36,774 / 3,677 | 0.208 / 0.202 / 0.186 (zero hits) | 0.192 / 0.199 / 0.217 |
| Han, unspaced | Token prefix | 70,644 / 7,064 | 0.375 / 0.338 / 0.415 | 0.337 / 0.332 / 0.399 |
| Han, unspaced | Title prefix | 0 / 0 | 0.183 / 0.188 / 0.280 | 0.168 / 0.195 / 0.288 |
| Han, unspaced | Whole run | 33,870 / 3,387 | 0.324 / 0.335 / 0.428 | 0.336 / 0.325 / 0.418 |
| Japanese, unspaced | Three terms | 36,774 / 3,677 | 0.278 / 0.248 / 0.190 (zero hits) | 0.199 / 0.181 / 0.224 |
| Japanese, unspaced | Token prefix | 70,644 / 7,064 | 0.322 / 0.325 / 0.414 | 0.346 / 0.341 / 0.426 |
| Japanese, unspaced | Title prefix | 0 / 0 | 0.203 / 0.196 / 0.278 | 0.170 / 0.191 / 0.268 |
| Japanese, unspaced | Whole run | 33,870 / 3,387 | 0.310 / 0.318 / 0.390 | 0.385 / 0.332 / 0.400 |
| Hangul, unspaced | Three terms | 36,774 / 3,677 | 0.218 / 0.197 / 0.224 (zero hits) | 0.193 / 0.187 / 0.184 |
| Hangul, unspaced | Token prefix | 70,644 / 7,064 | 0.346 / 0.363 / 0.416 | 0.338 / 0.341 / 0.393 |
| Hangul, unspaced | Title prefix | 0 / 0 | 0.186 / 0.197 / 0.296 | 0.167 / 0.183 / 0.280 |
| Hangul, unspaced | Whole run | 33,870 / 3,387 | 0.355 / 0.340 / 0.417 | 0.314 / 0.318 / 0.403 |

Reproduce this matrix with `--exact scoped_search_international` on the page executable under the same perf lock.


## Follow-up — Threads title and message search

The Threads probe reuses the [content message schema and Elias–Fano codec](../../crates/content/src/search/messages/), the `calternal-store` projection journal, and the folded-title B-tree in `calternal-content`. The existing [search lane](../reviews/search-2026-11-18.md) built the Notes and Threads discovery routes. Its [message postings follow-up](../reviews/notes-search-worker-2026-11-18.md) qualified compressed searchable message blocks. The production [server thread adapter](../../crates/server/src/threads/search.rs) validates saved frames and supplies exact byte ranges to the generic index. Here the append fixture supplies its own byte-framed `MessageDecoder` and uses the real content writer and worker. The scoped read harness reuses the schema and codec with a disposable authority projection, budgeted distinct-thread reduction and prefix expansion. Those additions are outside the existing product message API.

The synthetic corpus has 100,000 thread titles and 740,000 messages, with 2, 8, 16 or 32 messages per thread and a mix of review, planning and follow-up text. Every eleventh thread is hidden before indexing. The ready scope includes 90,909 titles and 672,724 messages, with 94,186,301 source message-body bytes tokenized. The scope's message and title index file occupied 93,519,872 bytes. The body bytes are input to the tokenizer. The read fixture returns thread IDs and uses location and rank fields without materializing an excerpt or opening the source revision.

A message term uses the current `calternal-content::message_terms` tokenizer, which splits non-alphanumeric characters and applies Unicode lowercase. Use one to three literal AND terms in the **same message**, or one dictionary prefix with at most 32 expansions. The probe intersects or merges the existing compressed blocks. It ranks each distinct thread by its newest matching authorized message, using saved timestamp and message UUID as the key. Equal timestamps were present in the fixture. `title_prefix` uses the independent folded title and UUID order. There is no shared relevance score.

Each cell uses 100 fresh connection probes on locked perf-01, a 200-row window, and a ready generation. The posting bound is calculated from scoped document frequencies before decoding. Row work is location/returned-ID lookups, block bodies visited and driver descriptors read. The small dictionary preflight is separately bounded by three literal terms or 33 prefix dictionary entries. The folded-title prefix row work is its covering walk. Read probes make zero commits or flushes.

| Query | Depth | Posting bound | Postings decoded | Row work | Returned rows / state | p50 / p95 (ms) |
| --- | ---: | ---: | ---: | ---: | --- | ---: |
| One term | 0 | 84,097 | 2,161 | 991 | 200 / ready | 0.771 / 0.810 |
| One term | 200 | 84,097 | 2,161 | 1,740 | 200 / ready | 1.233 / 1.413 |
| One term | 2000 | 84,097 | 4,214 | 3,896 | 0 / refine | 2.503 / 2.932 |
| Two AND terms | 0 | 137,918 | 3,542 | 831 | 200 / ready | 0.727 / 0.808 |
| Two AND terms | 200 | 137,918 | 3,542 | 1,420 | 200 / ready | 1.076 / 1.120 |
| Two AND terms | 2000 | 137,918 | 10,289 | 3,896 | 0 / refine | 2.685 / 2.828 |
| Dense three AND terms | 0 | 170,192 | 0 | 0 | 0 / refine | 0.143 / 0.165 |
| Dense three AND terms | 200 | 170,192 | 0 | 0 | 0 / refine | 0.142 / 0.164 |
| Dense three AND terms | 2000 | 170,192 | 0 | 0 | 0 / refine | 0.142 / 0.164 |
| Selective three AND terms | 0 | 154,098 | 3,956 | 486 | 200 / ready | 0.541 / 0.570 |
| Selective three AND terms | 200 | 154,098 | 7,725 | 733 | 200 / ready | 0.774 / 0.807 |
| Selective three AND terms | 2000 | 154,098 | 26,492 | 2,938 | 200 / ready | 2.658 / 2.794 |
| Message prefix | 0 | 137,918 | 3,542 | 1,033 | 200 / ready | 0.834 / 0.958 |
| Message prefix | 200 | 137,918 | 3,542 | 1,782 | 200 / ready | 1.280 / 1.414 |
| Message prefix | 2000 | 137,918 | 6,912 | 3,896 | 0 / refine | 2.597 / 2.692 |
| Title prefix | 0 | 0 | 0 | 200 | 200 / ready | 0.194 / 0.213 |
| Title prefix | 200 | 0 | 0 | 400 | 200 / ready | 0.211 / 0.255 |
| Title prefix | 2000 | 0 | 0 | 2,200 | 200 / ready | 0.286 / 0.305 |
| Broad single term | 0 | 672,724 | 0 | 0 | 0 / refine | 0.140 / 0.165 |
| Broad single term | 200 | 672,724 | 0 | 0 | 0 / refine | 0.149 / 0.179 |
| Broad single term | 2000 | 672,724 | 0 | 0 | 0 / refine | 0.147 / 0.177 |

### Shared depth guarantee

Every admitted shape measured for Notes and Threads completed the depth-200 window. The broad and otherwise unqualified diagnostics intentionally exercise refusal or refinement. Notes' worst p95 was **5.893 ms** (space token prefix), its highest posting bound was **149,999**, and its highest window row work was **600** (400 matching steps plus 200 ID lookups). The international Notes arms also fit under those maxima once their tokenizer is qualified. Threads' worst p95 was **1.414 ms** (message prefix), highest posting bound **154,098** and highest actual decode **7,725 postings** (both selective three terms). Its highest window-stage row work was **1,782** (1,696 location/ID lookups, two blocks and 84 driver descriptors). The worst cases remain below both budgets.

At depth 2,000, one/two terms and the tested common message prefix hit the 4,096-row ceiling before collecting enough distinct threads. The cap reserves output lookups as well as candidate work. Repeated matches in a long thread still cost a location lookup before collapsing to one result. A selective three-term query and the title prefix completed that depth. The 170,192-posting three-term input and 672,724-posting broad input refined before decode.

A per-thread best-match projection could remove repeated location work for an already-settled query. A per-term newest-matching-message projection can also implement one-term search and a prefix union. AND needs evidence that the terms matched the **same message**. Combining different per-term winners from one thread would change the semantics. A later AND projection must keep positional intersection or cache the query's joined winners. Keep this work out of the first contract.

Qualify Threads `title_prefix`, one to three literal message `terms`, and one bounded message `prefix` on ready, authorized data. Use `refine_query` with count mode `none` on posting or row exhaustion. Combined title/message relevance, mixed exact/prefix input, arbitrary Boolean, phrase, fuzzy plans and exact distinct-thread counts lack this qualification. Refuse those first-contract requests with `unsupported_query_plan`. The existing product phrase option remains a separate search route.

**Proposed specification sentence for both kinds**

> Every ready authority-versioned Notes or Threads search Bind qualifies its supported literal-term and prefix shapes for 200-row windows through depth 200, serves later windows while the 160,000 scoped-posting and 4,096-row budgets both hold, and returns a typed `refine_query` window with only bounded authorized results for that query and generation and count mode `none` on either budget's exhaustion.

### Append publication and hidden changes

The lag probe primes the existing message index at the real journal watermark with 672,724 searchable messages. It appends to a real committed thread via `Content::write` and waits on the existing search worker's message query. Once the appended marker is searchable there, the fixture publishes it to a same-sized scoped copy with a NORMAL SQLite commit and checks the published term. The asynchronous scope step includes the dictionary and visible version update. The prototype copies only the new message to the scoped projection for this append-only workload. The production worker replaces the changed thread's posting blocks and watermark on journal replay. Cold rebuild, server HTTP, excerpt delivery, and repaint are outside these timings.

| Load | Competing appends | Existing worker p50 / p95 (ms) | Scope searchable p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Idle | 0 | 35.892 / 46.281 | 36.187 / 46.847 |
| Busy, six placement-paced writers | 1,916 | 327.627 / 467.882 | 338.112 / 468.095 |

Each arm uses 100 append acknowledgments and 2 ms observing polls on perf-01 under the lock. An earlier locked run recorded scoped p95 of 46.816 ms idle and 522.076 ms busy, with 1,998 competing appends. A visible append leaves the previous scope revision in place until publication. The Bind uses the authorized `loading` state during that gap, with no rows or count.

Rebuilding the reader's scope after an excluded-source change kept every sampled rank, state and work count unchanged. A second probe appended to an existing hidden thread in the live space. The real upstream message worker published the private marker, and the reader's scope retained the same state, rank and term bound. It acquired zero private terms. A scoped version advances only on changes visible to that audience. The upstream space-wide watermark and coverage flag cannot become the reader-facing result state. Scope authority must still be checked before evaluation and delivery.

Reproduce with the page executable and filter `scoped_threads` under the
same lock below. The lag test defaults to 100 samples per arm.

## Nested rows and personal sections

The harness uses the sealed revision 12.5 base `bb43b24e`. It opens
disposable roots through the server's `Store`, uses real `notes.create`,
`threads.create`, `threads.list`, `sharing.grant` and `sharing.revoke`
operations, and reuses `authorize_item_access`, the item access epoch and
the live directory membership view. Turns supplies its own parent
relationships and cursor-ordered child IDs. The fixture declares the
Notes relationships and writes the disposable scoped projection from
authorized real item IDs.

The parent key is (viewer, space, parent, child Bind, sort key, ID).
A parent reference is random and bound to the view, viewer, Bind, space
and complete path. An admitted depth-one Turns window checks one live
item head and one live authority query, reads one exact count and seeks
one ordered window. A Notes ancestor also reads inherited collection
roots. Depth three repeats the ancestor check for each ancestor. The
first and second windows in the fixture visit two and three index
entries, respectively, and return two and one rows. The read paths run
zero commits or flushes and require no worker. The shared process-local
access epoch detects stale projection delivery in the fixture. Product
maintenance needs audience-scoped invalidation and a delivery fence.

The layout prototype uses `rusqlite` on the disposable authority
database, so its live-membership check and effect/Undo share one write
transaction. The backing space uses real Notes items. The fixture's
item lookup is a seeded projection for that shared-space membership
case. The transaction takes one membership and one revision read.
Create takes two more indexed reads, rename one, remove two including
the removed section's placements. Move to takes four more reads for
destination, item, prior placement and destination tail. Move up and
down each take two more reads for placement and neighbor. Undo adds one
receipt read to membership and revision. Each success uses one FULL
WAL commit and its sync. There are no independent effect/Undo commits.
Opening the temporary authority database for prototype transactions
serializes unrelated writers in that database. Product layout should
use the per-space writer with a live authority commit fence.

Both runs used `calternal-perf-01-backup01` under
`/usr/local/bin/perf-lock.sh` and a release binary built on dev-01.
Each timed cell has 100 repetitions per width. All latency cells are
p50 / p95 in ms for 256 and 4,096 placements, respectively.

| Injection | Pass condition | Measured | Verdict |
| --- | --- | --- | --- |
| Interleave offsets 0 and 2 under two Turns parents in one Bind | Independent exact windows in the real Turns order | 0.961 / 1.064 and 0.933 / 1.226 per four windows. All 800 timed windows matched. | Pass |
| Nested Notes and Turns, hidden Notes child, invalid refs | Authorized ancestry through depth three. Depth four, cycles, wrong view/Bind/viewer and forged refs confer no rows or counts. | 200 iterations with four correct depth-two/three windows and four denied refs each. Hidden child excluded from both rows and the exact count on all 200 Notes page pairs. Depth four and cycle refused in both runs. | Pass |
| Revoke a child grant, parent grant and Turns membership | No stale rows or counts. Refresh reflects the child revoke. | Two child revokes. Both stale projections suppressed and both refreshed counts fell from 3 to 2. Zero rows and counts on 1,400 revoked root/descendant reads. | Pass |
| Stock Turns parent query versus scoped index | Ordered seek under the parent constraint, with bounded visits | Stock query plan uses `item_list (kind=? AND updated_at<?)` with a residual parent predicate. Scoped plan uses `PRIMARY KEY (actor=? AND space=? AND parent=? AND bind=?)`, with no sort. | Existing plan misses, prototype passes |
| `host.layout.section.create` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.704 / 8.222 and 6.690 / 10.489. Undo 6.712 / 9.221 and 6.679 / 9.430. | Pass |
| `host.layout.section.rename` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.607 / 6.918 and 6.563 / 7.099. Undo 6.588 / 9.803 and 6.545 / 8.750. | Pass |
| `host.layout.section.remove` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.984 / 11.348 and 19.044 / 30.974. Undo 8.522 / 14.728 and 45.540 / 49.464. | 40 ms gate misses |
| `host.layout.move_to` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.476 / 8.953 and 6.712 / 10.339. Undo 6.459 / 9.121 and 6.632 / 9.926. | Pass |
| `host.layout.move_up` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.497 / 8.541 and 6.642 / 10.297. Undo 6.478 / 8.482 and 6.612 / 9.993. | Pass |
| `host.layout.move_down` | Effect and Undo atomic, each p95 ≤40 ms | Edit 6.931 / 8.769 and 6.702 / 8.083. Undo 6.900 / 8.778 and 6.666 / 7.777. | Pass |
| Lost reply, process exit and stale or foreign Undo | Neither effect nor Undo before commit. Both survive after commit and during reply publication. Restore exact placement/order. A newer layout edit prevents prior Undo. | 36 process exits across the six operations and two widths. 1,200 clean pre-commit rollbacks and 1,200 successful effect/Undo pairs. All 200 stale and 1,200 foreign Undo attempts refused. | Pass |
| Personal section windows and live membership | Scope to the authenticated person’s layout. Refuse item-only access, foreign/forged parent ref, and revoked section/placement reads, writes and Undo. | Second member unchanged on all 1,200 owner edits. Zero admitted revoked reads, edits or Undo across 200 membership removals. Each width also refused forged/foreign section refs and an actual item from a different space. | Pass |

### What the specification must change

The sealed text already requires a parent-scoped ordered index. The
stock Turns query cannot implement this contract at scale. For the
implementation, maintain the current authorized parent keys and their
counts at the same scope as the read. Keep unauthorized or stale counts
and rows unavailable until the replacement projection is ready.

Specify bounded work at the layout commit boundary. A section removal
should retire the section and its placement generation atomically and
retain that generation until its Undo expires. Undo should reactivate it
in one transaction. Copying or rewriting every placement at the
interactive boundary makes the operation depend on section size.

Build via `nix develop -c cargo test --locked --release -p calternal-server
--test v2_step0_nested --no-run` on dev-01. Copy the release test executable
and the Notes, orchestrator and Claude Code manifests to a disposable
ext4 directory on perf-01. Set `STEP0_PLUGINS` to the copied plugin root
and `TMPDIR` to the disposable directory. Run the executable under the
perf lock with `--ignored --exact parent_windows_and_personal_sections
--nocapture --test-threads=1`. The second arm sets `STEP0_LAYOUT_SIZE=4096`.

### Follow-up — retained section generations

The locked perf-01 run used the prototype's per-person SQLite transaction
with the actual directory membership view. The three widths use
synthetic UUID-shaped placement projections. The earlier real-engine
fixture supplies the two copy-based observations above. Each timed
foreground cell has 100 repetitions. All latencies are p50 / p95 in ms.

| Injection | Pass condition | Measured | Verdict |
| --- | --- | --- | --- |
| Remove and Undo 256 placements | Each p95 ≤40 ms, no placement writes | Remove 6.761 / 8.142. Undo 6.741 / 8.196. | Pass |
| Remove and Undo 4,096 placements | Same commit work as 256 | Remove 6.728 / 8.186. Undo 6.683 / 9.730. | Pass |
| Remove and Undo 100,000 placements | Same commit work as 256 and exact retained generation | Remove 6.806 / 8.512. Undo 6.795 / 8.050. | Pass |
| Foreground key touches and flushes | A single FULL transaction per effect or Undo, no size-dependent writes | Exactly three table rows change each time. The five logical B-tree keys include the layout watermark, section row and active-order entry, and Undo row and expiry entry. Zero placement keys change. At each width both paths made one actual sync call in 99/100 runs. The remaining removal made two sync calls and the remaining Undo made three, with WAL setup/checkpoint work included. | Pass |
| 200-row live section read before and during retention | No retired rows. Live windows keep the same order and visit bound. | Before → during p95 was 0.097 → 0.130, 0.087 → 0.113, and 0.106 → 0.128 at the three widths. Retired reads refused at p95 0.054, 0.041, and 0.051. | Pass |
| Reclaim after the Undo deadline | No reclaim before expiry or successful expired Undo. Bounded deletes, no reclaim on foreground commit. | At most 256 placement keys per maintenance step. Reclaimed all 256 / 4,096 / 100,000 in 2 / 17 / 391 successful steps, p95 7.224 / 7.336 / 10.352 ms per step. | Pass |

Removal reads membership and the active section, then bumps and obtains
the layout revision in the write statement. Undo reads membership,
Undo and the revision it pins. Each writes one FULL WAL transaction.
A live section page checks membership, reads the active generation and
seeks its first 200 placement keys. A retired section read stops at the
active-section check and visits zero placement keys. Reads commit and
flush zero times. The expired-Undo index selects one generation per
maintenance transaction. Each batch deletes the next at most 256 keys.
The last batch also discards the section marker and expired Undo. An
exact multiple of 256 uses a separate final metadata transaction. This
spike uses a 60-second fixture clock. The product retention duration
follows the host's Undo policy.

In the §4.4 remove row and the catalogue's `atomic_undo_when`, require
retirement and Undo to name one retained placement generation. Exclude
retired generations from reads and reactivate the exact generation
atomically after checking current membership and the post-effect layout
revision. Reactivation grants no item access. Reclaim only after Undo
expiry in bounded background batches. Removal and Undo perform zero
per-placement work.

The benchmark uses the same release test executable and the
`bounded::retained_layout_generation` filter. Build the Linux
sync-call probe on dev-01 with
`nix develop -c cc -O2 -shared -fPIC spikes/plugin-api-v2/layout_sync_probe.c -o target/release/libstep0_layout_sync.so -ldl`.
Copy it to perf-01 and set `LD_PRELOAD` to its absolute path inside the
perf lock. The probe counts `fsync` and `fdatasync` calls separately from
the timing statistic.

## Reproducing the measurements

Build in the existing lane copy on `calternal-dev-01-compute02`.

```sh
nix develop -c cargo test --locked --release -p calternal-content \
  --test v2_step0_page --test v2_step0_save --no-run
nix develop -c cargo test --locked --release -p calternal-server \
  --test v2_step0_carrier --no-run
```

Copy the three executable paths printed by Cargo, plus
`crates/server/tests/v2_step0_carrier/package.cjs`, to a disposable ext4
directory on perf-01. Set `TMPDIR` to that directory. Run the executables
through `/usr/local/bin/perf-lock.sh` with
`--ignored --nocapture --test-threads=1`. Set `STEP0_PACKAGE` to the
absolute path of the copied script for the carrier. The carrier uses 100
repetitions by default. The short development smoke can set `STEP0_RUNS=3`.

To reproduce the separate capped-width arm, set `STEP0_SCOPE=32` and run
the page executable with `--exact covering_combined_save` in addition
to the flags above. The other widths can be selected the same way.
The staged-save executable launches itself for the process-exit cases,
so retain its path until the parent exits.

`cargo fmt --all --check`, the content debug suite and targeted release
fixture tests ran on dev-01. The broad release content suite ran into a
pre-existing recovery test that waits for the debug-only
`AfterJournalCommit` hook. That test passed in the debug suite.

## Simplification opportunities

- Retain retired placement generations under the Undo lifetime and reclaim them in background batches.

- Use the same scoped count for empty, end and arbitrary-jump decisions.
- Keep a single item receipt namespace and use the committed outbox intent as
  the recovery record for a sealed upload.
- Share strict NDJSON validation between the child and enrolled adapters when
  their frame contracts converge.
