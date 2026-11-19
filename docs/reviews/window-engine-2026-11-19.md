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
