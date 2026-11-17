---
title: "Secrets"
---

<!-- Generated from: crates/secrets/README.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Per-owner key storage and recovery reference. Operator backup retention and key backup handling require a production runbook.


Secrets encrypted at rest. Plugins and agents get an opaque handle; the value is released
only through an audited `use_value` call that names one consumer.

Ported from the existing app's invite key (`crates/auth/src/invite_key.rs` on
`origin/dev`): one key file in each owner’s appdata folder with mode 0600 published atomically by exclusive rename,
refusal of a missing key once encrypted rows exist, AES-256-GCM through openssl, keyed
BLAKE3 for lookups, generic refusals.

## Layout

| Path | Holds |
| --- | --- |
| `appdata/accounts/<account-id>/secret.key` | Account key, 32 bytes, mode 0600 |
| `appdata/spaces/<space-id>/secret.key` | Space key, 32 bytes, mode 0600 |
| `appdata/system/auth.db`, `owner_secret_keys` | Protected fingerprints and erasing tombstones; never key bytes |
| `appdata/accounts/<account-id>/secrets.db` | One account's sealed values, pending requests, audit trail |
| `appdata/spaces/<space-id>/secrets.db` | One space's sealed values, pending requests, audit trail |

Each value is sealed with AES-256-GCM under a key derived from its owner’s key. The
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
| Owner key, `secret.key` | Bao's seal (unseal keys or auto-unseal KMS); no key file on the server |
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

Per owner: 1,000 secrets, 64 pending requests (expire after 15 minutes), the newest 10,000 audit rows (older rows
are pruned on each audited write). Values are at most 64 KiB, labels 120 characters,
reasons 500. Constants: `LIMITS` and `SecretValue::MAX`.

## Costs

Ordinary calls open only the selected owner's database and read its private key;
no key or plaintext is cached. Reads perform one schema-version read and one combined fingerprint/erasing-state
lookup, followed by their own query.
They perform no fsync. `use_value` loads the key under an IMMEDIATE owner transaction,
reads one ciphertext, authenticates it and commits its audit row (one WAL fsync).
Writes use one transaction; first use additionally initializes the owner schema and
publishes a key (file fsync and directory fsync). Key creation and erasure use the existing
auth worker/recovery barrier; ordinary use does not call it. Schema SQL is a constant and
is applied only when a database is first created.

## Recovery and erasure

Key creation publishes a synced private file by exclusive rename, then publishes its
fingerprint through the auth recovery barrier, then commits the owner-local fingerprint.
Only afterward can an encrypted row be committed. A crash before fingerprint publication
leaves an unused key that a retry adopts. Missing or mismatched recorded keys refuse
startup and use. No key bytes enter auth.db, userdata or the recovery log.

`configure_keys` attaches the server adapter before request admission. Standalone crate
users must attach `KeyLifecycle` themselves when protected recovery is required.
`erase_owner` publishes an irreversible recovery tombstone before unlinking the key,
fsyncs its directory, then commits a local tombstone. The owner database's IMMEDIATE
transaction serializes erasure against value release; it spans the fsync and recovery
round trip for that reason. A crash after unlink refuses use; startup finishes any
recorded erasure before admitting requests. Space deletion sets the tombstone in its
protected auth transaction and invokes erasure before acknowledging the operation.
The explicit erase API also supports account deletion; there is no account-delete route
in this preview. Old handles and old database copies cannot decrypt without the key.
Backups containing key files still contain keys: erasure cannot erase a separately retained
copy of a key.

One fingerprint/tombstone is retained per owner that used secrets, including deleted
owners. Tombstones cannot be pruned safely while old ciphertext backups might be restored;
operator backup-retention policy must settle a finite prune horizon before production.

## Authority

Personal secrets are visible only to their account, including when another account is a
space Manager or Owner. Space listing, provide, revoke and cancellation require
`secrets.manage`, granted to Manager and Owner by default. Handle requests still require
`plugins.use` and the plugin manifest's declared secret. Plugin secrets are stored under
the enabling space. Rotation replaces ciphertext and invalidates every older handle
immediately. There is no passkey-derived key and no unlock step.

## Simplification opportunities

Keep erasure as one lifecycle path for future account deletion. Do not add a second key
registry or a key cache; protected auth metadata and the owner-local fingerprint have
distinct restore and decryption checks.

## Reused primitives

`calternal_fsio::private` for 0600 files and 0700 directories, `calternal_fsio::sync_dir`,
the invite key's publication ordering, strengthened to exclusive rename so a crash cannot leave a second key link, `rusqlite` with WAL and `synchronous=FULL` as in
the other stores, and `AccountId`/`SpaceId` from `calternal-types`.
