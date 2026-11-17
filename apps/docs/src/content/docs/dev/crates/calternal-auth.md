---
title: "calternal-auth"
---

<!-- Generated from: crates/auth/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Protected identity storage primitives ported from Calternal's auth crate.

This crate preserves the v1–15 legacy schema, including sharing tables.
It supplies stored space authority, but no login endpoints or recovery coordinator.
Callers must validate and lock installation roots before opening the DB;
opening a missing path creates it and is not safe recovery of lost state.

[Full Rust API reference](/docs/dev/rust/calternal_auth/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
