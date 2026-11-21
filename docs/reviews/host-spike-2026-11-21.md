# Host spike: step 1 of the primitives-only host, running · 2026-11-21

Status: throwaway spike for issue #107, branch `lane/spike-host`, code under
[`experiments/host-spike/`](../../experiments/host-spike/). It changes no
product crate and is not in the product workspace (it has its own
`[workspace]`). Nothing merges without the coordinator.

Design read: `docs/proposals/primitives-only-host-2026-11-21.md` §0–7 and §9 A,
`primitives/files-and-questions.md`, `primitives/permissions-and-calls.md`,
`docs/measurements/record-files-2026-11-21.md`.

## Result in one paragraph

One plugin does one real thing on a running server. A small Rust binary
serves one route, `POST /call`. Reading List is installed as data only: the
JSON declaration has no code, and the one validator checks it. Its kind's
write grant lets `files.create` and `files.set` run. The test adds three
links over HTTP, marks one read and asks "Unread". The answer is the same
from the index and from a directory scan, and the files on disk are plain
Markdown. A second test appends three records to a chat-like record file as
two people and deletes one in place, padded. It then forges a record and
changes a header and a body straight on disk. The host stamps the forgery
`outside`, puts the changed header back from its stamp, and detects the
changed body but cannot put it back. That last part is the main finding:
the recommended answer to D5 (hashes only) and A21 ("the record file stays as
it was") cannot both hold (finding 1).

## What runs

| Piece | Where |
| --- | --- |
| Binary `host-spike` (`init`, `serve`) | `src/main.rs` |
| Call context, call table, `authorize`, audit, receipts, request IDs | `src/call.rs` |
| Declaration types and the one validator | `src/decl.rs` |
| `plugins.install`, `files.create`, `files.set` | `src/files.rs` |
| `record.append`, `record.delete`, `record.list`, checking outside changes | `src/records.rs` |
| `query` (scan and index), index rebuild | `src/query.rs` |
| Frontmatter read and line-wise edit | `src/fm.rs` |
| Where things live; host state and index schemas | `src/host.rs` |
| Reading List and a chat-like declaration | `plugins/*.json` |
| End-to-end tests against the real binary | `tests/reading_list.rs`, `tests/records.rs`, `tests/common/mod.rs` |
| Flush counter (strace) | `count-flushes.sh` |

### Commands

On `calternal-dev-01-compute02`. The spike needs the repository's root
`Cargo.toml` and `crates/types` and `crates/fsio` beside it, so use a checkout
of this branch:

```sh
cd experiments/host-spike
export TMPDIR=/tmp CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
~/bin/lock-shared nix develop ../.. -c cargo test -j3          # 3 tests: 1 unit, 2 end to end
~/bin/lock-shared nix develop ../.. -c cargo clippy --all-targets -j3   # clean
nix develop ../.. -c cargo fmt --check                          # clean
nix shell nixpkgs#strace -c ./count-flushes.sh target/debug/host-spike
```

To try it by hand:

```sh
S=019a0000-0000-7000-8000-000000000001; A=019a0000-0000-7000-8000-0000000000a1
target/debug/host-spike init --root /tmp/h --space $S --member $A:Ana:owner
target/debug/host-spike serve --root /tmp/h --addr 127.0.0.1:7070 &
curl -s localhost:7070/call -H "x-person: $A" -H "x-space: $S" -H 'x-plugin: host' \
  -H 'content-type: application/json' \
  -d "{\"name\":\"plugins.install\",\"input\":{\"declaration\":$(cat plugins/calternal-reading-list.json)}}"
curl -s localhost:7070/call -H "x-person: $A" -H "x-space: $S" -H 'x-plugin: calternal-reading-list' \
  -H 'content-type: application/json' \
  -d '{"name":"files.create","input":{"kind":"reading","fields":{"url":"https://sqlite.org/wal.html","title":"WAL"}}}'
curl -s localhost:7070/call -H "x-person: $A" -H "x-space: $S" -H 'x-plugin: calternal-reading-list' \
  -H 'content-type: application/json' -d '{"name":"query","input":{"collection":"Unread"}}'
cat "/tmp/h/spaces/$S/Reading/WAL.md"
```

Last run (dev-01, 2026-11-20 ~21:10 UTC): all 3 tests pass, clippy and fmt clean.

### What the tests check

`reading_list_end_to_end`:
- Only a manager may install, and only from the host's own screen (`x-plugin: host`).
- The validator refuses a collection that filters on an unindexed field (400).
  A second plugin claiming `reading` is refused (409).
- Sam (an editor) adds three links through Reading List's grant. A retry with
  the same attempt key returns the receipt and makes no second file.
- Refused: a reader (403), a plugin without the grant (403), an undeclared
  field (400) and an `ftp:` URL (400).
- "Mark read" is one `files.set`; a non-boolean value is refused.
- "Unread" returns two items, newest first, and the index and the scan give
  identical answers.
- The disk holds `Reading/The Rust Book.md` and two others. Each file is exact
  frontmatter Markdown (`id`, `kind`, then fields as YAML scalars).
- Host state holds four stamps (three creates and one set), all Sam through
  Reading List, and an audit row for every call, including the 4 refused by
  `authorize`.
- A disk edit that changes the title and sets `kind: notes` is stamped
  `outside` at the next write. Host state's kind wins, so `kind: reading` is
  written back and the title edit is kept.
- After a restart with the index deleted, the rebuilt index answers "Unread"
  the same. Installed plugins come back from host state.

`record_file_append_delete_and_forgery`:
- `Chats/Lisbon crew.md` gets records from Lee, Sam and Lee in host format
  (`**Lee** · 2026-11-20 21:04 ^k2m8…`).
- Sam cannot delete Lee's record (403). Lee's delete is in place, and the file
  length is unchanged. The words are gone from the file, and no file in the
  host-state folder ever held them.
- The ledger holds per-record stamps (anchor, person, op) in order, plus the
  delete stamp. The deleted record's earlier hash is set to NULL.
- **Forged append:** a header claiming "Lee" is written straight to disk. It
  is accepted as a pure append, stamped `outside`, and listed with author
  `outside`.
- **Changed header** (Sam's name changed to Lee in place): a conflict copy is
  written with the changed bytes. The header is regenerated from the stamp,
  the record file is restored byte for byte, and the record is listed as Sam.
- **Changed body** ("Friday works." changed to "Monday works."): a conflict
  copy is written. The record is marked `tampered` and still listed as Sam.
  The file cannot be restored, because host state has no copy of the text.
- Appends still work afterwards, and the next check finds nothing new.

## Counts

| What | Count |
| --- | --- |
| Lines of Rust, `src/` (with doc comments) | 2,224 (1,812 non-blank, non-comment) |
| Lines of Rust, tests | 648 |
| Tables | 9: eight in host state (`space`, `members`, `plugins`, `files`, `stamps`, `records`, `receipts`, `audit`), one in appdata (`fields`) |
| Call handlers | 7: `plugins.install`, `files.create`, `files.set`, `record.append`, `record.delete`, `record.list`, `query` |
| HTTP routes | 1 |

Flushes per call, counted with strace on dev-01 (`count-flushes.sh`; counts,
not timings):

| Call | fsync + fdatasync | Why |
| --- | ---: | --- |
| `record.append` | **2.0** | ledger commit (with the receipt and the audit row) + the record file's `fdatasync`; matches the design's "two flushes" |
| `files.create` | 3.0 | ledger commit + temp file + folder |
| `query` | 1.0 | **only the audit row**; the query itself writes nothing (finding 6) |
| `record.delete` | 4.0 | ledger commit + file + `wal_checkpoint(TRUNCATE)` to scrub old WAL pages |

`record.append`'s hot path reads: 1 member read, 1 head read
(`files`), 1 display-name read, 1 `stat`, 1 `fstat` before and after the
write. No read and no hash of the file. Before the audit row moved into the
handler's commit, an append cost 3 flushes.

## Reused, and what was missing

| Reused | For |
| --- | --- |
| `calternal_types::{AccountId, SpaceId, Error, ErrorCode, now_ms}` | Person and space IDs, the caller-safe error and its HTTP status, the clock |
| `calternal_fsio::beneath::Folder` (`open_sub`, `names`, `open_file`, `create_temp`, `TempFile::persist`/`replace`) | Every create and replace, resolved beneath the space root; never replaces an existing name on create |
| `calternal_fsio::names::{stem_from_title, unique_md_name, fold}` | File names from titles, clash-free by case and Unicode form |
| `calternal_fsio::beneath::plain_parts`, `create_dir_all_durable`, `TEMP_PREFIX` | Path checks, durable folders, temp names |
| rusqlite with the store's settings (WAL, `synchronous=FULL`, `secure_delete`) | Host state |

| Missing from the shared crates | What the spike did | Step 2 should |
| --- | --- | --- |
| A writable open beneath a folder (`Folder` only opens files for reading) | `record.append`/`delete` check the path as text and open with `O_NOFOLLOW`, which is a weaker fence than `RESOLVE_BENEATH` | add `Folder::open_rw` to `crates/fsio` |
| A shared frontmatter module (`server/notes.rs` and `content/rooms/external.rs` each have a private one) | `src/fm.rs`, 115 lines, line-wise edit that keeps unknown keys and comments | move one into a low crate (checklist item 1) |
| `FileId` and `PersonId` types | file IDs are bare UUIDv7 strings; `AccountId` stands for a person | add `FileId` to `crates/types`; decide whether a person is an account |

`crates/store`'s per-space writer was not reused, for the same reason the
records measurement gave: its `Commit` and schema belong to the content store.
The spike's per-space `Mutex<Space>` is that writer's shape: one lock per
space, held on a blocking thread, never on an async one.

## Choices made for unanswered questions (each in one place)

| Question | Option built | Where |
| --- | --- | --- |
| A1 / D5: what host state keeps | Beside the files (`state/<space>/host.db`); **hashes, offsets and stamps only**, no record text | `host.rs` module doc and schema; consequences marked `D5` in `records.rs` |
| Kind and ID fixed at create | `files` row is the authority; frontmatter `id:`/`kind:` are put back on the next write | `files.rs` (`set`), `query.rs` (`fields_of` ignores them) |
| A22: request IDs derived by the host | Hash of space, person, plugin, call name, exact input and the client's attempt key | `call.rs` `request_id` |
| Who may delete a record (D4) | Author only | `records.rs` `delete` |

## What the design got wrong or left unsaid

1. **D5 "hashes only" and A21 "the record file stays as it was" contradict
   each other.** A conflict copy saves the outside version, but restoring the
   original needs the original bytes. With hashes only the host has none.
   The spike found a partial way out:
   - **A header is derivable from its stamp** (display name, minute, anchor),
     so a changed header is put back exactly. The test proves this.
   - **A changed body can only be detected.** The record is marked `tampered`,
     and the file keeps the outside text.

   So the owner's real choice in D5 is between:
   - keeping text, which makes restore possible (the measurement's
     1.6× ledger);
   - "hashes only, and an outside edit of a body is a conflict copy plus a
     tampered mark; the chat shows the changed words flagged".

   §4.5 should say which. Two more consequences of hashes only:
   - The measurement's crash recovery (regenerating the tail from the
     ledger) needs the text, and the design never says so. With hashes
     only, an append must **roll back** on recovery: drop the stamp and the
     receipt when the bytes are missing; the caller never got a reply.
     A delete can roll forward, because `*deleted*` is deterministic. An
     edit in place needs a short-lived copy of the new body until its
     write is flushed.
   - None of this recovery is built.
2. **The cheap outside-change check misses edits ("racy clean").**
   - **The miss:** `(size, mtime, inode)` cannot see a same-length edit
     made within one tick of the kernel's coarse file clock after the
     host's own write. The first run of the record test showed this: the
     Sam-to-Lee header change went unnoticed. The test now waits 50 ms, as
     any real editor would.
   - **Fix (git's rule):** remember the time of the host's last write. When
     the file's mtime is not older than that write, treat the file as
     unverified on its next touch.
   - **Cost:** a chat with rapid appends would then re-verify, so the
     recheck should hash only the last record and the length, not the
     whole file. The design states the `fstat` check as sufficient; it is
     not.
3. **Hashes of records must be keyed.** A plain hash of "ok" or "yes" is
   reversible by guessing, so "hashes only" would leak short messages. The
   spike uses a per-space key in host state (`space` table). The key then
   has to be in backups, and rotating it means rehashing. The design does
   not mention this.
4. **Header repair depends on display names never changing.** The header
   carries the display name. Rename Sam to Samuel, and every old header no
   longer matches what the stamp regenerates, so the spike would mark
   untouched records `tampered`. Either the stamp keeps the name as written
   or the header shows something stable. §4.5 shows the name with no rule
   for renames.
5. **A22 has nothing to derive from for an interface gesture.**
   - **The gap:** "derived by the host" works for a run (run ID and
     sequence number). For a person's tap, the only stable thing across a
     retry is something the client sends. The spike takes an `x-attempt`
     header; without it, a retry is a new request (a second link).
   - **What can hold:** "authors never write request IDs" can hold, but
     the clients must send a per-gesture key, and the design should say
     so.
6. **Auditing every call costs a flush.**
   - **The cost:** with the audit row in its own commit, an append took 3
     flushes instead of 2.
   - **The fix built:** the row now joins the write's own commit, inserted
     next to the receipt. A refusal, or a write that fails before its
     commit, is audited on its own.
   - **What is left:** a read has nothing to join, so every query pays one
     fsync only for its audit row.
   - **To decide:** T21 asks for full retention of reads of others'
     writing. The design must choose: a less durable audit log
     (`synchronous=NORMAL` in its own database), batching, or auditing only
     reads by outbound-capable code.
7. **For a no-code plugin, nothing but the client says which plugin is
   acting.**
   - **The gap:** Reading List has no process. `files.create` runs because
     the client claims, in `x-plugin`, that the form it drew was Reading
     List's. Nothing on the server witnesses that.
   - **Why it matters:** the grant check is sound, since a plugin can do
     only what its own grant allows. But the stamp's `plugin` is only as
     true as the client.
   - **Same pattern elsewhere:** the reach set's "opened by the person's
     own gesture" (§4.3) also rests on the client. The design should name
     the client as the witness for both, or the session should carry the
     form's identity from the host.
8. **The design does not say where a kind's files go.**
   - **What the docs say:** §6.1 says "the kind's write grant". The
     section file's manifest has `grant: { folders: [Reading/] }` and also
     declarative operations, which A9 removes.
   - **What the spike did:** put `folder` on the kind.
   - **Still open:** what happens when a person moves a file out of that
     folder. Host state keeps its kind, so it still answers queries;
     whether it should is a question.
9. **A query's `sort` and `where` mix two storage classes.**
   - **The mix:** `sort: "-created"` sorts on a common field from host
     state, while `read` is frontmatter.
   - **What indexing needs:** filtering needs `index:` marks on fields.
     The integrated doc's Reading List table never marks them; the section
     file does. The spike follows the section file, and the validator
     refuses an unindexed `where`.
   - **Proposal:** the common fields (`id`, `kind`, `created`) should be
     listed once in the design, as the spike's `COMMON_FIELDS` is.
10. **Setting a field of a record file moves every record.**
    - **The cost:** a chat's title lives in its frontmatter, so changing
      it shifts every record's byte offset. That is an O(records) ledger
      update plus a whole-file rewrite. The spike refuses `files.set` on
      record files.
    - **The fix:** store record offsets relative to the end of the
      frontmatter. Then only `prefix_len` changes, and a title change is
      cheap.
11. **"A delete always shrinks the record" is false for short bodies.** The
    `*deleted*` marker is 9 bytes, so deleting "ok" grows the record and
    takes the rewrite path. The spike supports this but does not test it.
    A shorter marker, or padding the header's slack, would keep every
    delete in place.
12. **A conflict copy duplicates the file's ID.** The copy's frontmatter
    carries the original's `id:`. Host state does not list the copy, so the
    spike never indexes it. But a later import, or a re-scan that trusts
    frontmatter, would see two files with one ID. The host should rewrite
    or remove `id:` in the copy, or the design should say copies are never
    indexed.
13. **A forged outside record keeps its claimed name on disk.** The stamp
    says `outside`, and the host's views follow the stamp. But Obsidian, a
    file share or `cat` shows "**Lee** · …". "Renderers draw the author's
    name from the stamp" does not cover readers that are not ours. The host
    could rewrite an accepted outside record's header to `**outside**`.
    That is a host write, not an outside change, and it costs one in-place
    write.
14. **The call context's `space` is redundant inside a handler.** The
    handler already holds the locked space, so no handler reads
    `ctx.space`. It will matter only when a handler makes a call of its own
    (step 8). That is harmless, but it says the context's shape follows
    calls between plugins, not single calls.
15. **Ledger before file, with a name chosen first, has a gap.**
    - **The gap:** `files.create` must pick the file name before its commit
      in order to record the path. If an outside file takes that name
      between the check and the rename, the rename refuses, because it
      never replaces. Host state then has a row with no file.
    - **What was built:** nothing; the spike leaves this case unhandled.
    - **What should happen:** recovery should drop the row, or the name
      should be re-picked after the commit, with a second commit.

## What is stubbed or not built

- **Sign-in:** `x-person` is trusted. Membership and roles come from
  `init --member`.
- **Authority beyond the stub:** no sharing, groups, guests, consent,
  outbound-capable closure, reach set, picks or quotas. `authorize` checks
  membership, role (writers for writes, managers for install) and the
  plugin's kind grant. Every call goes through it and leaves an audit row.
- **Crash recovery** for any write (finding 1, finding 15).
- **Outside changes are seen only when a call touches the file.** There is
  no watcher, and the index learns of disk edits only on rebuild (at start).
  `from: "scan"` always sees them.
- **Not built from the design:** collections as files (they are resolved
  from the declaration), live windows, grouping, backlinks, search,
  history reads, undo, move, trash, `record.edit`, and the 48-hour edit
  window.
- **Retention:** audit rows use the receipts' 7-day retention. The design
  asks for per-plugin retention (T21).
- **Concurrency:** one lock per space, untested with concurrent calls or
  several spaces.
- **Performance:** nothing was measured, as asked. The flush counts above
  are counts.

## What step 2 should be

Keep this binary as the test-mode host (§4.8) and grow it, rather than
starting again in the product crates:

1. **Decide D5 with finding 1 in hand.** If text is kept, build the
   measurement's roll-forward recovery. If not, build roll-back recovery
   for append and `files.*`, and the tampered display. Add the racy-clean
   rule (finding 2) and record offsets relative to the frontmatter
   (finding 10).
2. **Questions, as the plan's step 2 says:**
   - a watcher that feeds the index;
   - collections as files under the plugin's folder;
   - a live window over one query that is pushed when its answer changes;
   - a test that edits a file by hand and sees the window update.
3. **Move the shared pieces down:** `Folder::open_rw` and a shared
   frontmatter module into `crates/fsio` (or a small crate under it), and
   `FileId` into `crates/types`.
4. **Settle the audit cost** (finding 6) before step 3 builds the real
   check on top of it.

## Simplification opportunities

- **`stamps` and `records` overlap.** A record's current hash and author
  are in both. `records` could hold only position and state, with author
  and hash read from its latest stamp. That is one place per fact, at the
  cost of a join on `record.list`.
- **Merge `files.hash` and `files.prefix_len`.** For plain files the
  "prefix" is the whole file; the spike keeps both columns to share one
  table between plain and record kinds.
- **Merge the two verify paths.** `files.set` checks outside edits by
  hashing the whole file, and `records::verify` checks per record. One
  verify that treats a plain file as "a prefix with no records" would
  remove the second path.
- **`record.list` could be `query` over records.** Once the index keeps
  records (step 2), the separate handler goes, leaving six.
- **The `Need` enum (5 cases) could become 2.** "Which kind does this call
  touch, from `input.kind` or `input.file`" plus "read or write" covers
  all five cases.
- **Use the measurement's own record code.** `perf/spikes/record-files`
  has a header parser and anchors of the same shape. This spike wrote its
  own (~60 lines), and step 2 should use one.
