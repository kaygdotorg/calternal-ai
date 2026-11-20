# Window query engine, 2026-11-19

Issue [#85](https://git.kayg.org/kayg/calternal-ai/issues/85) implements
the first ordered content projections. Unfiltered `notes.list` queries live
heads and keeps its signed timestamp/UUID cursor and summary shape. The API also qualifies live explicit Notes collections ordered
by their exposed title, using SQLite’s ASCII lowercasing and UUID as the tie break. Other sources, sort/filter
shapes and parent windows require their own qualified plan.

The product publishes window indexes from the durable journal. The owner
approved that design on 2026-11-20. The same-transaction implementations
appear only in the historical measurements below.

## Storage and authority

- The native `notes.list` route queries live heads. The v2 window engine
  reuses the source item's primary key, the reverse Notes membership index,
  the outbox, the durable journal and the existing projection worker.
  Source schema 9 adds definition data and enforces admission of new
  memberships. Background replay writes row keys and counts together in a
  separate private WAL database.
- A live note can have its space projection and up to 31 explicit
  collection projections. A new edge that would exceed the cap aborts the
  authoritative transaction with HTTP 422 `effective_scope_limit_exceeded`.
  Inherited over-limit notes remain editable and keep their Notes or
  Trash row. Replay omits the affected explicit-collection rows and
  records the item until its memberships are repaired.
- Guest rows expose titles and view-scoped selectors. Hidden metadata and
  body edits leave the guest scope version unchanged. Grant, revoke and
  membership authority are checked before storage access and before
  response delivery. Public and guest windows are reachable from no HTTP
  route in this merge.
- Public-link records have no count-disclosure field. The future public
  host should select mode `none` for those grants. The engine accepts
  estimates certified by a trusted host for the current result version.
  End-relative windows require exact count permission. Deep count-free
  starts require the preceding ready boundary from the same view.

## Integration and proposed revisions

- Native Notes exercises the product planner and the read-only v2 Notes route
  calls `Content::window`. The installed-view integration for #81 and #82
  resolves plugin Binds, provisions approved order definitions and issues
  session-bound action contexts. There are no persisted member-cache records
  in this lane.
- **Proposal for effective audience equivalence.** Specify that scopes
  count distinct physical `(collection, projection)` row sets and that grants
  with identical row and field authority reuse an existing projection.
  Directory auth commits remain the source of live grant authority. For
  example, sharing a list with two groups that can both see every title
  should authorize two grants against one projection. Per-recipient variants
  would add fan-out and require coordination between the directory and
  per-space content databases. Keep identical grants on one projection and
  qualify a separate design before exposing any audience-specific row set.
- **Proposal for the encoded byte limit.** The engine uses
  `materialization_limit` for a window whose encoded reply exceeds the
  catalogue limit. For example, 200 long multibyte titles can exceed 128 KiB
  even though the row count is legal. Returning a short successful page would
  falsely signal the end. Keep the typed refusal and explicitly document its
  use when a full reply exceeds the byte cap.

## Registered kinds and parent windows

Source schema 9 uses existing kind registrations. Claimed-text kinds
default to newest. The host can provision one scalar sort and title field
from a checked kind declaration, such as `reading-reminder.due`. Both
directions use the same covering index with UUID as the stable tie break.
A changed definition invalidates publication and starts a bounded scan
of the space. Notes Trash orders by move time. An explicit collection
can publish newest order for members or title order for guests. Requests
for an unprovisioned order return a typed plan refusal.

A registered parent field adds a covering key under the parent UUID and
child kind. The host supplies a checked UUID and edge token after
validating the view and ancestry. The worker changes the two scopes'
rows and counts in the same sidecar step for a child move. A nested
window also checks the parent against its live source head.

- **Proposal for simultaneous sort orders.** One physical ordering per scope
  preserves the save bound of two covering keys per changed scope. For example,
  a Notes collection can serve title order to guests or newest order to
  members. Publishing both would create two ordered keys over the same visible
  row set and would need a clarified cap and a locked combined-save proof.
  Keep the existing scope bound and refuse unpublished alternatives until
  the contract accounts for both costs. Two-field composite plans also
  require qualification.
- **Proposal for declared parent storage.** A view declares the edge but the
  sealed manifest does not identify a persisted relation for an arbitrary
  kind. The host can use an existing canonical relation such as
  `threads.parent_thread`, or register a verified relation from its own
  loader. For example, a reading reminder can only appear under a reading
  item after the host knows which stored UUID field establishes that link.
  Guessing the relation from a field name risks widening authority. Keep
  explicit registration and add a manifest declaration if arbitrary kinds
  need portable edges. Per-person section placements also need the
  host-owned layout store.

Standalone count scheduling, layout placement windows, generic manual
membership and search are outside this slice. The development-only Reading
List adapter binds its pinned title and read mark fields.

## Product v2 route

The authenticated read-only Notes view is available at
`GET /api/v2/spaces/{space}/notes-view`. Its response names a host-owned
`notes` Bind with title projection and a signed view reference. The optional
`count=none` selects the count-free variant. A sealed `ViewOpenRequest` at
the same URL opens a 15-second lease. Submit the sealed `WindowRequest` to
`/window` below that URL with `X-Calternal-View-Lease` and
`X-Calternal-Cache-Partition`. The success body is the sealed result. Plan
refusals use HTTP 422 with the sealed reason body. `/renew` rotates the lease
without retaining renderer rows. Internal continuation keys remain usable
only at a matching current result boundary.

The route binds the opaque lease and partition to the authenticated actor,
device session, space, view and current authority epoch. It checks membership
and session on the blocking request worker before indexed work and again
before returning any reply. It fences changes just before response release.
A current exact count is disclosed only inside this authorized space view.
Count-free views omit the count and refuse End. The content engine also
accepts an audience-certified estimate when a future trusted Bind supplies
one. Route admission uses actor shards, independent per-space quotas, one
in-flight window per view and the catalogue's rate and lease limits. There is no
SQLite write or fsync for open, window or renew. The product publication
table below accounts for window storage reads. The route adds two protected membership reads and two
session-current checks for installation sessions, plus the existing HTTP
guard's credential checks. Lease open classifies a personal space with a
directory point read. The existing directory list runs through its write
queue and advances the fence even for a read. This route uses the read
queue for classification to keep active leases valid.

The Step 0 harness issued fixture references, held test rows and included
fault hooks. A product route needs the installed description and its kind
declarations checked against the manifest, live source and field permissions,
and a registry that issues references for each checked Bind and parent edge.
The private Notes view supplies a fixed server-owned Bind. The generic
installed-view adapter still needs to resolve policy and input operands,
authorize complete parent ancestry, issue action contexts and enable
guest/public links under their existing grant checks. Live invalidation,
retained renderer rows and action contexts need the GenUI view service
used by the #81 and #82 renderers. No public guest or generic plugin route
accepts unvalidated descriptions.

## Reading List product-path fixture

The SDK example at `f038a7a8` supplies the checked-in emitted manifest and
description under `crates/server/tests/fixtures/v2-reading-list`. The
ordinary installer calls the strict `ManifestV1` decoder before it can
persist the archive. It rejects the first kind's v2 `icon` field. The
v1 model also lacks the v2 indexed fields and app descriptor, and its
validator requires `api_version: 1`. The slice uses an explicit development
fixture until an installer can validate and persist a v2 package with
its pinned description. It verifies the emitted template digest and
registers the manifest's claimed-text kind, approved field projection,
and single ordered Bind. Workers, writes, reminders and search are outside
the read-only route.

`GET /api/v2/spaces/{space}/reading-list-view` returns the signed
`view_ref`, `bind: "items"`, `fields: ["title","added","read"]`,
the count mode, and the emitted description. POST the sealed
`ViewOpenRequest` to the same route. POST `WindowRequest` to its
`/window` route with `X-Calternal-View-Lease` and
`X-Calternal-Cache-Partition`. The `/renew` route uses the same lease
envelope and policy as Notes. The member list returns `added DESC` over
canonical UTC timestamps, an exact count by default, and the declared
boolean `read` field for the mark. The author-approved field mapping
qualifies the sort and selects fields at materialization. The slice
qualifies no read-field predicate. Unknown Binds receive
`422 unsupported_query_plan`. The space membership and session checks run
before index access and again before delivery. A different member space and
an item-only grant confer no list access. The protected grant schema only
admits Notes and collections, so the HTTP suite proves the latter with a
real Note grant in the same target space. Generalizing named grants to
installed kinds needs the production kind-aware authority loader.

On dev-01, build the fixture-enabled server under the shared lock and run
it against fresh disposable roots. The benchmark's lock is separate.

```sh
flock -s /tmp/calternal-dev01.lock nix develop -c \
  cargo build --locked -p calternal-server --features test-plugins
fixture_root=$(mktemp -d)
CALTERNAL_TEST_READING_LIST=1 "${CARGO_TARGET_DIR:-target}/debug/calternal" serve \
  --development --port 0 --create \
  --installation-id 00000000-0000-0000-0000-000000000001 \
  --appdata "$fixture_root/appdata" --userdata-ssd "$fixture_root/ssd" \
  --userdata-hdd "$fixture_root/hdd" --plugin-root "$fixture_root/no-plugins"
```

The disposable `appdata/startup.json` gives the base URL and local
development-token filenames. The owner-visible personal and shared space
each hold three fixture articles. Use the owner token and the personal
space from `/api/v1/describe` for the HTTP flow. Retain the renderer's
fixture credentials locally.

A warm Reading List window adds one registered-order lookup to the root
path, then parses metadata once on each of up to 200 selected heads.
It uses two scope probes, a covering range, the existing admission and
delivery checks and zero flushes.

## Locked performance run

The first perf-01 run executed checksum-verified release binaries under
`/usr/local/bin/perf-lock.sh`, with 100 samples per arm. Its list fixture
contained 100,000 Notes heads and requested 100 rows over HTTP using pagination
cursors for the deep pages. The comparison is `85add29a` → `49fb0187`.
The candidate's server and content crates match `e307f153`.

| Notes list offset | Baseline p95 | Candidate p95 |
| --- | ---: | ---: |
| First page | 1.071 ms | 1.247 ms |
| 10,000 | 1.069 ms | 1.430 ms |
| 90,000 | 1.619 ms | 1.261 ms |

| Effective scopes | Combined save p95 |
| --- | ---: |
| 1 | 43.688 ms |
| 4 | 52.519 ms |
| 16 | 56.589 ms |
| 32 | 60.576 ms |

All four save measurements crossed 40 ms. The first run omitted a control
save arm. It used a constant 8 KiB body, an alternating title, a zero-second
idle threshold and 6,000 members per additional explicit collection. Step 0
used a short changing body, a fixed title, a warm 120-second idle threshold and
a rotating timestamp-sorted membership. Both end the timed interval at the
`Content::write` acknowledgment. Both include the FULL revision/outbox
commit, FULL journal publication and FULL outbox retirement. Projection
maintenance is in the revision transaction. Filesystem materialization and
projection verification occur afterward. These differences prevent attributing
the initial save result to the projection. A controlled rerun copies Step 0's
seed, body, timer and background scope distribution and exercises the
product's real newest projection and membership triggers. The matched
control on `85add29a` returned 24.992 / 27.262 / 28.604 / 29.144 / 35.285 ms
p95 at 0 / 1 / 4 / 16 / 32 scopes, with 100 saves per arm. The zero-scope
arm used the original engine without a timed projection. The nonzero arms
used the Step 0 fixture triggers. In the product arm the implicit space
projection takes one scope. At one scope that leaves no moving count.
At wider scopes the fixture moves one additional newest-first scope on
each save and verifies its two counts. The fixture enrolls 257 background
audiences with the same population distribution as Step 0.

### Matched perf-01 comparison

The same-lock comparison ran checksum-verified release binaries for
`85add29a` and `c32a5982` with 100 measured samples per arm. All four
harness invocations completed. The list fixture used 100,000 Notes heads and
100-row pages over HTTP.

| Notes list offset | Baseline p95 | Candidate p95 | Change |
| --- | ---: | ---: | ---: |
| First page | 1.092 ms | 1.110 ms | +0.018 ms |
| 10,000 | 1.064 ms | 1.055 ms | −0.009 ms |
| 90,000 | 1.232 ms | 1.098 ms | −0.134 ms |

The fused query uses one prepared statement for the ordered covering range
and up to 101 primary-key head probes, with zero flushes. This removes the
per-head statement calls from the first candidate. The original list
regression at 10,000 is gone in the matched run. First-page p50 improved
from 1.000 to 0.922 ms while p99 rose from 1.115 to 1.257 ms. The candidate
still traverses the scoped index and the item primary-key index, where the
baseline uses the live-items index to reach item heads. The HTTP run did
not time those traversals separately, and a single pair of arms cannot
isolate the cause of the tail difference. The first-page p95 misses the
strict no-regression gate by 18 microseconds. Keep the lane out of
`dev` pending coordinator acceptance or another verified candidate.

| Effective scopes | Step 0 control p95 | Product p95 | Change |
| --- | ---: | ---: | ---: |
| 0 | 26.794 ms | — | — |
| 1 | 27.005 ms | 25.137 ms | −1.868 ms |
| 4 | 25.819 ms | 29.825 ms | +4.006 ms |
| 16 | 28.340 ms | 37.538 ms | +9.198 ms |
| 32 | 34.597 ms | 33.717 ms | −0.880 ms |

All four product arms passed 40 ms. The tightest margin was 2.462 ms at
16 scopes. The product maintains a version for each affected scope in the
content transaction. The spike updated one shared result-version row
alongside its key and count changes. That is a difference in SQLite work.
The p95 deltas are non-monotonic and the matched run did not time internal
statements or syncs separately. The 16-scope observed maximum was 75.842 ms.
The measured save still uses three FULL boundaries: content revision and
outbox, journal publication, and outbox retirement. The projection adds no
transaction or flush. The controlled run qualifies the matched warm,
short-body workload. The earlier 8 KiB run used a different cache policy
and membership pattern and remains unqualified.

On the measured candidate, dev-01 passed formatting, all nine release window
tests and the existing collection membership integration test. The broader
54 content and 30 HTTP tests, contract verification and documentation gate
passed on the Reading List predecessor `bdc29d76`.

### Longer first-page run and 8 KiB control

The same release artifacts ran on locked perf-01. The first-page comparison
used 1,000 samples per arm after 100 warmups, alternating which request ran
first in each pair. It also ran both queries on one warmed rusqlite
connection against the same candidate fixture with identical head decoding.
That second timer excludes HTTP, routing and authority.

| First-page measurement | Baseline p50 / p95 / p99 | Candidate p50 / p95 / p99 |
| --- | ---: | ---: |
| HTTP | 1.028 / 1.413 / 1.829 ms | 1.010 / 1.377 / 1.916 ms |
| Isolated SQLite and head decoding | 0.136 / 0.196 / 0.220 ms | 0.138 / 0.194 / 0.222 ms |

`EXPLAIN QUERY PLAN` selects the baseline's `item_live_list` and the
candidate's covering range plus item primary-key probes. The isolated
p99 difference is +2 µs, too small to account for the observed HTTP
p99 difference of +87 µs. The HTTP p99 delta across ten consecutive
100-sample blocks ranged from −580 to +359 µs and changed sign. This
supports an end-to-end tail variation explanation. It does not isolate
the host scheduling or transport source of that variation. The longer
HTTP comparison's p95 improved by 36 µs.

The 8 KiB follow-up keeps the original constant body, alternating title,
zero-second idle threshold, per-width membership growth, warmups and
post-acknowledgment placement. Both arms ran under the same lock with
100 recorded saves per width.

| Effective scopes | Baseline p95 | Candidate p95 |
| --- | ---: | ---: |
| 1 | 26.707 ms | 42.975 ms |
| 4 | 31.352 ms | 50.759 ms |
| 16 | 27.166 ms | 57.130 ms |
| 32 | 41.628 ms | 57.281 ms |

The baseline breaches 40 ms at 32 scopes. The candidate additionally
breaches at 1, 4 and 16 scopes, where the baseline passes. This workload
has a lane regression. The whole-write timer does not identify which
durability span or SQLite work accounts for the difference. A separate
stage breakdown is in progress. Keep the branch out of `dev`.

### Save gate fixture

The inherited 40 ms p95 budget is the editor `sync.save_ack` target.
The product Notes scenario measures the real HTTP `notes.update` with its
short, changing `seed … save …` body and warm Notes corpus. It keeps the
base revision current. The original product qualification used 100,000
Notes in one space as one of its fixtures and reports the warm arm
separately. The Step 0 projection experiment applies the same 40 ms target
to engine acknowledgment, using the real `Content::write` on one warm
note in its 100,000-head corpus, a short changing body and deferred file
placement. Its 8 KiB search experiment also applied the original gate,
with a warm cache and separately described search overhead.
[Product qualification](../performance.md#notes-acknowledgment-and-guest-storage)
· [Product harness](../../crates/perf/src/notes_scenario.rs)
· [Step 0 matched saves](../measurements/plugin-api-v2-step0.md#combined-content-save).

The 16 MiB thread append comparison studies foreground durability barriers
for a different operation. Its required power-cut evidence and append
path do not define the Notes save latency fixture.
[Thread append durability](../thread-append-durability.md).

### Inherited HTTP save gate on both revisions

On locked perf-01, the existing `calternal-perf` Notes scenario ran three
warm AB/BA pairs with one space, 100,000 notes, the short varying body, and
100 measured saves per arm. The harness provisioned the shared disposable
seed through `85add29a` so both revisions could open independent copies
of the same database. The candidate was `c32a5982`. The timed request,
subscriptions and placement checks are those of the original scenario.
All six arms completed and passed file placement checks.

| Pair | Baseline notes.update p50 / p95 | Candidate notes.update p50 / p95 |
| --- | ---: | ---: |
| 0 | 37.184 / 61.269 ms | 38.110 / 56.630 ms |
| 1 | 46.682 / 81.653 ms | 38.356 / 52.893 ms |
| 2 | 37.387 / 55.879 ms | 37.833 / 52.152 ms |

Every arm exceeded 40 ms p95. Candidate p95 improved in all three pairs.
The inherited HTTP gate miss predates the projection, and the harness
exited 1 for the reproduced breach. The separate 8 KiB/zero-idle engine
regression at 1, 4 and 16 scopes remains open.

### Isolating the 8 KiB save regression

A disposable build of `c32a5982` used the existing save spans plus a
span around the head and projection SQL. Two AB/BA pairs used the 8 KiB
zero-idle fixture at one and four effective scopes, with 100 measured
saves per arm and width under the perf-01 lock. Both arms kept the
window schema, fixture setup, and the same binary. The control removed
only `window_note_edit` before the save sequence. It leaves projection
keys stale and is valid solely as a diagnostic.

| Pair | Scopes | Save p95, trigger off → on | Content commit p95, off → on | Head SQL p95, off → on |
| --- | ---: | ---: | ---: | ---: |
| A | 1 | 24.931 → 59.966 ms | 8.717 → 29.755 ms | 1.422 → 1.878 ms |
| A | 4 | 33.387 → 64.445 ms | 12.575 → 33.994 ms | 1.521 → 1.850 ms |
| B | 1 | 28.427 → 42.087 ms | 8.571 → 16.822 ms | 1.516 → 1.821 ms |
| B | 4 | 29.575 → 48.906 ms | 8.980 → 22.187 ms | 1.475 → 1.821 ms |

All trigger-off arms were below 40 ms. All trigger-on arms breached.
The edit projection's writes increase the cost of the existing FULL
content commit. The SQL execution difference was below 0.5 ms p95 in
every comparison. The journal and retirement spans also varied, and stage
percentiles cannot be added. The path has the same three FULL transaction
boundaries. The probe does not distinguish changes in WAL pages written,
auto-checkpoint work, or block-device sync latency within the content
commit. Isolate those costs before changing the durable projection layout.
Both authority and count updates still need to share the item transaction.

### Page and synchronization attribution

The locked run used the same instrumented `Content::write` release
binary with the edit trigger enabled and removed, on the 8 KiB zero-idle
workload. A checksummed static strace 7.2 built on dev-01 follows the
disposable engine process and journal worker. It recorded timed `pwrite64`,
`fdatasync`, `fsync`, and checkpoint-lock calls. Per-commit markers delimit
the FULL content transaction from the surrounding save acknowledgment.
The decoder reads page numbers from WAL frame headers and associates
them with B-trees using before/after `dbstat` maps. Traced saves are
mechanism measurements. The untraced runs give the latency comparisons.

The current `window_scopes` stores total and version in one row. The 8 KiB
arm changes sort keys and versions while retaining the same membership,
so the trigger leaves the totals unchanged. The short Step 0 arm moves
one membership between two scopes at widths greater than one. It changes
two totals in different rows. Batching these saves would still touch each
affected scope row at least once. Moving the version into memory while
durably updating its same count row would also retain that page write
and would need a consistent restart generation.

Both trigger arms completed. The decoder matched all four phase markers
and reconciled frame bytes with frame headers for all 800 measured saves.
SQLite used 4,096-byte pages and 24-byte frame headers. WAL bytes below
include any WAL header written in the commit. Frames equaled distinct
page IDs at the reported percentiles.

| Scopes | Frames and distinct pages p50 / p95, off → on | WAL bytes p50 / p95, off → on | Checkpoints in commit, per 100 saves, off → on |
| ---: | ---: | ---: | ---: |
| 1 | 15 / 16 → 18 / 19 | 61,800 / 65,920 → 74,160 / 78,280 | 2 → 2 |
| 4 | 15 / 18 → 23 / 26 | 61,800 / 74,160 → 94,760 / 107,120 | 2 → 2 |
| 16 | 15 / 19 → 47 / 51 | 61,800 / 78,280 → 193,640 / 210,120 | 1 → 5 |
| 32 | 15 / 18 → 80 / 83 | 61,800 / 74,160 → 329,600 / 341,960 | 1 → 9 |

Across 100 trigger-on saves at the four widths, the identified projection
leaf touches per save were 1 / 3 / 15 / 32 in `window_cover`, 1 / 4 /
16 / 32 in `window_rows`, and one in `window_scopes`. The trigger-off
arm touched none of those projection leaves. The last leaf holds the
unchanged counts and updated versions together. Endpoint `dbstat` maps
leave some nonprojection pages unmapped or with changing ownership.
The traced workload does not support two covering leaves per scope per
save. The covering index averaged about one leaf per scope. The separate
row storage contributed about one more, and the scope versions shared a
leaf. These measurements characterize repeated updates to this warm note.
They do not measure arbitrary updates across the whole list.

| Scopes | Content sync calls p50 / p95, off → on | Summed content sync ms p50 / p95, off → on | All save sync calls p50 / p95, off → on | Summed all save sync ms p50 / p95, off → on |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1 / 1 → 1 / 1 | 6.954 / 8.691 → 6.888 / 9.700 | 3 / 3 → 3 / 3 | 20.585 / 24.500 → 20.489 / 25.935 |
| 4 | 1 / 1 → 1 / 1 | 6.995 / 8.923 → 6.824 / 7.995 | 3 / 3 → 3 / 3 | 20.667 / 23.046 → 20.064 / 23.987 |
| 16 | 1 / 1 → 1 / 1 | 6.974 / 9.059 → 7.261 / 11.213 | 3 / 3 → 3 / 3 | 20.663 / 24.242 → 20.828 / 28.333 |
| 32 | 1 / 1 → 1 / 3 | 7.802 / 11.074 → 7.556 / 16.258 | 3 / 3 → 3 / 6 | 22.827 / 28.911 → 20.778 / 35.682 |

We classified a checkpoint inside the content-commit timer when the
process acquired the checkpoint lock and wrote the main content database.
The normal acknowledgment has three FULL transaction boundaries.
Checkpoints can issue more kernel syncs within an existing boundary.
The trace establishes additional WAL traffic and more in-commit
checkpoints at 16 and 32 scopes. Noncheckpoint sync p95 does not rise
monotonically. At 32 scopes it was 11.074 ms off and 9.620 ms on.
The measured WAL amplification and checkpoint frequency account for
extra work, without identifying a proportional fsync cost for each page
or reproducing every low-width latency tail from the untraced runs.
The raw archive and decoded per-save records are retained in lane
evidence.

### Disposable options

The option harness compares unchanged FULL maintenance, the existing
`window_rows` primary key stored `WITHOUT ROWID`, and two separate-file
NORMAL-WAL variants. It runs 100 saves at 1, 4, 16 and 32 effective scopes
on both the 8 KiB zero-idle fixture and the warm short-body Step 0 shape.
The size and warmed-count checks run outside acknowledgment.

The separate file stores rows, visible titles, counts and watermark.
Foreground mode adds its bounded journal replay tick to save time.
Background mode sends a nonblocking hint through the existing
projection worker's queue and measures publication lag separately.
One save is outstanding in this fixture. An explicit hint is necessary
with zero idle retention because the content slot can close before the
worker polls active slots. Worker code for this experiment lives only
in the disposable dev copy.

Dev-01 smoke checked the packed layout, both sidecar readers and the
short body's rotating exact counts. The sidecar process-exit case cut
after the item acknowledgment and before sidecar publication. On
reopen, the source held the new title and the sidecar held the prior
title and watermark. Reading the durable journal repaired the sidecar
to the latest title and generation. This is a process-exit check.
It does not qualify hardware power loss, a long backlog, or live
production authority across the two stores.

The checksummed release harness completed all eight locked perf-01 arms.
Each table cell is p95 across 100 acknowledgments. The FULL control and
all three alternatives use the same harness and corpus for each workload.

| Workload | Layout | 1 scope | 4 scopes | 16 scopes | 32 scopes |
| --- | --- | ---: | ---: | ---: | ---: |
| 8 KiB, zero idle | FULL control | 98.032 ms | 51.952 ms | 54.729 ms | 60.894 ms |
| 8 KiB, zero idle | A · FULL, packed `WITHOUT ROWID` | 44.143 ms | 47.830 ms | 47.824 ms | 54.925 ms |
| 8 KiB, zero idle | B · NORMAL sidecar, before acknowledgment | 27.484 ms | 27.202 ms | 28.619 ms | 41.532 ms |
| 8 KiB, zero idle | C · NORMAL sidecar, after acknowledgment | 27.259 ms | 28.920 ms | 32.329 ms | 31.823 ms |
| Short body, moving membership | FULL control | 27.459 ms | 30.762 ms | 30.413 ms | 36.981 ms |
| Short body, moving membership | A · FULL, packed `WITHOUT ROWID` | 31.227 ms | 31.682 ms | 29.758 ms | 37.757 ms |
| Short body, moving membership | B · NORMAL sidecar, before acknowledgment | 25.059 ms | 33.950 ms | 36.898 ms | 43.660 ms |
| Short body, moving membership | C · NORMAL sidecar, after acknowledgment | 26.432 ms | 30.049 ms | 26.293 ms | 29.036 ms |

**A.** Packing clusters all of an item's projection rows in the primary
B-tree and removes the redundant rowid table/index split. Both workloads
still update the covering key when it changes. The current title guard
already skips explicit scope updates for equal visible titles. For this
single-item save, the count and version share one write per affected
scope row. Separately caching a version and persisting it with that same
row would retain the page write. No page-size or scope-version variant was
measured. The packed layout improved all four 8 KiB p95s in this run
and all four still exceed 40 ms. The first FULL arm's 98.032 ms p95
is materially above earlier same-fixture runs. The run does not isolate
the size of the packed layout's benefit from that variance. The SQLite
commit still orders the item and current window atomically. Crash recovery
shows either the old complete state or the new complete state, without
an index catch-up state. This needs no change to the sealed text.

**B.** Keeping NORMAL sidecar replay in the acknowledgment passes the
40 ms line through 16 scopes and misses at 32 on both workloads.
Readers can get a current window and exact count when the checked sidecar
watermark matches the durable source. If a process exits after the
source commit and before sidecar publication, it can leave a durable
item without an acknowledgment. A request after restart sees loading
with no rows or count while the index catches up. Durable receipt replay
can resume publication without writing the item twice. The split
durability boundary needs the proposed revision even if successful saves
preserve read-after-ack behavior.

**C.** The background worker passes all eight fixture/width combinations.
Observed publication lag p95 at 1 / 4 / 16 / 32 scopes was 2.152 / 2.156 /
2.175 / 2.184 ms for 8 KiB and 2.152 / 2.153 / 2.178 / 2.176 ms for the
short body. The observer polls every 2 ms with one save in flight.
The 32-scope lag p99 was 20.830 ms and 16.791 ms respectively. After a
process exit, an acknowledged item can be newer than the last published
generation. A ready old window requires the explicit freshness semantics
below. A loading reply with no rows or count is the conservative first
implementation. The prototype qualifies the engine timer on these two
fixtures. It does not qualify the separate inherited HTTP save gate,
mixed workloads, a backlog, power-loss durability, or current viewer
authority during replay.

### Proposed revision 14 — Journal-backed window generations

**Decision for the owner.** Should a Notes save be allowed to acknowledge
before the ordered window and count reflect that save? The current v2
qualification keeps the item, authorized ordered keys and maintained
counts in the same transaction. A different answer changes what renderers
can safely show immediately after a save and after restart. For example, a
member changes an article's title and its order in a 100,000-row list.
Today the next valid current window observes the committed ordering.
A journal-backed index may still have the previous generation when that
same window is requested.

The authoritative item, membership and grant updates stay durable on the
existing commit path. A rebuildable projection may publish a generation in
one transaction in its own database. That transaction includes the row
snapshots used for materialization, exact per-scope totals, and the durable
journal epoch and watermark. On startup the host checks the current epoch,
authoritative outbox and relevant authority before reading an index. A
mismatch, interrupted publication or lost sidecar WAL holds the view in
loading until bounded durable replay or rebuild verifies the publication.
Replay from a durable watermark is idempotent. Older/newer source epochs
cannot be combined. Scope admission still enforces the 32-scope bound
inside the authoritative mutation.

For a foreground sidecar publish, the host waits for the verified current
generation before acknowledging a successful current-list edit. The window
can return exact counts from that generation. If the item committed and
publication failed, a retry with the same request ID resumes from the
durable receipt and waits for publication. It never repeats the item write.

For a background publish, the item acknowledgment can precede the new
window. A ready reply from the last published generation needs an explicit
stale marker and opaque generation token. Its rows and count must come
from the same snapshot, with current authority checked before reading
and again before release. The existing current `exact` count is omitted
while stale. A distinct snapshot-exact count would need its own typed
marker, renderer treatment and owner decision. If scope authority, grant
revision or eligibility changed and the old row set cannot be proven safe,
return loading without rows or a count. Renewal and cache reuse fence the
generation as well as grant revision. A superseded snapshot cannot authorize
an action against the live head.

**Recommendation.** Hold this branch from `dev`. Authorize option C only
as the next product qualification candidate under this proposed revision.
It was the sole prototype below 40 ms p95 at every tested scope width
on both fixtures. Option A preserves the sealed freshness and crash behavior
with one store, and missed all four 8 KiB arms. Option B preserves
read-after-ack behavior after successful publication, adds a recovery
boundary and a possible retry after a durable write, and missed both
32-scope arms. C trades immediate window freshness for lower save
latency. Require a separate backlog, power-cut, revocation and exact-count
gate, plus the inherited HTTP Notes save gate. Start with loading and
no count during catch-up. Enable an explicitly stale ready window only
after the owner approves the renderer behavior and the authority tests
show that every row still qualifies. If immediate window freshness remains
a requirement, retain the sealed same-transaction contract and keep the
merge held.

## Product background publication

The owner approved option C on 2026-11-20. A durable save can acknowledge
before the ordered window catches up. While its journal epoch, watermark,
source configuration, or outbox check is unsettled, an authorized window
returns `state: loading`, an empty row array, null version and window
references, and omits the count. The host clears its retained seek boundary.
Publication advances keys, scope totals and the watermark in one sidecar
transaction. The reader loads row fields from the live source snapshot.
Once the checks match, exact counts describe the same current set as its
covering index. A root Notes window may serve the published exact result across at most
128 contiguous, classified journal entries when neither those entries nor
any pending intent affects Notes or collections. Other cases withhold the
entire response until the publication reaches the current journal head.

The content schema adds version 9 to the previous release's version 8. The
existing release migration runner marks already opened databases after
additive migration. The first image-pair trial at `3a2620e6` found that
the previous release refused a space first opened after cutover. That
creation now uses the shared schema runner's exact predecessor marker
for version 8. The migration
reuses the existing Notes head and reverse membership indexes, the content
outbox, the durable journal, and the shared search projection worker. The
32-scope bound is checked in the authoritative transaction. Earlier
releases could already have saved an oversized membership. The sidecar
records each affected item and omits its explicit-collection rows. The
root Notes and Trash windows still include it in the appropriate state. The index and
count rows live in a separate private SQLite WAL database with
`synchronous=NORMAL`. The first authorized v2 window requests a build.
The shared worker polls active spaces for further durable journal entries.
Authorized loading windows may wake the worker. The legacy `notes.list`
timestamp and UUID cursor queries the live heads as on dev.

| Path | Reads and index work | Synchronous flushes |
| --- | --- | --- |
| Existing-item ACK | Existing content and journal statements. The projection adds zero reads, SQLite statements or queue requests. | Same three FULL transaction boundaries as dev. |
| Warm ready v2 window, current head | No opens or attachments when reusing a reader. One publication read, two journal head statements (two seeks each), two source config/outbox checks, one source binding lookup, one scope lookup, one covering range, and one source point lookup per returned row. Declared orders and parents add point checks. The host checks live authority before storage and before delivery. | 0 |
| Warm root Notes with unrelated lag | Each freshness check may add one journal range, at most 128 indexed kind probes and one epoch recheck. The outbox check examines at most 129 pending intents and probes kinds for at most that many. Unknown entries fail closed. | 0 |
| Cold v2 window | May stat/open the sidecar, check its schema version, attach the source and open the journal reader with one journal binding lookup. Each slot can cache up to two reader/journal pairs and one writer/journal pair. The service retains at most eight pairs of each class and expires them after five idle seconds. The cap controls retention only. A request or replay can open a pair on demand. | 0 |
| Replay or rebuild step | One page of up to 128 journal entries, source items or rows removed. A collection event advances a separate member UUID cursor in batches of up to 128, drawing from bounded seeks into source edges, prior edges and the recorded omissions. | At most one NORMAL sidecar commit per step. The worker also performs creation, checkpointing and recovery. |
| Collection save | Unchanged member arrays still require old/new JSON comparison. A retitle does zero reverse-edge mutations and no scope-cap probes. Adding an edge checks whether it already exists and probes the capped reverse-note range. Membership changes compare bounded arrays of up to 10,000 entries. | Existing authoritative flushes. |

The sustained-save fixture starts with one published note, commits 260
updates and one additional note, then commits another 64 updates between
the first and second replay pages. It sees loading after 0, 128,
and 256 entries have replayed. The final 69 entries publish two rows and
an exact count of two. The HTTP suite holds a prepared sidecar behind
its writer lock, verifies loading after an item commit, revokes the active
member, and verifies the old lease returns HTTP 404. The content fixture
sends SIGKILL to a writer subprocess after the item is durably acknowledged.
It separately restores a checkpointed sidecar image and removes its WAL
and SHM to deterministically discard a published sidecar tail. Both
recovery paths withhold rows and count until replay restores the exact
authoritative result. A real power cut on a disposable VM was not
exercised. It would additionally test persistence ordering across the
guest OS, hypervisor and block device, including interrupted checkpoint
and file creation writes.

On dev-01, `2c6cb481` passed 57 content tests, 31 HTTP tests with
`test-plugins`, formatting, Rust contract export and the sealed fixture
verifier. On the branch as checked in, `scripts/check-docs.sh` reached the
generated-site check and refused two stale installer pages inherited
from dev. The command inputs for `calternal plugin install` and
`calternal plugin review` are identical on dev and on this branch.
Regenerating those pages in the disposable checkout let the complete
docs gate pass, including all 436 pages. Dev then regenerated the two
installer pages in `95ea43c0`.

`scripts/verify-additive-upgrade.sh` passed in both directions on dev-01
with the running release `localhost/calternal-ai:31fb41179460` and the
candidate `localhost/calternal-ai-ca85:2c6cb4813a26`. The proof binds
the previous image ID
`563931284f834459ed3e242de5496b722714e82373d60b8a6bba0b893601d710`
to candidate image ID
`3db2a71cc106cfb40e54636b02e23414419272b85304c7a6194f93e63f687a56`.
Both images were exercised against disposable data on dev-01.

### Inherited HTTP Notes gate

The locked perf-01 comparison uses the static server binaries extracted
from those same two images. Their SHA-256 digests are
`10b63189edf7f0b4218b5307287e8c69fb3de2b77e6d9f09366147011f5481c9`
(running release) and
`95e5950f30b96d9df8b84be770858788c1310536914bb90d883c649d815c5ed4`
(candidate). The runner restores a common stopped 100,000-note baseline
seed under the same pinned plugin distribution. It alternates three warm
100-save pairs and times real HTTP `notes.update` with short changing
bodies, with separate file placement and subscriber observations. This
fixture exercises native Notes reads and saves before any v2 window has
requested a sidecar. It measures the inherited 40 ms p95 save gate.

| First run pair | Release save p50 / p95 (ms) | Candidate save p50 / p95 (ms) | Release / candidate native first-page p95 (ms) |
| ---: | ---: | ---: | ---: |
| 0 | 23.161 / 34.192 | 27.957 / 36.769 | 1.609 / 1.563 |
| 1 | 28.913 / 47.321 | 28.470 / 37.179 | 1.600 / 1.435 |
| 2 | 23.508 / 38.449 | 27.375 / 47.473 | 1.606 / 1.456 |

Each release breached the save ceiling in one arm, in different pairs.
Across 300 saves per release, 16 baseline and 17 candidate responses
exceeded 40 ms. The first-page p95 improved in all three pairs. The
runner completed all arms with zero file-placement breaches. The plain
matrix's exit status of 2 indicates that the wider performance suite
requires separate release commit timing, job and idle probes.

The candidate's median save was higher in pairs 0 and 2. Those pairs
also saw higher candidate medians in save-to-subscriber delivery and
acknowledgment-to-placement. The raw samples therefore show differences
on both sides of the acknowledgment. This comparison has no per-stage
timing and does not isolate any extra worker scheduling from storage or
host variation. The evenly alternating repeat uses four warm pairs.

| Save-wake repeat pair | Release save p50 / p95 (ms) | Candidate save p50 / p95 (ms) | Release / candidate native first-page p95 (ms) |
| ---: | ---: | ---: | ---: |
| 0 | 27.744 / 35.842 | 28.197 / 38.270 | 1.650 / 1.520 |
| 1 | 24.088 / 35.734 | 23.227 / 35.120 | 1.748 / 1.547 |
| 2 | 23.138 / 33.441 | 24.240 / 35.519 | 1.743 / 1.639 |
| 3 | 27.748 / 35.965 | 35.534 / 63.082 | 1.696 / 1.498 |

All baseline arms passed in the repeat. The candidate breached one arm,
with 36 saves above 40 ms. No arm failed or missed the file placement
budget. Native first-page p95 again improved in every pair.

The first candidate woke the shared worker after each saved item. That
wake also cleared Search's cached readiness, and the fixture's
`calternal-notes` package enables Search. This can change Search scheduling
with a cold window sidecar. `5370a9d0` removes the wake from the save path.
The search worker already polls active spaces after its existing 50 ms
idle wait. A qualifying HTTP test observes a new ready count in the sidecar
after a save, without issuing a window request, before holding a later
publication stale and exercising revocation.

On dev-01, `5370a9d0` passed 57 content tests, 31 HTTP tests, formatting,
contract export, and the sealed fixture verifier. The full docs check
passed with the same two inherited installer pages regenerated in
scratch. The exact additive image-pair check passed again against the
running release. The candidate image ID is
`2d929dfbb0a81f24482ad1af0ca9532d906dc9c8efad9242aa8db1886c2586d0`.
Its static server SHA-256 is
`f605adaebce9114190dd489709cfad0582b463ce003deb8bccd22d3c1779399b`.

| Poll-only pair | Release save p50 / p95 (ms) | Candidate save p50 / p95 (ms) | Release / candidate native first-page p95 (ms) |
| ---: | ---: | ---: | ---: |
| 0 | 23.642 / 36.710 | 22.718 / 33.075 | 1.898 / 1.730 |
| 1 | 23.645 / 35.520 | 26.053 / 34.174 | 1.477 / 1.467 |
| 2 | 23.383 / 35.683 | 23.138 / 34.579 | 1.460 / 1.693 |
| 3 | 27.913 / 44.229 | 32.688 / 55.696 | 2.111 / 1.780 |

Both releases passed three of the four poll-only pairs. The candidate
was faster at save p95 in the three passing pairs and slower in the
fourth, when both breached 40 ms. That pair had six above-limit saves
on the baseline and 15 on the candidate. The median save-to-subscriber
and acknowledgment-to-placement both increased for both releases in
the fourth pair. With neither sidecar work nor a replay wake in the
candidate save path, the remaining tail warrants a hold. The run
does not isolate those slower saves to a particular FULL commit.
The native first page was faster in three pairs and 0.233 ms slower
in one. Its SQL is identical to the baseline. All arms completed and
passed the file-placement budget. Across the 400 samples per arm, pooled
nearest-rank save p95 was 36.710 ms on the release and 41.898 ms on the
candidate. Pooled native first-page p95 was 1.725 ms and 1.693 ms,
respectively.

### Monitored save comparison

The follow-up uses the same two packaged binaries and fixture with ten
alternating warm pairs. It adds monotonic request boundaries to the
disposable harness and leaves the measured server code and HTTP timer
unchanged. The external host monitor follows the web timing lane's
one-second CPU counter and pressure deltas. A separate process writes
4 KiB and calls `fsync` every 200 ms on the same filesystem, with 30
seconds of idle reference before and after the comparison. Monitor logs
go to tmpfs. The per-arm window runs from the first save request's start
to its final save acknowledgment and covers the intervening saves.

For attribution, a control spike is a write plus fsync at or above
the larger of 10 ms and twice the idle control p99. Count overlaps
within 200 ms of a save. Compare that rate for slow and ordinary saves,
as well as the distributions across whole arms. A proposed quiet-arm
rule, contingent on the evidence, would reject a pair if either arm
has two such control spikes or one control operation above 40 ms.
This rule uses the independent control before judging save latency.

The monitored perf-01 run completed 20 arms with 100 saves each and no
placement breaches. The table reports milliseconds. The monitor recorded
642 one-second host intervals and 2,916 control writes on the ext4
filesystem. The control's idle p95/p99/max were 16.209/24.261/60.449 ms.
The prespecified spike threshold was 48.522 ms.

| Pair | Arm | Save p95 | Save p99 | Save max | Control write+fsync p95 | Control write+fsync max | Control fsync p95 | Control fsync max |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | Release | 34.729 | 39.339 | 94.436 | 20.639 | 22.658 | 20.62 | 22.644 |
| 0 | Candidate | 38.884 | 71.157 | 83.097 | 20.171 | 21.824 | 20.157 | 21.808 |
| 1 | Release | 63.074 | 93.164 | 130.062 | 35.384 | 42.488 | 35.348 | 42.456 |
| 1 | Candidate | 37.787 | 51.747 | 86.475 | 22.273 | 24.432 | 22.259 | 24.418 |
| 2 | Release | 47.769 | 58.242 | 97.996 | 27.371 | 30.591 | 27.356 | 30.575 |
| 2 | Candidate | 47.608 | 85.403 | 94.307 | 32.5 | 48.973 | 32.488 | 48.948 |
| 3 | Release | 62.128 | 77.441 | 105.244 | 40.109 | 44.785 | 40.077 | 44.766 |
| 3 | Candidate | 40.018 | 57.479 | 99.612 | 23.855 | 24.529 | 23.841 | 24.514 |
| 4 | Release | 53.452 | 98.589 | 133.771 | 23.403 | 39.623 | 23.384 | 39.607 |
| 4 | Candidate | 60.646 | 79.908 | 105.518 | 25.527 | 46.807 | 25.514 | 46.794 |
| 5 | Release | 40.943 | 65.558 | 88.504 | 19.108 | 24.601 | 19.094 | 24.588 |
| 5 | Candidate | 46.483 | 67.431 | 94.756 | 25.06 | 26.485 | 25.046 | 26.472 |
| 6 | Release | 39.01 | 68.293 | 84.772 | 22.266 | 25.047 | 22.25 | 25.017 |
| 6 | Candidate | 47.357 | 64.871 | 85.221 | 27.641 | 28.262 | 27.627 | 28.244 |
| 7 | Release | 40.45 | 45.695 | 84.35 | 19.426 | 22.055 | 19.411 | 22.031 |
| 7 | Candidate | 37.324 | 51.717 | 98.696 | 18.994 | 19.672 | 18.98 | 19.658 |
| 8 | Release | 68.621 | 102.268 | 171.459 | 64.936 | 124.257 | 64.919 | 124.237 |
| 8 | Candidate | 57.551 | 74.14 | 116.227 | 27.587 | 34.288 | 27.57 | 34.27 |
| 9 | Release | 39.242 | 49.02 | 100.567 | 21.104 | 22.371 | 21.089 | 22.353 |
| 9 | Candidate | 57.232 | 99.604 | 107.538 | 33.35 | 37.415 | 33.336 | 37.399 |

Each release missed the 40 ms save gate in seven arms. Nine of 185
above-limit saves were within 200 ms of a control spike, compared with
10 of 1,815 other saves. Baseline pair 8 showed a clear shared-I/O event.
Its control reached 124.257 ms and peak full I/O pressure reached 52.117%.
The proposed quiet-pair filter excludes pairs 1, 2, 3, 4 and 8. Five of
the ten remaining arms still missed. This experiment therefore does not
justify a quiet-only passing gate. The 200 ms probe can miss short I/O
events, and five control fsyncs per second can alter the shared workload.
The next comparison tests that observer effect, with the commit-stage
build paused.

### Control-probe observer effect

The observer comparison moved to perf-02 after preflight found a
Firecracker guest on perf-01. On perf-01, Notes used ext4 `8:1` and the
`vm88` logs used XFS `8:17`. On perf-02, the initial locked preflight and
all 16 pre-arm checks found zero foreign guests, relays or builds, and
the `vm88` log directory was absent. The run used the same two release
binaries with four probe-off and four probe-on pairs interleaved. Both
conditions used each binary order twice. The arm preflight followed the
warm copy and its `sync`.

| Pair | Arm | Probe | Save p95 (ms) | Save p99 (ms) | Save max (ms) | Control p95 / max (ms) | Free / available RAM (MiB) | Cache (MiB) |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | Release | off | 33.109 | 53.356 | 84.011 | - / - | 315.6 / 3629.0 | 6338.7 |
| 0 | Candidate | off | 32.895 | 52.280 | 84.919 | - / - | 311.9 / 3612.4 | 6338.8 |
| 1 | Release | on | 40.865 | 53.525 | 97.236 | 21.841 / 23.547 | 287.9 / 3592.0 | 6338.8 |
| 1 | Candidate | on | 37.297 | 48.157 | 93.450 | 28.021 / 35.612 | 295.7 / 3603.9 | 6338.9 |
| 2 | Candidate | off | 29.820 | 43.200 | 89.763 | - / - | 305.4 / 3616.0 | 6338.9 |
| 2 | Release | off | 37.002 | 59.150 | 89.960 | - / - | 300.4 / 3613.7 | 6339.0 |
| 3 | Candidate | on | 38.494 | 68.925 | 90.626 | 18.695 / 20.208 | 295.8 / 3610.9 | 6339.0 |
| 3 | Release | on | 35.031 | 53.877 | 92.525 | 18.299 / 19.813 | 274.9 / 3593.8 | 6339.1 |
| 4 | Release | off | 34.981 | 38.257 | 86.190 | - / - | 287.6 / 3608.9 | 6339.1 |
| 4 | Candidate | off | 33.390 | 41.735 | 103.400 | - / - | 265.8 / 3589.4 | 6339.1 |
| 5 | Release | on | 35.941 | 47.098 | 90.882 | 21.451 / 24.447 | 280.8 / 3605.7 | 6339.2 |
| 5 | Candidate | on | 42.239 | 70.372 | 90.203 | 23.015 / 24.302 | 285.4 / 3612.5 | 6339.2 |
| 6 | Candidate | off | 57.727 | 93.382 | 177.325 | - / - | 275.1 / 3603.4 | 6339.3 |
| 6 | Release | off | 33.754 | 138.615 | 253.917 | - / - | 273.1 / 3603.0 | 6339.3 |
| 7 | Candidate | on | 37.865 | 51.024 | 86.429 | 20.928 / 28.872 | 276.8 / 3607.3 | 6339.3 |
| 7 | Release | on | 61.852 | 114.467 | 124.458 | 26.663 / 54.060 | 272.5 / 3603.9 | 6339.4 |

The active writer increased median arm p95 on both binaries. Release
p95 moved from 34.368 to 38.403 ms and candidate p95 from 33.143 to
38.180 ms. Within adjacent blocks with the same binary order, the
median increases were 4.358 and 6.538 ms, respectively. The release
missed zero of four off and two of four on, and the candidate missed
one arm in each condition. Pair 6's candidate save p95 was 57.727 ms
with the probe off and full I/O pressure reached 63.706%. This
establishes an observer effect on perf-02, while the probe-off miss
shows it explains only part of the tail. The clean perf-02 preflights
exclude a concurrent lane workload inside that VM during this run.
They do not identify the cause of every older perf-01 arm or establish
isolation of the underlying hypervisor storage.

### Passive device wait and same-save syncs

Perf-02 ran ten alternating pairs without a control writer, using
the same release binaries. The host sampler captured per-device await
from `/proc/diskstats`. The existing `strace -T` syscall approach
recorded only the launched server's `fsync` and `fdatasync` calls. Trace
output went to tmpfs. Monotonic request spans were correlated with
the trace's wall clock using each arm's preflight offset. The thread
ID and ordering around the journal commit assign calls to content,
journal and retirement. This includes foreground checkpoint syncs
and excludes concurrent placement calls on a different thread.

| Pair | Arm | Save p95 / p99 / max (ms) | Content / journal / retirement sync p95 (ms) | Content / journal / retirement sync max (ms) | Device / flush await (ms) |
| ---: | --- | ---: | ---: | ---: | ---: |
| 0 | Release | 38.738 / 51.888 / 201.851 | 10.114 / 9.334 / 10.064 | 18.256 / 16.057 / 17.353 | 2.566 / 3.925 |
| 0 | Candidate | 41.689 / 54.117 / 172.922 | 10.593 / 9.637 / 9.582 | 22.482 / 23.694 / 14.058 | 2.290 / 3.375 |
| 1 | Release | 37.923 / 44.272 / 181.617 | 10.523 / 8.778 / 9.291 | 18.484 / 15.468 / 13.677 | 2.307 / 3.418 |
| 1 | Candidate | 39.240 / 50.422 / 184.002 | 12.473 / 8.995 / 9.544 | 16.787 / 14.951 / 20.340 | 2.531 / 3.468 |
| 2 | Release | 40.959 / 49.444 / 186.598 | 10.946 / 10.792 / 9.069 | 15.544 / 16.357 / 13.932 | 2.902 / 4.069 |
| 2 | Candidate | 43.234 / 53.748 / 183.510 | 11.347 / 10.055 / 10.147 | 22.214 / 15.194 / 23.410 | 2.324 / 3.411 |
| 3 | Release | 45.241 / 53.954 / 173.259 | 10.967 / 11.306 / 9.389 | 16.612 / 21.392 / 20.701 | 2.330 / 3.481 |
| 3 | Candidate | 41.191 / 56.699 / 186.594 | 11.821 / 9.745 / 10.695 | 24.158 / 16.219 / 15.269 | 2.514 / 3.998 |
| 4 | Release | 45.711 / 64.518 / 185.103 | 15.298 / 13.288 / 12.392 | 18.172 / 21.503 / 31.625 | 2.623 / 3.927 |
| 4 | Candidate | 41.239 / 52.689 / 175.312 | 10.058 / 10.347 / 9.539 | 14.773 / 16.633 / 23.257 | 2.311 / 3.430 |
| 5 | Release | 64.199 / 168.707 / 187.331 | 20.638 / 16.907 / 15.289 | 124.277 / 25.523 / 29.755 | 2.993 / 4.828 |
| 5 | Candidate | 39.694 / 53.901 / 188.158 | 9.901 / 12.012 / 9.387 | 23.449 / 19.135 / 13.068 | 2.516 / 3.914 |
| 6 | Release | 36.884 / 52.408 / 173.102 | 9.457 / 9.567 / 8.602 | 21.283 / 13.573 / 13.263 | 2.370 / 3.765 |
| 6 | Candidate | 40.911 / 49.395 / 173.895 | 10.077 / 10.449 / 9.562 | 15.214 / 17.200 / 13.526 | 2.321 / 3.443 |
| 7 | Release | 39.329 / 53.602 / 181.132 | 9.694 / 9.400 / 8.892 | 15.828 / 19.495 / 13.655 | 2.603 / 3.368 |
| 7 | Candidate | 43.964 / 74.122 / 180.047 | 11.652 / 14.324 / 8.337 | 22.450 / 21.774 / 21.100 | 2.280 / 3.372 |
| 8 | Release | 37.636 / 62.597 / 179.414 | 8.810 / 9.575 / 9.481 | 18.902 / 24.906 / 13.413 | 2.284 / 3.346 |
| 8 | Candidate | 38.743 / 49.692 / 198.410 | 9.326 / 9.126 / 9.146 | 21.418 / 19.197 / 12.976 | 2.233 / 3.281 |
| 9 | Release | 39.306 / 56.981 / 192.428 | 10.588 / 9.829 / 9.574 | 22.525 / 26.219 / 21.713 | 2.390 / 3.574 |
| 9 | Candidate | 38.107 / 44.774 / 181.844 | 8.412 / 9.324 / 9.260 | 15.656 / 15.511 / 13.345 | 2.472 / 3.983 |

All 2,000 saves matched. Four release arms and six candidate arms
missed at p95. Each arm had exactly three saves with a foreground
checkpoint sync on both binaries. The highest content sync was
124.277 ms on the running release, in pair 5. Excluding each arm's
first save for this diagnostic comparison, pooled content, journal
and retirement sync p95 were 12.617/12.157/11.032 ms on the release
and 10.439/10.316/9.681 ms on the candidate. The pooled HTTP p95
for these 990 later saves per release was 42.216 and 40.656 ms.

In later responses above 40 ms, combined median time in the three
sync stages was 35.780 ms on the release and 33.740 ms on the
candidate. Ordinary responses spent 20.370 and 20.299 ms there.
Median time outside those sync calls rose by 0.710 ms and
1.497 ms respectively. Checkpoints occurred in 23 of 66 and 24
of 54 later slow responses. The recurring tail is largely in
the existing sync and checkpoint path shared by both binaries.
The trace did not find an added candidate sync boundary.

The first save was the maximum in every traced arm. Median first
save was 183.360 ms on the release and 182.677 ms on the candidate.
Only 25.333 and 21.870 ms, respectively, were in the measured
foreground sync stages. The trace does not assign the remainder
to a stage. Since syscall tracing itself changes the measurement,
this series diagnoses the tail and does not qualify the untraced
gate. The controlled comparison showed that the active storage
probe raised p95, and the locked perf-02 checks found no live
foreign workload inside that VM. Neither finding explains every
miss. Keep the branch held.

## Review gate

The measured poll-only candidate `5370a9d0` was built on dev
`37cadea0`, which includes the #89 Notes delivery fix. The running release
`31fb4117` has the same server, plugin and Cargo build inputs as that base.
The final series was rebased onto `origin/dev` at `a4154906`.
`eb4516db` has the same Rust, plugin, web and deployment build inputs
as the measured candidate. The exact rebase passed 57 content tests,
31 HTTP tests, formatting, contract export, the sealed verifier and the
full 436-page docs gate using the installer pages inherited from dev.

1. **Shared types.** Reuses `SpaceId`, `AccountId`, `Page`, the shared
   `ErrorCode` and the directory's capability set. The window envelopes
   are the first Rust types for these sealed message shapes. Public routes
   reuse them directly.
2. **Dependency direction.** No new crate. Server resolves authority and
   Bind declarations, content owns SQL and durability, and existing lower
   crates supply shared types and filesystem/journal services. SQLite is
   accessed through rusqlite.
3. **Request isolation.** Readers open the selected space's read-only
   databases under its owner lifecycle guard. Server lease mutexes are
   sharded and only protect bounded
   in-memory bookkeeping. No mutex introduced by this lane spans I/O or
   an async yield. Each space owns its rate and permit counters. Shards
   partition the map only. The existing process-wide authority
   revision is atomic and can retire unrelated leases. This costs an
   unnecessary reopen in those cases. No thread or global SQLite connection
   is introduced.
4. **Async safety.** Server uses its existing blocking adapter for
   synchronous directory and SQLite work, including the live session and
   authority checks inside the content callback. Start-up installs fixture
   rows through the same blocking boundary. The async route performs typed
   decoding, in-memory admission, and bounded response encoding.
5. **Hot path.** Native Notes decodes its cursor once and reads its existing
   live head page. V2 reads the covering index once and materializes only
   returned rows, with the authority and freshness reads counted in the
   product table. Both routes issue zero write flushes. A save retains
   the existing head, placement, receipt and journal reads and three FULL
   boundaries. Replay adds no reads, statements, queue requests or flushes
   to the save.
6. **Constant work.** The sealed catalogue is decoded once per owning
   layer with `OnceLock`. SQLite statements use the connection's prepared
   cache. The fixture checks the pinned package and compiles its Bind at
   initialization. The route serializes the approved description on
   discovery.
7. **Bounds and retention.** Per item, the engine admits at most 32 window
   scopes. Scope counts disappear when empty, and projection rows follow
   live membership and the content lifecycle. The 128-slot wake queue
   holds space IDs. Each sidecar holds one publication watermark, a
   bootstrap cursor, a membership cursor and at most one omission per
   inherited over-limit note. The private retry marker is one bounded record.
   Published heads survive idle handle eviction. The service retains at most
   eight reader and eight writer pairs, with a five-second idle expiry. Each query is capped by the
   sealed 4,096 materialized-row and 250,000 skip budgets, with the request
   size further bounded by the catalogue. Leases expire after 15 seconds,
   with at most 128 retained entries per shard and one preceding result
   per lease. Expiration is pruned on admission and saturation. The lane
   uses existing content retention, including the configured receipt
   retention and its seven-day default.
8. **Additive upgrade.** The existing native cursor and response remain
   usable on either release. Source schema 9 adds the definitions and
   admission trigger to schema 8. The sidecar contains rebuildable state.
   The Reading List install fixture remains explicitly development-only.
9. **Space identity.** Use typed UUIDs at the route and content boundaries.
   Index keys are local to a space. Notes and Reading List are kind and
   app identities.
10. **Single declarations.** The sealed catalogue supplies query caps. Its
    32-scope ceiling is enforced in the authoritative SQLite trigger and
    checked during replay for inherited memberships. The scope boundary
    test checks these SQL limits against the catalogue. Resolve package
    field and Bind policy against the pinned
    description and manifest before building a trusted projection.
11. **Callers.** Leased Notes windows and the Reading List fixture use the
    engine, and the existing projection worker calls replay. The
    **v2 scoped collection and nested host route lane**
    must wire explicit collections, Trash, parent scopes, public/guest
    variants, and collection-order provisioning. The **v2 host activation
    and authority lane** must call `WindowResult::item_for_row` when
    resolving checked row selectors. Estimated counts require an issuer in
    the **v2 host count policy lane**. These engine surfaces have tests.
    Until the coordinator assigns those callers, their integration remains
    open. Issue #85 names the follow-up lanes. Guest and public windows
    are reachable from no product route in this merge. The registry lane (#87)
    owns checked manifest admission. The host follow-up can use its
    `Registry::snapshot` and `Catalog::v2_kind` plus `ManifestV2::reads`
    to qualify package declarations. Those lookups confer no activation
    or viewer authority.
12. **Reuse.** Uses the existing head and reverse membership indexes,
    the item commit/outbox and durable journal, the search projection
    worker and its queue, and the existing directory/session authority.
    The host uses its existing signing key and blocking adapter.
13. **Documentation.** Public items carry API comments. The qualification
    records the format, contract, fixture and documentation gates.
14. **Code and report.** The crate README and storage reference describe
    the scoped projection. This dated report records measurement and
    product-route limitations. The sealed source contracts remain
    unchanged by this lane.
15. **Formatting and file size.** The implementation passed
    `cargo fmt --all -- --check`. Content SQL, registration, query, replay
    and recovery tests live in separate files. Per-space leases, query
    admission and the package fixture have separate modules.

## First independent review follow-up, 2026-11-20

These measurements use the first review's candidate. The second review
follow-up below records subsequent recovery and omission behavior.

1. **Recovery.** Confirmed the baseline returned 500 for a corrupt sidecar.
   The shared worker now discards the derived DB, WAL and SHM and rebuilds.
   A per-space private marker distinguishes reader and replay failures, with
   a retry delay from 200 to 3,200 ms. Recovery logs at warn and the route
   returns HTTP 503 `unavailable` while a known failure persists. A path
   that cannot be safely removed stays unavailable until repaired. Only an
   inherited over-limit note is skipped, with its reason recorded in the
   sidecar. Other rows publish. HTTP covers corruption, unopenable storage
   and recovery. The content test covers editing and repairing an
   inherited over-limit member.
2. **Idle polling.** Confirmed repeated SQLite opens in the baseline. The
   worker compares the cached published head to the in-memory journal head.
   A dormant space without an authorized window request skips projection
   I/O. The first cache implementation failed near 50 active spaces at
   the perf host's SSH soft limit of 1,024 file descriptors. A separate
   dev-01 reproduction traced `openat(content.db-wal) = -1 EMFILE`.
   Idle connection retention is now capped separately at eight reader
   pairs and eight writer pairs for the service. A space can retain two
   reader pairs and one writer pair for up to five idle seconds. Cache
   exhaustion and expiry preserve the in-memory head and ordinary admission.
3. **Request reads.** Confirmed one cold open and attach sequence for every
   baseline request. The pooled path reuses both connections, checks
   authority outside the read transaction, and uses the reads in the table
   above with zero flushes. A warm-reuse test marks the borrowed SQLite
   connection and observes the same marker after a second product query.
   A full cache test proves a later space still gets its exact result.
4. **Other-kind traffic.** Confirmed root Notes starvation in both fixtures.
   Root Notes can now prove irrelevance across up to 128 contiguous
   journal entries, with bounded pending-intent inspection and before/after
   freshness and authority checks. Unknown events, relevant writes,
   unresolved epochs and partial membership replay still withhold rows and
   count. HTTP covers unrelated and relevant writes against a held
   projection lock. A deterministic save between row materialization and
   delivery also withholds rows and exact count. A real viewer can still
   need a fresh lease when the process-wide authority epoch changes.
5. **Bounded replay.** Confirmed unbounded delete and collection work.
   Epoch and gap recovery clear at most 128 derived rows per transaction.
   Bootstrap reads at most 128 source IDs. A collection event persists
   a member UUID cursor and bounds the next batch to 128 notes. The
   content test observes 128 of 129 members with no released result,
   resumes the last member and checks the bounded reset of a prior epoch.
6. **Collection retitles.** Confirmed repeated reverse-index work and the
   inherited-data edit failure. The authoritative trigger now computes a
   membership delta. Retitling compares old/new serialized member arrays
   once and runs zero reverse-edge mutations or scope-limit probes. A
   genuinely new edge does the existing indexed admission. The concern
   about other admitting writers mapping to 500 did not hold. Both
   collection HTTP mutations use `Content::write` and `Staged::write` and
   return typed 422 on new overflow. Linked and live head writes share
   that path. Direct placement, purge and Notes-only recovery SQL do
   not add collection edges. Inherited active edges stay editable.
   Restoring a collection adds live edges and must fit the cap.
7. **Review gate.** Confirmed stale docs and shared quotas. Actual space
   IDs own their rate and permit counters. The HTTP test exhausts exact
   starts in one space and still serves another UUID in the same former
   shard. The budget for retained SQLite handles does not gate requests.
   Issue #85 names the callers for every remaining window scope, audience,
   row selector and estimate. Guest and public windows are reachable
   from no product route in this merge.

### Locked review measurements

Baseline server source was `eb4516db`. Candidate server source was
`92dd29c8`. Both binaries were built on dev-01, transferred with
SHA-256 verification and run under `/usr/local/bin/perf-lock.sh` on
perf-02. The idle runner used the default 1,024-descriptor soft limit.
Each arm used fresh disposable roots and one published Notes window
per space. The Linux `/proc` CPU counters and the `search-index` thread's
selected `strace` syscall counters came from separate 60-second runs.
The trace selected file syscalls, close, read, pread64, pwrite64, fcntl,
fsync, fdatasync and futex. Candidate totals include one-time idle
handle closes. Futex accounts for 1,196 calls at one space and 1,195
at 50. No `openat` call occurred in either candidate measurement.

| Open spaces | Worker CPU ms before / after | Process CPU ms before / after | Worker selected syscalls before / after | Worker openat before / after | Candidate FDs start / end |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1,850 / 30 | 1,900 / 70 | 160,433 / 1,220 | 7,063 / 0 | 59 / 50 |
| 50 | 33,840 / 90 | 34,120 / 310 | 1,040,601 / 1,315 | 46,094 / 0 | 635 / 593 |

For the streaming fixture, a disposable loopback shim feeds synthetic
one-byte chunks into the normal assistant-output checkpoint handler and
verifies the durable thread text. It uses a single thread and one turn,
targets one durable append every 50 ms, and makes no provider calls.
The import fixture saturates real HTTP `threads.create` requests in the
same space. The paced checkpoint runs sustained 19.7 and 19.5 successful
appends per second. The uncapped imports sustained 17.0 and 18.2 successful
creates per second. An earlier repeated-send probe hit the one-active-turn
guard and was discarded. The first baseline shell wrapper printed an EOF
after the locked server and retitle arms had emitted their complete records.
The result checker validated each arm's sample count. Both fixtures sample the real exact Notes window at 500 ms intervals
for 60 seconds. The harness reopens a stale lease once and records
the retry separately. Remaining import responses outside ready/loading
were HTTP 404s from the existing authority epoch. The baseline's
streaming ready latency reflects only two ready samples. The baseline
longest runs of 64 and 28 consecutive loading samples represent roughly
32 and 14 seconds respectively.

| Workload | Arm | Durable changes | Ready / loading / other windows | Longest consecutive loading samples | Lease retries | Ready p95 (ms) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| checkpoint | before | 1181 | 2 / 118 / 0 | 64 | 2 | 4.778 |
| checkpoint | after | 1168 | 120 / 0 / 0 | 0 | 2 | 1.937 |
| import | before | 1017 | 1 / 113 / 6 | 28 | 119 | — |
| import | after | 1092 | 115 / 0 / 5 | 0 | 119 | — |

The retitle fixture times the real durable `Content::write` path for a
collection with 5,000 distinct stored member IDs and unchanged membership.
Each arm warms ten saves, then measures 100 saves with 25 ms between writes.
The drafts change the title and write the full collection payload each time.
Baseline p50/p95/p99/max were 84.078 / 93.712 / 105.580 / 113.181 ms.
Candidate values were 27.232 / 51.256 / 57.205 / 66.444 ms. This includes
the existing history, journal and placement work. The retitle binary
reuses the candidate's identical collection trigger and writer from the
preceding build, with the background projection worker disabled in both
retitle arms. The 40 ms inherited gate measures short-body HTTP Notes
edits on the warm 100,000-note fixture.

Server SHA-256 digests were
`6f9ef4e3cab3bd01dc85d1b2d20442dd20eebb4e67d55d420f8aa7b658496263`
and `5e31ea94fd8b648491d46ebecb804145eba4c05a254c51c8d12468685b6d9ec1`.
The checkpoint fixture binaries were
`0438f8cb70053d497f85e10600fe587f3ac6b60bae51679e17e73e42c07fe9bc`
and `077bfe7e5eb0528b6d55f4346f0b1feb51a3fdbf41a84c30bbb67a69721b83e5`.

On dev-01, `92dd29c8` passed `cargo fmt --all -- --check`,
`cargo test --locked -p calternal-content --tests` (61 tests),
`cargo test --locked -p calternal-server --features test-plugins --test http`
(36 tests), Rust contract export with `--check`, the sealed fixture
verifier and `scripts/check-docs.sh` (436 generated pages).
The release-image upgrade and backend acceptance gates for this revision
remain with the coordinator. The earlier physical power-cut qualification
gap remains open.

## Second independent review follow-up

- Over-limit notes keep the root Notes or Trash row. Replay omits only their
  explicit-collection rows. The first committed omission for an item logs
  one warning. The record remains through a move to Trash until memberships
  are repaired or the item is purged. The private `window-status` command
  reads the recorded count and persistent failure state.
- Corrupt, foreign-format or incompatible-schema sidecars are replaced.
  Failures in the authoritative database, its journal or an ambiguous
  cross-database statement retry in place. A poison item with malformed
  metadata reaches a persistent failure after five attempts. Reads return
  typed unavailability during backoff and after exhaustion. Inspect the
  failure category with `calternal window-status`. After repairing a
  terminal source failure, stop the server, remove that space's private
  `windows-v1.failure` marker and restart to resume the saved cursor.
- A committed ready sidecar clears its replay failure even with pending
  journal entries. Root Notes applies its existing disjoint-lag proof.
  Relevant lag returns loading without rows or an exact count.
- SQLite sidecar schema version 2 includes the membership cursor and
  omission table. A previous generation at the same derived path rebuilds.
  The authoritative schema and sealed contracts retain their existing versions.
- A content request stages one changed item and its outbox row together.
  Linked edits use a distinct request for each item. The outbox key enforces
  uniqueness and the writer asserts insertion of one row. The journal
  publishes that same item, which the root Notes lag check classifies.
- Issue #85 tracks collection-scoped omission repair, an inode-swap audit
  and lag acceptance for additional scopes together.

The coordinator's full-workspace run on `2afa2a19` hit the existing
one-second userdata placement guard during the sustained-write fixture.
The preceding `save-253` journal commit took 662 ms on the nearly full,
shared dev filesystem. The test now retries that specific backpressure
response with the same request ID, revision and payload for up to ten
seconds. Its assertions still check the same 128-entry replay boundaries
and keep count withheld until the final relevant page.

## Simplification opportunities

- Generalize the existing collection membership relation before qualifying
  plugin kinds and parent edges. It can own the reverse membership lookup and
  the window projection updates for all those callers.
- Use a space-local authority revision when the installed-view service can
  identify its complete dependency scope. The current process epoch can
  retire a view after unrelated authority or session activity.
- Share the bounded journal cursor and epoch-reset logic with Search. The
  projections can use a common replay loop while bearing their own row
  derivation and publication checks.
