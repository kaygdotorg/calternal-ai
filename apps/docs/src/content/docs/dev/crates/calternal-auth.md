---
title: "calternal-auth"
---

<!-- Generated from: crates/auth/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Protected identity storage primitives ported from Calternal's auth crate.

The shared SQLite migration runner applies changes to the pinned
`auth.db` baseline in `schema.sql`.
It supplies protected account and space authority to the host HTTP adapter.
Callers must validate and lock installation roots before opening the DB;
opening a missing path creates it and is not safe recovery of lost state.

[Full Rust API reference](/docs/dev/rust/calternal_auth/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
