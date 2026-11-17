# calternal-secrets

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

## Costs

Each call opens and closes the owner's SQLite database (about 0.1–0.5 ms); writes and
`use_value` are one fsync'd transaction; sealing and opening are one AES-GCM pass over
at most 64 KiB.
