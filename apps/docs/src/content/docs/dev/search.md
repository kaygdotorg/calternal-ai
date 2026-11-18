---
title: "Search and palette lookups"
---

<!-- Generated from: docs/search.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

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

| Operation | Input and result |
| --- | --- |
| `threads.lookup` | Literal title `prefix` across readable spaces, optional `space_id`, `limit` and `cursor`. |
| `items.lookup` | Same title prefix over every enabled, opted-in kind. Lookup results are ordered by folded title and UUID. |
| `people.lookup` | Current `space_id` and optional display-name `prefix`. Returns up to 50 direct account and group grants, excluding disabled accounts. Group member identities stay private. |
| `skills.list` | Current `space_id` and optional skill-name `prefix`. Returns up to 50 names and one-line descriptions from `AI/Skills/<name>/SKILL.md`. |
| `plugins.manifests` | Current `space_id`. Returns enabled contributions as `manifests: [{plugin, version, commands, settings_sections}]`. Commands supply the web palette's `id`, `label`, `group`, optional `words` and either `go` or `op`. Settings sections also appear as navigation commands in the Settings group. |

Skill directories can be placed in userdata. The server reads up to 8 KiB
of each candidate for the frontmatter name and single-line description and
skips symlinks. The declared name must match the directory as specified by
[Agent Skills](https://agentskills.io/specification).
One request examines up to 512 directory entries. A missing directory yields
an empty list. The preview ships no per-space skill files or skill-write
operation.

## Index

A manifest opts a Markdown kind in with `searchable: true`. Notes and
Threads opt in. The space's effective plugin activation determines which
kinds appear. The index uses the existing per-space `content.db`, current
revision and transaction. Search fields come from the saved heading metadata
and a bounded slice of the already-validated Markdown bytes after frontmatter.
Search uses SQLite FTS5 and title character pairs, adapted from the original
Calternal search in ADR 0031. Prefix lookups use a B-tree range. Up to 128 read-only connections are
retained for active space queries, with a 256 KiB SQLite page-cache target per
reader. Read-only queries use one SQL statement per selected space in the
ordinary case. A query may need a writer round trip to recover an unretired
intent or build a missing projection.

The bound is 250,000 content head slots per space, with a title of at most
256 Unicode scalars and the first 8,192 body scalars per searchable document.
The frontmatter delimiter must be in the first 64 KiB. `partial` is true
if an enabled selected kind has a head above the space's slot ceiling. This
does not change which documents can be saved. SQLite pages and WAL remain subject
to the space's database and checkpoint lifecycle.

Saving a revision updates the index in the same SQL transaction. A save adds
bounded text projection and index page writes, with zero extra userdata reads,
Markdown parses, SQL transactions or durability flushes. A crash after commit
preserves both the revision and its projection. Dropping the derived index
tables offline causes the next lazy open to reconstruct them from the current
stored revisions. Userdata remains the source of the saved Markdown.

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
