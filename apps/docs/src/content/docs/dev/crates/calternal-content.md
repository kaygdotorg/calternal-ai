---
title: "calternal-content"
---

<!-- Generated from: crates/content/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Generic per-space item storage shared by Notes and other plugins.

Metadata, immutable history and publication intents live exclusively in
`<appdata>/spaces/<uuid>/content.db`; current bytes live in the selected SSD
space tree. The global catalog contains routing identifiers only. The
inherited store owns each space's journal and account-qualified receipts.

A FULL content transaction records a head and outbox intent. Publication
durably records its journal event/receipt, before acknowledgement. The existing lifecycle worker then uses checked fsio
placement within ADR 0062's one-second maximum; overdue placement blocks new
writes. First use replays unfinished intents before exposing that space. A changed/deleted/foreign file blocks writes and is preserved.
This preserves the preview materialization contract; complete file-only
recovery, external-editor reconciliation and tier moves remain separate work.

Callers must retain installation root locks and enforce current authority.
SQL and file work belong on blocking host threads, never on async executor workers.

Locks: work in one space never waits for another space's work, except that a
new item's first catalog route takes the one catalog writer (below). The space
cache map lock is held only to find, insert or remove an entry (no I/O: an
evicted space is closed after the map lock is released). Each space has its own
lock, held for that space's whole operation, including its SQLite, journal and
file fsyncs; it serializes work within the space only. Catalog reads use a
[`ReadPool`]; the one catalog writer connection is locked only when an item is
first routed (a create), for one insert. The cache keeps at most 64 open spaces
and closes idle ones after a configurable deadline; a background thread sweeps
every 30 seconds even with no requests.

[Full Rust API reference](/docs/dev/rust/calternal_content/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
