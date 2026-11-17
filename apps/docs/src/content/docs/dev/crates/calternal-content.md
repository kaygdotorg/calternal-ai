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
durably records its journal event/receipt, then checked fsio placement makes
the current file durable before acknowledgement. Startup replays unfinished
intents. A changed/deleted/foreign file blocks writes and is preserved.
This preserves the preview materialization contract; complete file-only
recovery, external-editor reconciliation and tier moves remain separate work.

Callers must retain installation root locks and enforce current authority.
The cache is capped at 64 active spaces with a configurable idle deadline;
a background sweep runs every 30 seconds even with no requests. The journal
bridge has a bounded 32-command queue. SQL and file work belong on blocking
host threads, never on async executor workers.

[Full Rust API reference](/docs/dev/rust/calternal_content/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
