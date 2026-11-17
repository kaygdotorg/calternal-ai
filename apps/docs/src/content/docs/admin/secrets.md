---
title: "Secrets"
---

<!-- Generated from: crates/secrets/README.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Implementation reference with a known source mismatch: this README describes an older master key layout. The accepted plugin API decision specifies one key file per owner and no unlock step. Do not use this page as a production setup procedure.


Secrets encrypted at rest. Plugins and agents get an opaque handle; the value is released
only through an audited `use_value` call that names one consumer.

Ported from the existing app's invite key (`crates/auth/src/invite_key.rs` on
`origin/dev`): key file in `system/` with mode 0600 published atomically by hard link,
refusal of a missing key once encrypted rows exist, AES-256-GCM through openssl, keyed
BLAKE3 for lookups, generic refusals.

## Layout

| Path | Holds |
| --- | --- |
| `appdata/system/secret.key` | 32-byte master key, mode 0600 |
| `appdata/system/secrets.db` | Key fingerprint; which owners have secrets. No values |
| `appdata/accounts/<account-id>/secrets.db` | One account's sealed values, pending requests, audit trail |
| `appdata/spaces/<space-id>/secrets.db` | One space's sealed values, pending requests, audit trail |

Each value is sealed with AES-256-GCM under a key derived from the master key. The
associated data is (owner kind, owner id, name, version), so a row copied to another
owner, renamed or given another version fails to decrypt. Handle tokens are a keyed
BLAKE3 MAC of the same four fields (`sh-` + 40 hex digits), matching the plugin host's
identifier rule. Encryption, handle MACs and the key fingerprint use separate derived keys.

## Guarantees and their tests (`src/tests.rs`)

- Crash during put or rotate (process abort just before commit): old state intact.
- Wrong owner, copied database, renamed row, tampered ciphertext: refused as not found.
- Missing key with existing secrets: refused, no replacement key minted. Wrong key or
  wrong mode: refused.
- Accounts and spaces isolated, including an account and a space with the same UUID.
- Leak scan: a known value is absent from every file under appdata (databases, WAL, key),
  from audit, metadata, pending requests, handles, errors and `Debug` output.

## OpenBao mapping (later backend)

| Here | OpenBao |
| --- | --- |
| Master key, `secret.key` | Bao's seal (unseal keys or auto-unseal KMS); no key file on the server |
| Owner database | KV v2 path prefix `calternal/accounts/<id>/` or `calternal/spaces/<id>/` |
| Secret name | KV v2 key under that prefix |
| Version, `rotate` | KV v2 version; rotate = write new version, then `destroy` older ones |
| AEAD owner binding | ACL policy templated per owner path (isolation by policy, not by cipher) |
| `Metadata` label | KV v2 `custom_metadata` |
| `delete` | `metadata delete` (all versions) |
| Handle token | Unchanged: host-side MAC; resolving it maps to a KV path and version |
| `use_value` | Read with a short-lived token limited to that one path, after our audit row |
| Audit trail | Bao audit device (who read which path) plus our own audit row (which consumer) |
| Pending requests | Stay in the host database: they hold no value |

## Bounds

Per owner: 1,000 secrets, 64 pending requests, the newest 10,000 audit rows (older rows
are pruned on each audited write). Values are at most 64 KiB, labels 120 characters,
reasons 500. Constants: `LIMITS` and `SecretValue::MAX`.

## Costs

No installation-wide lock or connection: each call opens only its owner's database (one
`open` and a `user_version` read; the schema is created once per file). `use_value` is one
indexed read, one AES-GCM open and one commit (one WAL fsync). Writes are one commit; an
owner's first write also records it in `system/secrets.db` and creates its directory.
Reads do not fsync. The `system/secrets.db` registry is opened only at startup and on an
owner's first write.

## Reused primitives

`calternal_auth::private_fs` for 0600 files and 0700 directories (imported once in
`lib.rs`; it moves to `calternal-fsio`), `calternal_fsio::sync_dir`, the invite key's
hard-link publication, `rusqlite` with WAL and `synchronous=FULL` as in the other stores,
and `AccountId`/`SpaceId` from `calternal-auth`.
