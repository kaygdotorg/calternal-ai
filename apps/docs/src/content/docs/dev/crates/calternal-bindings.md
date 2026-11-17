---
title: "calternal-bindings"
---

<!-- Generated from: crates/bindings/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Async native operation boundary plus optional contract validation helpers.
Credentials are supplied by trusted platform sign-in/secret storage and are never
formatted in diagnostics. Explicit cancellation stops local waiting, not server
effects. Retain NativeRequest after an uncertain mutation for safe retry.

No editor input, composition, caret, layout, animation or per-frame work crosses
this boundary. Invoke background operations with coalesced edits and batched results;
JSON helpers copy payloads and must stay off native UI hot paths.

[Full Rust API reference](/docs/dev/rust/calternal_bindings/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
