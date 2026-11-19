# Search and palette lookups

The palette calls `search.query` while typing. Send `query` and optionally a
`space_id` UUID, `kinds`, a page `limit` from 1 to 50, and a `cursor`.
Omit the space filter to search every space where the caller currently has
`space.read`. An unreadable or absent space contributes an empty result.
The host checks current grants before touching content and before delivering
a result. A thread credential remains scoped to its own space and to the
actions granted by its authority.

Each hit carries `address`, `kind`, `space_id`, `title` and an `excerpt`
array of plain-text segments. Render segment text literally and apply emphasis
where `matched` is true. Clients may ask for the next page using the returned
`next_cursor` with the same query and filters. Cursors are signed for the
account, operation and input. Results reflect current data on each page.
Search puts title matches first, then matches that use the body. UUID breaks
ties. Up to twelve words from a 512-byte query match token prefixes. When a single word has no exact-prefix
matches, the index can find titles within one edit, including adjacent swaps.
An empty query returns no hits.

Use `kinds: ["threads"]` for message search. Each hit keeps its thread
address and adds `message_id` and `message_at`. Messages match literal words
or an adjacent phrase between double quotes. They are ordered by saved UTC
instant descending, then by message UUID descending. An unfiltered search
places message matches after item title and body matches. Open a specific
record with `threads.get` and `message_id`, or `calternal threads get ID
--message-id MESSAGE_ID`. The operation checks the existing thread authority,
uses the exact current revision, and returns one framed message. A removed
record or a stale locator cannot open a different message.

| Operation | Input and result |
| --- | --- |
| `threads.lookup` | Literal title `prefix` across readable spaces, optional `space_id`, `limit` and `cursor`. |
| `items.lookup` | Same title prefix over every enabled, opted-in kind. Lookup results are ordered by folded title and UUID. |
| `people.lookup` | Current `space_id` and optional display-name `prefix`. Returns up to 50 direct account and group grants, excluding disabled accounts. Group member identities stay private. |
| `skills.list` | Current `space_id` and optional skill-name `prefix`. Returns up to 50 names and one-line descriptions from `AI/Skills/<name>/SKILL.md`. |
| `plugins.manifests` | Current `space_id`. Returns enabled contributions as `manifests: [{plugin, version, commands, settings_sections}]`. Commands supply the web palette's `id`, `label`, `group`, optional `words` and either `go` or `op`. Settings sections also appear as navigation commands in the Settings group. |

`threads.lookup` reads current titles from the existing content-head index.
Each enabled readable space takes a plugin activation read and one indexed
title range query, with no thread body reads. The picker includes renamed
titles as soon as their heads are committed.

Skill directories can be placed in userdata. The server reads up to 8 KiB
of each candidate for the frontmatter name and single-line description and
skips symlinks. The declared name must match the directory as specified by
[Agent Skills](https://agentskills.io/specification).
One request examines up to 512 directory entries. A missing directory yields
an empty list. The preview ships no per-space skill files or skill-write
operation.

## Index

A manifest opts a Markdown kind in with `searchable: true`. Notes and threads
opt in. The live catalog caches the search
projection per immutable snapshot. Each request reuses its one catalog read
from admission across spaces and match passes. The worker pins one snapshot
per bounded sweep and shuts down when its owner closes.

Each probe reads effective plugin activation from current space controls.
Disabled kinds, packages scoped to another space, and unreadable spaces
contribute zero hits and zero coverage information. The live catalog adds
zero SQL or file reads. A warm Notes probe uses six SQLite statements per
space and match pass. A message probe checks readiness and resolves terms
before reading candidate blocks. It reuses the current revision lookup for
hits in the same thread and reads one bounded byte range per candidate it
verifies. The request normally uses two authority reads to confirm the
same spaces before delivery.

The derived per-space `search-v2.db` opens lazily. A separate maintenance
thread scans at most 128 heads per turn and cycles through spaces. It runs
outside foreground read and write admission. Its four warm writer handles
and 128 idle reader sets are bounded. Prefix lookups use a B-tree range.
Notes text and typo matching use SQLite FTS5 and title character pairs,
ported from ADR 0031. The independent message index uses 16 KiB SQLite pages,
a 32 MiB writer cache, and compressed 16,384-position blocks. The worker writes
each sorted run in block and term order. SQLite builds the chronological and
UUID secondary indexes after the bulk load. The UUID index starts with the
thread slot, so it also locates all of a thread's messages on replacement
or deletion. Each posting records its newest
time and UUID. A shared lower bound per block lets continuations skip newer blocks.

A save performs zero search SQL statements, userdata reads, parses, or
flushes. The worker tails the durable journal position, uses the saved
title and validated Markdown bytes from the already staged version, and
commits the projection with its watermark in one transaction. The worker
tokenizes text for search. A worker crash replays from the surviving
watermark. The healthy lag target is one second with work admitted and
storage responsive. Readers see the previous published generation while
the watermark is behind and receive `partial: true`. A missing or invalid
generation or an unpublished authoritative intent yields an empty partial page
until recovery catches up.

On first use, a fresh generation scans retained UUID heads that have a durable
version. It opens each eligible Markdown file beneath the space and checks
that it is regular, within the full-file byte limit, and has the durable
hash before extracting the searchable prefix. Pending placement, a missing
or unreadable file, and a size or hash mismatch use the exact staged version
from the same bounded read snapshot. The journal catches changes during the
scan before the worker publishes readiness. A change to the release version,
schema generation, searchable kinds, or journal epoch invalidates the
projection.
The scan opens existing userdata directories only. The content writer
provisions missing directories with fsio's owner-only mode.
Foreground Notes list, ID reads and saves keep using existing head metadata
and the journal during rebuild. This covers the Notes search projection.
Other indexes use their existing lifecycle.

Each space has at most 250,000 indexed head slots. Titles cap at 256 Unicode
scalars, bodies at 8,192 scalars, and the worker reads only the first 64 KiB
of each staged body for keyword indexing. The frontmatter delimiter must be
within those bytes. A space beyond the slot ceiling sets `partial: true`.

The Notes and message writers each cap their WAL at 64 MiB per space. At 48 MiB the
background worker attempts a truncated checkpoint before another turn. A
reader that pins the WAL can delay search catch-up. Foreground notes and
journal writes keep their existing admission and durability. An oversized
uncommitted index write fails and leaves the watermark unchanged.

Message search has a separate file and journal watermark. The worker reads
verified files once during a rebuild and loads the exact staged version on
fallback. It replaces a changed thread's postings and advances the projection
watermark in the same transaction. The healthy lag target is one second.
Normal thread list, full thread open, and saves work during the rebuild. Search
returns `partial: true` until the generation is ready. A cold phrase request
waits for verification reads and then returns the verified page. The results
stay partial if a request reaches the verification work limit.

The message index stores a locator per message, a dictionary for live terms,
and a compressed posting for each distinct term in a block. Bytes grow in
proportion to indexed transcript bytes. The maintenance worker reserves 1 GiB
of appdata free space, space for unpublished SQLite rollback work, and a 64 MiB
message WAL. It pauses publication if the reserve cannot be met, leaves the
watermark unchanged, and reports partial coverage. SQLite reuses pages freed
by edits and deletes. The allocated file can retain the generation's previous
high-water size until a rebuild. Authoritative saves continue when the
derived index pauses.

`search-messages-v1.all.db` is the all-time partition of a space's message index.
A future partition can replace `all` with a UTC interval key using the same schema and generation.
Readers can merge those intervals by the existing timestamp and message UUID cursor.

## Pending product decisions

- Ranking within each match tier could use UUID order, cross-space relevance
  features, or the active app as a bias. UUIDs give stable keysets. Relevance
  needs comparable scores across spaces. App bias needs an additional request field.
- Recents could remain client-local, become per-account server history with
  retention, or use a decaying frecency score. Device-local history is cheap.
  Server history syncs across devices and needs access rechecks, retention and
  update rules. Frecency needs interaction events and ranking tests.
- Structured `kinds` and `space_id` filters are available. Typed filter tokens
  could stay client-side or become a shared grammar for all clients and the CLI.
  A shared grammar needs quoting, ambiguity and interoperability rules.
