---
title: "calternal-cli"
---

<!-- Generated from: crates/cli/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Shared plumbing of the Calternal command-line programs.

[`output`] is the one place that decides how any `cl` or `calternal` command
prints: aligned tables and labelled blocks for people, one JSON document with
`--json`, plain-language errors, and exit codes. Command layers build a
[`output::Reply`] or a [`output::CliError`] and hand it to an [`output::Output`];
they never print results themselves. The reference for users is `docs/cli.md`.

[Full Rust API reference](/docs/dev/rust/calternal_cli/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
