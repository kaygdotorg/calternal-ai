# Plugin API v2 item writer qualification

Lane [#95](https://git.kayg.org/kayg/calternal-ai/issues/95) covers Slice 2.
The host base is draft 14.35 at `36d49713`. The Threads spike at
`6125ad96` supplies the exchange sequence. The owning package's checked
invocation or attempt attributes a write to the acting person.
Unproven writes set protection. Staged receipts retain separate begin
and final-save identities.

## Implementation order

1. In `crates/content/src/revision_slice.rs`, add the exact-revision
   BLOB page primitive. Use the shared `RevisionRef` and keep existing
   adjacent-revision reads on the same BLOB reader. Qualify it against
   the real content database in `crates/content/tests/v2_item_ranges.rs`.
2. Extend the existing content writer and outbox for checked placement,
   write transformations and discovery. Exercise create, append and leaf
   rename against the checked revision and the existing placement
   recovery. Add schema steps only through the content migration list.
3. Bind the wire exchanges to an authenticated package, operation and
   invocation through the v2 broker. Check item read and writer
   authority before commit or delivery and bind create outcomes to
   their invocation.
4. Add bounded staging and test exact replay, invalidation, restart and
   the release upgrade before measuring on a locked perf VM.

## First qualification on dev-01

Drive the installed-package broker and the space writer using the operands
from `plugins/calternal-threads/test/host.ts`. Register two disposable
packages with separate owning kinds. Use the existing content lifecycle,
publication worker and a temporary userdata root. Assert typed replies,
durable rows and materialized bytes at the boundaries below.

Trace both callers. The app creates the conversation, reads the head and
range, updates title metadata and appends output. The contribution creates
its own log and appends its reply. The log returns a portable locator and
confers no app item access or activation navigation. Test that supplying
that locator in place of a checked item ref does not grant access.

| Exchange | Acceptance checks |
| --- | --- |
| `item_create` → `ItemContentReceipt` | Set a folder, leaf, title metadata and Markdown bytes. A second create at the same readable location allocates a deterministic suffix. A matching retry returns the original receipt, locator and revision. A different body with that request ID conflicts. Reject reserved names, traversal, symlink ancestors and a saturated 32-candidate probe without creating an item. Race two creates for the same location. |
| Invocation output → `ActionOutcome.created` | Navigation receives the exact item created by the declared operation and an exact replay of it. Return an old, foreign or unrecorded ref, swap refs between concurrent invocations, or claim a ref from an operation without `creates`. None produces a created outcome. Revoke delivery authority after commit and verify the ref is withheld. |
| `item_head` → `OwnedItemHead` | Read current metadata, byte length and revision. Wrong owning package, Trash and absent item have the same opaque refusal. A caller with title access alone cannot read a body length. |
| `item_save` → `ItemSaveReply` | Check the head, replace bytes, change the title and rename the leaf in one commit. The leaf stays in the original folder. Metadata-only edits preserve the bytes and live checkpoint. Exercise occupied names and a stale expected revision. A conflict reply exposes the current head only under live read authority. |
| `item_range` → `ItemRangeResult` | Assemble the old bytes in paged reads of at most 64 KiB while the head advances. Check padded base64, exact end boundaries and omission of the cursor on the final page. Revoke between read and delivery, then exercise purge and pin expiry. |
| `item_append` → `ItemAppendReceipt` | Append opaque output to an opted-in item, check the exact byte boundary, lose the ACK and retry with the same request ID. Append another output frame after rename using the new revision. Replaying the first append returns its original boundary. Race unmatched attempts, stale revisions, rename and retirement. Exactly one writer claims a given expected head. |

## Recovery, discovery and staging

- Inject a cut before the FULL content commit, after commit before
  publication, and during readable placement. Restart the same space.
  Before commit there is no new revision, intent or receipt. After commit
  recovery produces one publication and exact replay. Check both filename
  and body together after recovery, including an occupied destination.
- Start with a host-attested creator-only item visible to its creator's
  attempt. Race an opaque import with list, search, direct head/range
  and pending derived output. Once the write commits, those paths exclude
  the item, including old revision refs and cache entries. Holder reads
  with ordinary live authority still work. An ordinary save or Undo cannot
  lift the marker.
- Begin a 2 MiB escape-heavy replacement, then send 32 chunks of 64 KiB
  and seal the SHA-256 digest.
  Repeated matching begin, chunk and seal calls are idempotent. Changed
  input, conflicting overlap, holes, oversized chunks, a bad digest and
  foreign or invalidated handles fail. Exercise two handles per actor,
  four and 8 MiB per package, and eight and 16 MiB per space. Check 60-second
  idle expiry, reclamation within 30 seconds, startup orphan cleanup, head
  advance and live revoke. Seal publishes nothing. Save consumes the stage
  under the writer fence and a lost ACK replays the committed receipt.
- The release upgrade proof is retired. Owner decision, 2026-11-20 (#78):
  production is unused and every deploy before the Threads cutover is a
  fresh install, so schemas are edited in place with no migration chain.
- Run the 2 MiB latency gate on a perf VM under
  `/usr/local/bin/perf-lock.sh`, using the verified development build
  artifact. Reconstruct the saved bytes after advancing the head, under a
  live 60-second range pin. Require p95 at most five seconds and byte-exact
  reconstruction. Record the sample count, p95 and test arms separately
  from dev qualification.

## Reuse points to verify

- Files use the existing no-follow beneath-directory primitive, guarded file
  swaps and collision normalization in `calternal-fsio`. Link edits use
  the existing content rename engine when requested.
- Durability uses the existing per-space content revision and outbox
  transaction, journal writer, and pending materialization recovery.
  Slice 2's tables are one content schema step, edited in place.
  The existing `restart_finishes_content_commit_before_file_placement_and_receipt`
  test supplies the recovery fixture. Include the ordinary file lifecycle
  in the dev gate.
- Locking uses the existing per-space writer and lifecycle permits. The
  broker runs synchronous storage work on its blocking worker.
- Validation reuses kind/file registration, bounded content encoding and
  installed package declarations. Wire types come from the shared host
  base.
- The Step 0 fixture on `origin/lane/api-v2-step0` provides the large-save
  input and crash arms. Drive those checks through the typed broker with
  the contract's SHA-256 stage.

## First storage verification

On dev-01 the checked writer test passed create with a folded-path
collision, atomic title/body/leaf edit, rejection of an append from a
stale head, and checked append. The range test reconstructed an immutable
version after the head advanced. All 12 existing content integration
tests passed in the same run.

- Range uses one indexed lookup and at most one incremental SQLite BLOB
  read. It reads no userdata and does not flush.
- Warm update loads one head, hashes the current file, and reads one
  current version for the body and metadata. A leaf change probes at
  most 32 candidate names through the inherited name cache and indexed
  folded-path check. Pending placement also finishes the previous
  revision, then rereads the header because its file hash changed.
- A fresh write checks the request's receipt once, then commits one
  FULL content transaction with any protection row. The publication
  worker path reads the pending outbox, the item route and the receipt,
  commits the journal and retires the outbox, for two further FULL
  commits. New item routing adds a catalog insert and its FULL commit.
  The inherited placement worker flushes the file and directory.
  Creating each missing folder also flushes its parent.
- The 64 KiB append and page bounds and 2 MiB document bound reuse the
  content engine's revision retention. Protection occupies at most one
  row per item and the outbox receipt uses the configured seven-day
  default retention.
- Slice 2 adds one content schema step (9). The compatible-creation
  marker and the release-pair proof that it needed were removed with the
  owner's fresh-install decision.

## Broker qualification on draft 14.35

The dev-01 item engine test exercised the checked create, head, range,
save and append exchanges. It used the live content writer with a pinned
manifest fixture. It verified exact create and append replay, rejected a
foreign invocation's created claim, and refused delivery after revoke.
The package kept creator-only discovery through checked writes. A write
without the owning invocation then fenced direct head reads and delivery
of an already pinned revision. The broker staged and reconstructed
2 MiB after head advance and replayed both committed stage identities.

The item response uses a durable opaque ref minted with the create. The
portable locator carries the independent item UUID. Ref lookup takes one
indexed space query. The head reader takes one query with an indexed
version probe for size and a separate lookup for the opaque ref.
Metadata checks are bounded by the declaration's field limit. A range
uses one indexed receipt-free version query and up to one BLOB read.
Attempt discovery adds indexed creator/protection checks at admission
and delivery. Reads make no flushes.

Staging uses provisional space memory. The limits are two handles per
actor, four handles and 8 MiB per package, and eight handles and 16 MiB
per space. Chunks and seal perform no userdata or database writes.
Seal checks the SHA-256 digest once. The existing writer commits the
reference or attribution, version and replay data in its ordinary FULL
content transaction. The inherited journal and placement flushes still
apply. The author-context evidence row follows its revision's lifetime.

Slices 3 and 4 supply attempt admission and the carrier adapter. Slice 6
plumbs created outcomes through activation and owns the shared stage
pool and its idle reclamation. Slices 5 and 8 supply transport egress and
list/search projection qualification. The new callback test covers the
broker and writer boundary. The enrolled Threads path, pre-ranking
discovery filters, scheduler cleanup and the locked performance gate
still require qualification. Draft 14.36 adds source-byte evidence and is
required before enabling discovery through the live product path.

## Live enablement and checked sources on 14.44

Rebased on `lane/v2-host-base` 3e62ca07 (revision 14.44).

**Activation.** `plugins.enable` and `plugins.disable` accept an installed
v2 package. Enable registers the package's declared kinds through
`Store::provision_plugin`, the same path v1 uses. Both record the plugin
control row and its receipt. The item engine fences every commit on that
exact control value inside the content writer. Disable commits under the
same space lock, so a write cannot commit after a disable that came first.
Disable then fences the package's awake placements. Reads and deliveries
recheck the control row.

**Dispatch.** A declared v2 operation in an enabled space goes to
`space_broker::package::invoke`. It checks the person session, the
operation's `required_capability`, `space.read` and `plugins.use`. It
validates the author input against the install-cached schema, wakes or
reuses the package placement, and mints a ticket. The ticket checks the
same capability on its grant revision. The carrier `invoke` now carries
the contract's `author_context` (request ID, actor, authored time) through
the machine to the worker pipe. Each item callback (`item_create`,
`item_head`, `item_range`, `item_spans`, `item_save`, `item_save_begin`,
`item_save_chunk`, `item_save_seal`, `item_append`) runs through an owned
`ItemEngine` on a blocking worker. The engine moves between callbacks, so
no thread waits on the worker. Terminal output is validated against the
declared output schema and the created-outcome rule. Staged saves use one
pool per space. A cold wake reads the pinned worker from the installed
archive once and rechecks its digest.

**Checked sources (14.36–14.38).** An attested write composes package
framing (at most 512 bytes each) around a middle the host selects:

- `input`: the text a checked composer or editor route bound to the
  operation's `write_origins.host_input` field, passed to `invoke` as
  `captured`. It must equal that input field and is scoped to the target
  item or the create.
- `output`: the resolver the execution runtime attaches for the exact
  attempt (`with_attempt_outputs`). It is not wired yet (slices 1 and 3).

An ineligible source is refused with `ineligible_source` and is never
turned into an opaque write. The writer records the full span, middle,
origin and optional entry anchor in the same transaction as the revision.
A reused live anchor conflicts before any effect. The receipt carries the
span ref, and the fingerprint binds the digest of the selected middle.
Raw plugin bytes, including creates with a body, appends and staged saves,
set `consent_only` (`DiscoveryWrite::Protect`). A trigger on `versions`
retires every live span and marks the body unproven for any other body
change to a checked item. That includes other host writers that do not
maintain spans yet. `item_spans` pages up to 128 spans of a pinned
revision with an HMAC cursor bound to package, actor, space, item and
revision.

Not yet implemented: `replace_span` edits and deletes, checked quotes,
`host_ranges` from host rewrites (link rewriting currently retires spans,
which fails closed), and the attempt-output resolver.

**Append hot path** (warm placement, one attested or raw append; counted
from the code, not measured). SQLite reads: item ref resolve 1, receipt
lookup 2 (engine replay check and writer), activation control 2 (in-lock
fence and delivery), item header 1, current version body 1 (the whole
body is read and rewritten as the new version), creator check 1, anchor
check 1, trigger probes 2. Userdata: one full read and hash of the item
file (`check_header`). If the previous revision's file is still pending,
it is placed first (file write, file and directory fsync). Flushes on the
request path: one FULL content commit, plus the publication path's journal
commit and outbox retirement (two further FULL commits). File placement
for the new revision is deferred to the lifecycle worker.

**Qualification on dev-01.**
`enrolled_package_writes_and_reads_items_through_live_callbacks` installs
the package through the real archive path and enables it through
`plugins.enable`. It dispatches through the router entry, over the TLS
carrier, to an enrolled runner with a Node child worker. The worker
creates, appends, reads the head and a pinned range, and is refused an
uncaptured attested source. A captured attested create returns one
recorded span with the person origin and anchor, and stays creator-only
discoverable. The raw item is protected and unproven. An exact retry
replays every subeffect without a new revision. A capture that differs
from the input is refused. After disable the operation is not found. The
engine tests cover attested create and append spans, cursor forgery, a
capture-bound fingerprint conflict, anchor reuse, foreign-target captures,
and an opaque staged save that retires spans while the earlier revision
keeps its own.

## Simplification opportunities

- Use the immutable version BLOB range reader for both ordinary pinned
  pages and existing adjacent-version callers. Keep a single readable
  placement and outbox recovery path for create, save and append.
- Retire the operator-approved item-head profile (`ApprovedItemHead`,
  `Registry::with_space_item_heads`). It has no production caller, and the
  package path serves `item_head` through the engine. Its carrier and KVM
  product tests should move onto the package path first.
- An append reads and rewrites the whole current body as a new version
  row. A delta or chunked version store would make append cost
  independent of thread length.
- Every deploy is now a fresh install, so the space `content.db` steps 4–9
  could collapse into the initial batch. This lane owns that list, but the
  steps carry other lanes' tables, so it is left for a separate change
  after the coordinator confirms.
- The engine checks the receipt before the writer, and the writer checks
  it again. One of the two lookups can go.
