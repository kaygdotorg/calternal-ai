---
title: "calternal-roots"
---

<!-- Generated from: crates/roots/src/lib.rs. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Three-role storage startup guard, ported from Calternal's storage startup.

The caller pins an installation UUID in configuration outside appdata and
explicitly opts into first setup. All roles and identities are checked before
any lock sentinel or store is created. Existing installations require a
nonempty regular `system/auth.db`; this is not SQLite integrity validation or
FO1 reconstruction. This library never creates a database.

On Linux, Android and Apple, every path component is opened with `NOFOLLOW`
and directories are retained by descriptor. Markers, locks and setup writes
resolve through those descriptors, so path symlink substitutions cannot redirect
writes. Lexically overlapping paths and equal device/inode roots are refused.
Bind-mount ancestry under different path spellings is not detected. These checks do not
prove separate devices, mount IDs, SSD/HDD hardware, mount continuity, or
provenance of copied markers/auth files. No device separation is enforced:
userdata-ssd and appdata are allowed on the same filesystem. An unavailable
mount is detected through missing roots/markers/identity, not a mount-table
check. Hostile concurrent mutation of real directories or marker bytes is
outside the cooperative root-lock contract; this is not a tenant sandbox.

Incomplete first setup is preserved and refused on subsequent opens; there
is no automatic repair, adoption of legacy single-root storage, or automatic re-pair. Deliberate offline re-pair requires explicit confirmation.

[Full Rust API reference](/docs/dev/rust/calternal_roots/index.html). Generated with stable `cargo doc --workspace --no-deps`; dependency documentation is omitted.
