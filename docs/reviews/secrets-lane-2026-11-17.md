# Secrets lane: review gate, 2026-11-17

Branch `lane/secrets`, issue #9, thread thr_cbee7mbfjx. Crate `crates/secrets`
(`calternal-secrets`), server module `crates/server/src/secrets.rs`, contract entries in
`crates/core/src/secrets.rs`.

| # | Item | How it is met |
| --- | --- | --- |
| 1 | No copied shared types | `AccountId`, `SpaceId` from `calternal-auth`; `Capability`, `Policy`, `HostError` from the server. Own `calternal_secrets::Error` is a closed refusal enum (no strings), mapped to `HostError` in one function; no existing type has `KeyMissing`/`KeyUnusable`. One private `now_ms` copy, because auth's is crate-private: move it to the shared types crate. |
| 2 | Dependencies downward | secrets → auth (IDs, `private_fs`), fsio. `private_fs` is imported once (`lib.rs`), a one-line change when it moves to fsio. Auth's entropy helper was dropped for `getrandom` (same version auth uses). |
| 3 | Nothing global | No installation-wide lock or connection in the store; the owner registry is opened only at startup and on an owner's first write. The server takes its store lock only to clone an `Arc`, no I/O under it. |
| 4 | No blocking on async | All store calls run in `spawn_blocking`; session checks use auth's async API. |
| 5 | Read/flush counts | `crates/secrets/src/local.rs` module docs and README "Costs": `use_value` 1 open, 1 read, 1 fsync. |
| 6 | Constant work cached | Schema applied once per file (`user_version`); catalogue entries live in core's existing catalogue. |
| 7 | Bounds | `LIMITS`: 1,000 secrets, 64 pending requests, 10,000 audit rows per owner; 64 KiB values. Retention of audit by count, not time: question on #9. |
| 8 | No compatibility code | None. |
| 9 | Spaces by UUID | `space_id` must parse as a UUID; development aliases are not accepted. |
| 10 | One place per fact | Space capability per operation lives only in its `POLICIES` row; the provide capability is one constant. |
| 11 | No unwired code | Wired: request/list/revoke/provide, put via provide, delete, cancel. Not yet called outside tests: `plugin_handle`/`token` (plugin broker, plugin-host lane), `resolve`/`use_value` (machine-role sandbox and outbound requests), `rotate`/`audit` (secrets management screen). Named on #9. |
| 12 | Reuse | `private_fs`, `fsio::sync_dir`, the invite key's hard-link publication, rusqlite WAL pattern. |
| 13 | Docs gate | `calternal-secrets` added to `scripts/check-docs.sh`; passes. |
| 14 | Docs match | `crates/secrets/README.md` (layout, bounds, costs, OpenBao mapping). |
| 15 | fmt, file size | Files of this lane are rustfmt-clean (the rest of the workspace already has drift on dev); largest file 564 lines. |
