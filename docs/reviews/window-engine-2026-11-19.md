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

- The GenUI v2 host view/lease integration lane will call
  `Content::window` and issue action contexts, coordinate the lease and
  partition, and apply the transport delivery check for the Svelte and SwiftUI
  consumers in #81 and #82. Native Notes exercises the index and planner in
  product code today. There are no persisted member-cache records in this lane.
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

Nested windows, generic plugin kinds, standalone count scheduling, field
projections beyond title and the remaining qualified list sorts are pending.
Search belongs to its own lane. This work runs functional gates on dev-02.
The product save p95 and cold-page latency gates require the separately
authorized locked perf run.

## Simplification opportunities

- Generalize the existing collection membership relation before qualifying
  plugin kinds and parent edges. It can own the reverse membership lookup and
  the window projection updates for all those callers.
- Move the timestamp-to-key conversion into a single shared primitive if
  another native cursor needs this sort. The current formats are locked
  together by the notes cursor contract.
