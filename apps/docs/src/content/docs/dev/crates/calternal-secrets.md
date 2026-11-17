---
title: "calternal-secrets"
---

<!-- Generated from: crates/secrets/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Secret storage: values encrypted at rest, released only through audited use.

A secret belongs to exactly one [`Owner`]: an account (personal credentials) or a space. Plugins and agents never receive a value; they get
an opaque [`HandleToken`] that names (owner, name, version). The value leaves the
store only through [`SecretStore::use_value`], which names the [`Consumer`] (one
sandbox environment variable for one thread, or one outbound request header) and
writes an audit row first. Values never appear in errors, logs, audit rows,
metadata or handles; [`SecretValue`] has no `Display`, `Serialize` or revealing
`Debug`, and wipes its buffer on drop.

Authorization (who may put, list, use or revoke for an owner) is the host's job.
The store enforces only that a value decrypts for the owner, name and version it
was written for. [`LocalStore`] is the on-disk backend; the trait is shaped so an
OpenBao backend can replace it (see `README.md`).

[Full Rust API reference](/docs/dev/rust/calternal_secrets/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
