---
title: "calternal-plugin-host"
---

<!-- Generated from: crates/plugin-host/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Data-only plugin registry and bounded worker-v2 session engine.

This library never starts a process or grants authority. An execution adapter must
isolate each (plugin, space) worker, deny ambient filesystem/network/credentials,
enforce CPU/memory/deadline limits and kill unresponsive cancelled workers.
Host brokers must recheck current authority at every effect and delivery boundary.

[Full Rust API reference](/docs/dev/rust/calternal_plugin_host/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
