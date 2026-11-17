---
title: "calternal-store"
---

<!-- Generated from: crates/store/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

# Per-space storage

Each space has its own SQLite database, `index.db`, holding the
change journal and idempotency records. Independent writers avoid a global
application write queue; SQLite and host resource contention remain possible.
The journal and receipts are durable state, not rebuildable file indexes.

All writes for one space go through a single writer thread, in arrival
order, as small transactions. Nothing holds a lock while waiting on the
disk: callers send a message and wait for the reply. Reads use their own
short-lived connections and never wait for the writer.

[Full Rust API reference](/docs/dev/rust/calternal_store/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
