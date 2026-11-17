---
title: "calternal-client"
---

<!-- Generated from: crates/client/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Async HTTP effects adapter. The pure contract and pending identities live in core.
Session credentials come from the trusted passkey/OIDC flow; platform adapters own
secret storage and supply the credential without logging it. A 401 requires sign-in.
Dropping a request future or cancelling a token stops waiting locally; it does not
undo server commits. Retain pending identities after an uncertain mutation outcome.

UniFFI is a background operation boundary: editor composition, selection, keystrokes,
layout, animation and per-frame rendering stay native. Submit coalesced transactions
and consume batched results; never copy an entire document per keystroke. Journal
following uses explicit bookmark acceptance; see `SYNC.md`.

[Full Rust API reference](/docs/dev/rust/calternal_client/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
