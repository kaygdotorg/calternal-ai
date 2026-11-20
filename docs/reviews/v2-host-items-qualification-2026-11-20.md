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
- Build and run the release upgrade proof on dev-01 with
  `scripts/verify-additive-upgrade.sh` and previous image
  `localhost/calternal-ai:31fb41179460`. Record both image digests and
  preserve the proof outside the repository.
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
  Any schema addition uses the existing additive migration gate.
  The existing `restart_finishes_content_commit_before_file_placement_and_receipt`
  test supplies the recovery fixture. Include the ordinary file lifecycle
  and release upgrade in the dev gate.
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
- The migration uses the existing compatible-creation marker so the
  previous schema-8 writer can open a space first created by the
  schema-9 candidate. The bidirectional release proof covers that
  creation path alongside existing-space migration.

The release proof passed for `e74c765e` on dev-01 using
`localhost/calternal-ai:31fb41179460` and candidate image
`localhost/calternal-ai-items:e74c765e5ec1`. Podman, Firecracker and
provider sign-in leak checks passed. The proof lives at
`/home/claude/Developer/lanes/v2-host-items/upgrade-proof-e74c765e5ec1.json`.

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

## Simplification opportunities

Use the immutable version BLOB range reader for both ordinary pinned pages
and existing adjacent-version callers. Keep a single readable placement and
outbox recovery path for create, save and append.
