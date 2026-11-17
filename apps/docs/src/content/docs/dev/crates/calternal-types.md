---
title: "calternal-types"
---

<!-- Generated from: crates/types/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Types every Calternal-AI crate shares: typed identifiers, the acting
[`Actor`], the caller-safe error, the closed host capability vocabulary and the
millisecond clock and shared worker-frame bound.

The crate does no I/O, holds no locks and depends only on `serde`,
`serde_json` and `uuid` (plus `schemars`/`ts-rs` behind the `contract`
feature for the wire contract exporter). Crates that need an identifier or
a caller-facing error import it from here instead of defining their own.

[Full Rust API reference](/docs/dev/rust/calternal_types/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
