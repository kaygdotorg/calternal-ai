# Window query engine, 2026-11-19

Issue [#85](https://git.kayg.org/kayg/calternal-ai/issues/85) implements
the first ordered content projections. Unfiltered `notes.list` uses the same
covering seek as the sealed API and keeps its signed timestamp/UUID cursor and
summary shape. The API also qualifies live explicit Notes collections ordered
by their exposed title, using SQLite’s ASCII lowercasing and UUID as the tie break. Other sources, sort/filter
shapes and parent windows require their own qualified plan.

## Storage and authority

- Reuses the live reverse membership index in `collection_members.sql`, the
  per-space SQLite writer and `Staged::write`. Schema 9 backfills current
  heads and live memberships atomically. The membership trigger applies
  additions and removals. Triggers on item heads also cover lifecycle moves and
  the linked rename path.
- Each live Notes item has one space projection and up to 31 distinct
  collection projections. Private direct item shares authorize item reads.
  Current people, group and public grants to the same explicit collection
  authorize the same live member set. A grant change in the protected
  directory makes that set newly accessible or inaccessible, with zero
  content writes or index-key fan-out. Membership changes maintain the set in
  the content transaction. The 33rd scope aborts the whole transaction with
  HTTP 422 `effective_scope_limit_exceeded`, including its staged receipt.
- Guest rows expose titles and view-scoped selectors. The title determines
  ordering. Hidden metadata or body edits leave the guest version unchanged.
  The host must resolve the live explicit collection, admission, field policy,
  lease and grant revision. The reader checks authority before opening the
  index and again before release, and discards the entire reply if the scope
  version changed. The existing sharing routes retain their own grant checks.
- Existing public-link records have no count-disclosure field. The host should
  select mode `none` for those grants. The API accepts `exact` for an
  explicitly count-authorized projection and a member estimate certified for
  the current result version. Guests receive no estimate or end-relative
  result without exact count authority. Deep starts require the prior
  ready-window boundary from the same live view and omit count.

## Work accounting

| Path | Reads / index work | Flushes |
| --- | --- | --- |
| Warm sealed result | Two maintained scope probes, one covering range, at most 200 title lookups, and two host authority evaluations. A seek visits one page. An arbitrary jump visits at most width plus 250,000 eligible keys from the nearer permitted end. An exact out-of-range reply uses zero covering keys. | 0 |
| Warm native unfiltered Notes page | One covering seek and up to 101 head reads for the 100-row page. Existing dispatch checks authority on admission and delivery. | 0 |
| Warm existing-note save | One head point read and one root-key update. A title change adds one head point read and a reverse range over at most 31 explicit memberships, with at most two covering keys per changed scope. A sort or title change leaves the count alone. | 0 additional. The existing FULL content/history, journal/receipt and outbox retirement barriers still apply. |
| Membership update | Delta over the bounded collection membership, with indexed note admission and scope-count maintenance for changed edges. | 0 additional. Uses the collection's existing item commit. |
| Equivalent grant or revoke | Existing protected directory admission and mutation. No content reads or added keys for the window projection. | 0 additional. Existing protected authority commit. |

Index rows are bounded per item. Scope counts and versions are deleted when
their last row leaves. Readers reuse the existing read pool where available
and open a read-only connection for an idle space. They require an initialized
schema on existing databases. A read runs no journal or placement operation.
Retain seek boundaries only within the host's current view lease.

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
- **Proposal for existing oversized membership.** A retained space can have
  more than 31 explicit collection memberships per note from an earlier
  writer. A live item above the cap refuses schema backfill atomically. A
  previously trashed item above the cap cannot restore until membership is
  reduced. For example, a note in 32 lists would require 33 physical scopes.
  Refusal preserves every list but leaves the affected space or restore
  unavailable. Automatically dropping memberships would change saved data.
  Keep atomic refusal and add an operator preflight before any rollout that
  may encounter these spaces.
- **Proposal for the encoded byte limit.** The engine uses
  `materialization_limit` for a window whose encoded reply exceeds the
  catalogue limit. For example, 200 long multibyte titles can exceed 128 KiB
  even though the row count is legal. Returning a short successful page would
  falsely signal the end. Keep the typed refusal and explicitly document its
  use when a full reply exceeds the byte cap.

## Registered kinds and parent windows

Schema 10 uses existing kind registrations and the same covering table and
maintained counts. Claimed-text kinds default to newest. The host can provision
one scalar sort and title field from a checked kind declaration, such as
`reading-reminder.due`. Both directions use the same B-tree and reverse the
complete stable order, including its ID tie-break. A replacement definition
rebuilds only that kind's window rows in one space commit. Notes Trash has a
separate exclusive projection in move-time order. An explicit Notes collection
can be provisioned in newest order for members, or title order for guests.
A non-matching sort is a typed plan refusal.

A registered parent field adds a covering key under the parent UUID and child
kind. The host supplies the checked UUID and the issued edge token after
validating the current view, Bind and entire ancestry. Child moves maintain
both parent counts in the same item commit. A nested window uses the same
bounded range and title materialization as a root window, plus one indexed
definition lookup and two indexed checks that the parent still exists. A
new read connection also verifies its stored space binding. The normal Notes native read count is unchanged.
A generic warm save bounds maintenance to the item, its root scope and at most
one parent scope. It adds no flush. A manual collection window adds one
configuration probe.

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

Standalone count scheduling, field projections beyond title, layout placement
windows, generic manual collection membership and search are outside this
slice. Product qualification requires the locked perf run and the current
view/lease adapter. The original Notes release binaries and their same-host
baseline are queued under the perf-01 lock.

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
one. Route admission uses sharded actor and space limits, one in-flight
window per view and the catalogue's rate and lease limits. There is no
SQLite write or fsync for open, window or renew. Window storage reads match
the root path above. The route adds two protected membership reads and two
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

## Review gate

The series was replayed onto `origin/dev` at `f41c3429`. The replay had
no conflicts and the measured candidate's crate sources, Cargo inputs and
sealed contract bytes match the rebased runtime files.

1. **Shared types.** Reuses `SpaceId`, `AccountId`, `Page`, the shared
   `ErrorCode` and the directory's capability set. The window envelopes
   are the first Rust types for these sealed message shapes. Public routes
   reuse them directly.
2. **Dependency direction.** No new crate. Server resolves authority and
   Bind declarations, content owns SQL and durability, and existing lower
   crates supply shared types and filesystem/journal services. SQLite is
   accessed through rusqlite.
3. **Request isolation.** Reads use the content lane's per-space SQLite
   readers. Server lease mutexes are sharded and only protect bounded
   in-memory bookkeeping. No mutex introduced by this lane spans I/O or
   an async yield. Rate and permit shards are conservative shared quotas
   for IDs that hash to the same shard. The existing process-wide authority
   revision is atomic and can retire unrelated leases. This costs an
   unnecessary reopen in those cases. No thread or global SQLite connection
   is introduced.
4. **Async safety.** Server uses its existing blocking adapter for
   synchronous directory and SQLite work, including the live session and
   authority checks inside the content callback. Start-up installs fixture
   rows through the same blocking boundary. The async route performs typed
   decoding, in-memory admission, and bounded response encoding.
5. **Hot path.** Native Notes parses a cursor once and executes one covering
   range with up to 101 head probes. The v2 engine takes two indexed scope
   probes, a covering range, and one head lookup and metadata parse per
   returned row. Registered kinds add one order lookup. A child adds
   parent-live checks before and after the snapshot. Both routes use
   existing authority checks and zero write flushes. A save retains the
   existing head, placement, receipt and publication reads and three FULL
   boundaries. Projection maintenance adds bounded index work in the
   content transaction.
6. **Constant work.** The sealed catalogue is decoded once per owning
   layer with `OnceLock`. SQLite statements use the connection's prepared
   cache. The fixture checks the pinned package and compiles its Bind at
   initialization. The route serializes the approved description on
   discovery.
7. **Bounds and retention.** Per item, the engine admits at most 32 window
   scopes. Scope counts disappear when empty, and projection rows follow
   live membership and the content lifecycle. Each query is capped by the
   sealed 4,096 materialized-row and 250,000 skip budgets, with the request
   size further bounded by the catalogue. Leases expire after 15 seconds,
   with at most 128 retained entries per shard and one preceding result
   per lease. Expiration is pruned on admission and saturation. The lane
   uses existing content retention, including the configured receipt
   retention and its seven-day default.
8. **Compatibility.** Existing native cursor and response contracts stay
   stable. The v2 routes are additive. The feature-gated Reading List
   fixture explicitly fails when the old installer starts accepting the
   pinned v2 manifest. There is no fallback in the ordinary installer.
9. **Space identity.** Use typed UUIDs at the route and content boundaries.
   Index keys are local to a space. Notes and Reading List are kind and
   app identities.
10. **Single declarations.** The sealed catalogue supplies query caps. Its
    32-scope ceiling is enforced in SQLite triggers so concurrent and
    indirect writes receive the same atomic refusal. The SQL literal is a
    second representation of that limit, covered by the scope boundary
    test. Resolve package field and Bind policy against the pinned
    description and manifest before building a trusted projection.
11. **Callers.** Native Notes and the leased Notes and Reading List routes
    use the engine. The **v2 scoped collection and nested host route lane**
    must wire explicit collections, Trash, parent scopes, public/guest
    variants, and collection-order provisioning. The **v2 host activation
    and authority lane** must call `WindowResult::item_for_row` when
    resolving checked row selectors. Estimated counts require an issuer in
    the **v2 host count policy lane**. These engine surfaces have tests.
    Until the coordinator assigns those callers, their integration remains
    open. Issue #85 names the follow-up lanes. The registry lane (#87)
    owns checked manifest admission. The host follow-up can use its
    `Registry::snapshot` and `Catalog::v2_kind` plus `ManifestV2::reads`
    to qualify package declarations. Those lookups confer no activation
    or viewer authority.
12. **Reuse.** Uses the existing per-space content index, reverse collection
    membership, item commit/outbox and journal/receipt path, and existing
    directory/session authority. SQLite triggers extend that transaction
    and reuse its rollback. The host uses its existing signing key and
    blocking adapter. There is no rank table, per-plugin store or separate
    projection writer.
13. **Documentation.** Public items carry API comments. On dev-01, the
    archived rebase at `299257a0` passed formatting, 54 content tests and
    30 HTTP tests with `test-plugins`, contract export, the sealed fixture
    verifier and `scripts/check-docs.sh` (429 pages). Later commits in
    this series record results in this report.
14. **Code and report.** The crate README and storage reference describe
    the scoped projection. This dated report records measurement and
    product-route limitations. The sealed source contracts remain
    unchanged by this lane.
15. **Formatting and file size.** The measured candidate passed
    `cargo fmt --all -- --check`. Content SQL, registration, query and
    tests live in separate files. The server route module is 601 lines.
    Its package fixture has its own module.

## Simplification opportunities

- Generalize the existing collection membership relation before qualifying
  plugin kinds and parent edges. It can own the reverse membership lookup and
  the window projection updates for all those callers.
- Use a space-local authority revision when the installed-view service can
  identify its complete dependency scope. The current process epoch can
  retire a view after unrelated authority or session activity.
- Move the timestamp-to-key conversion into a single shared primitive if
  another native cursor needs this sort. The current formats are locked
  together by the notes cursor contract.
