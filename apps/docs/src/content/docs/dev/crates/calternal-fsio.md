---
title: "calternal-fsio"
---

<!-- Generated from: crates/fsio/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

# Safe file writes

Calternal writes note content through [`write_unless_changed`]: flush
an adjacent temporary image, then place it exclusively or exchange it
with the checked existing file. [`rename_safely`] moves notes without
replacing a destination. Unsupported atomic operations fail safely.
[`write_atomic`] replaces application-owned metadata and cache files.
A crash leaves a whole image rather than an empty or half-written file.

These are host filesystem mechanisms, not tenant authorization or a sandbox.
Callers select authorized roots and retain their guards for all I/O.

Also here:
- [`write_unless_changed`]: the same, but it never replaces a version
  another app wrote since the caller last looked (note saves use it,
  in two halves: [`Prepared`]).
- [`DataLock`]: only one Calternal process may own a data folder.
- [`names`]: file names are normalised (NFC) and compared ignoring case
  and normalisation, so `Note.md` and `note.md` never become two notes.
- [`write_atomic_from`]: [`write_atomic`] for a stream (an upload). Big
  writes go to the disk in steps, so they never make a note save wait
  behind them ([`paced`]).
- [`beneath`]: opening and writing files below a folder (a space) by a
  path a person gave, without any way to reach outside it.

[Full Rust API reference](/docs/dev/rust/calternal_fsio/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
