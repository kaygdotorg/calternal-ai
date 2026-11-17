---
title: "calternal-machine"
---

<!-- Generated from: crates/machine/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

The machine role of the `calternal` binary.

A machine (the user's own Mac or Linux box, or a cloud VM the operator enrolled)
connects out to the server, reports its capacity and runs each assigned thread
in a per-thread sandbox it starts itself. The server never connects to a
machine and never starts containers or VMs.

- [`protocol`]: the wire types shared with the server role.
- [`identity`]: the machine key and its 0600 credentials file.
- [`enroll`]: exchange a one-time code for a machine identity.
- [`connection`]: the outbound session, long-poll loop and reconnect backoff.
- [`runner`]: applies server messages to sandboxes and streams exec output.
- [`sandbox`]: the sandbox interface with Podman and direct backends.
- [`cli`]: `calternal machine enroll | run | status | discard`.

[Full Rust API reference](/docs/dev/rust/calternal_machine/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
