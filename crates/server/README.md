# calternal-server: localhost development-preview host

Development preview with protected account sign-in and stored space authority.
Use disposable data. The preview content store does not provide inherited tiering,
FO1 recovery integration, or a production plugin sandbox.

## Run

```sh
scripts/with-toolchain.sh cargo build -p calternal-server -j4
preview_data=$(mktemp -d /tmp/calternal-preview.XXXXXX)
target/debug/calternal serve --data-dir "$preview_data" --port 0 \
  --web-dir apps/web/dist --plugin-root plugins
```

The listener binds `127.0.0.1`. `startup.json` (0600) reports `base_url`,
`passkey_origin`, `setup_token_file`, `development_tokens`, `pid`, and token file
paths. The default passkey origin is `http://localhost:<bound-port>`: WebAuthn
needs a hostname. Use the reported URL and preserve the port across restarts to
preserve the passkey origin. `--origin https://example.org` selects an exact
public origin behind a TLS proxy. `--trusted-proxies` accepts explicit CIDRs;
without it, forwarded client addresses never influence rate-limit buckets.
Only the configured public authority and the listener's loopback authorities
are accepted as Host values. TLS is supplied by the deployment proxy.

With no accounts, the privileged local setup token is written to `setup.token`
(mode 0600) and its path appears in startup metadata. Its value is never logged.
Redeem it through setup/start and setup/finish below. Enrollment creates an account,
not a space; use `spaces.create` after signing in. An account's server admin role
grants no content access.

`--development` explicitly enables disposable seeded accounts and development
bearer files; this flag is **off by default**, including after restart. Existing
token files do not grant authority without this flag. Development `base_url`
retains the preview's numeric loopback URL for existing bearer clients; use
`passkey_origin` for browser passkey sign-in. The `owner` fixture owns the
`personal` and `shared` space aliases; `guest` reads only `shared`. Each account
and each space has an independent UUIDv7. Token files are 0600 under a 0700
`dev-tokens` directory. New spaces use UUID roots. Aliases are retained temporarily.

## HTTP

Trusted account endpoints use `/api/v1/auth/` and bare JSON results:

| Method/path | Input or result |
|---|---|
| GET status | `{setup, signed_in}`, no directory information |
| POST setup/start | `{token, name?}` -> `{ceremony, options}` |
| POST setup/finish | `{ceremony, credential, device?}` -> user and host cookie |
| POST login/start | empty body or JSON -> discoverable options; no username |
| POST login/finish | `{ceremony, credential, device?}` -> user and host cookie |
| POST reverify/start, reverify/finish | session-account/purpose-bound challenge, same finish shape |
| POST logout | revokes current installation; 204, clears cache; never clears a newer cookie |
| GET me | private profile/server capabilities and installation ID |
| GET installations | current account's devices, with `current` flags |
| POST installations/revoke | `{installation}`; foreign and absent IDs both not found |
| GET passkeys | current account's passkeys |
| POST passkeys/add/start | `{}`, requires recent interactive proof |
| POST passkeys/add/finish | `{ceremony, credential, name?}`, rechecks proof |
| POST passkeys/rename | `{id, name}` |
| POST passkeys/remove | `{id}`, requires recent proof; never removes the last key |

Account POSTs require JSON and the exact configured Origin when present. Bodies
are capped at 64 KiB. Source and target admission, bounded purpose-specific
ceremony pools, account signature-failure backoff, token hashes and current
revocation checks come from the ported auth library. Cookies are
`__Host-calternal_session`, `Path=/; HttpOnly; SameSite=Lax; Secure`, without a
Domain. Duplicate host cookies are refused across the merged router. No legacy cookie is recognized or cleared; delayed logout/refusal never expires
a newer host cookie. Native clients may send a session bearer. DAV/app passwords are not
accepted. Account mutation receipt replay and private live event feeds are not
yet integrated (see PORTING.md and the ignored source test).

`GET /api/v1/describe` returns `DescribeResult`. `POST /api/v1/operations` takes
`{request_id, operation, input}` and returns the shared operation envelope. A
session supplies only account identity; current Directory capability rows decide
each operation, retry, commit and asynchronous delivery.

`resolve {kind, id}` resolves `/<kind>/<id>` to `{kind,id,space_id,access}`. The
current preview has only the `notes` item kind. Wrong kind, missing item and no
access return identical 404 `not_found`/`item not found`. Resolution currently
probes current membership roots, without enumerating content; the future storage
index can replace this linear-in-memberships lookup without changing clients.
`notes.get`, `notes.update`, `notes.history` and `notes-insights.summarize` accept
just an item ID (plus their own fields). A supplied compatibility `space_id` must
match the resolved owner. Collections and creation still require a space.
The CLI's get/update/history commands omit the hint by default.

Every response retains CSP, nosniff, no-referrer, DENY framing and API no-store.
There are no CORS headers. Static routes such as `/notes/<id>` and
`/collections/<id>` fall back to index.html; missing assets and API routes do not.
Core operation bodies remain capped at 2 MiB, Markdown at 1 MiB. The server
supports the settled addresses; the current web router still needs Claude-owned
changes listed in HTTP-VALIDATION.md.

## Durability profile (preview)

SQLite (`state.sqlite3`, WAL, `synchronous=FULL`) is authoritative for revisions, receipts and plugin state. Reads always come from SQLite. `spaces/<space>/Notes/<uuid>.md` is a checked, portable copy of each note's head revision.

A write proceeds as follows:

1. **Commit.** One transaction commits the full revision, the receipt `(principal, request_id) → operation, SHA-256 fingerprint of canonical input, result` and `mat_state=pending`.
2. **Check the existing file.** It must be missing on a first write, or match the SHA-256 recorded at the last placement. Symlinks and non-regular files never pass.
3. **Place the new bytes.** Write a temp file, fsync it, rename it over the target, then fsync the directory. Newly created directory ancestry is also fsynced.
4. **Mark and acknowledge.** Set `mat_state=materialized` with the new hash, then return the acknowledgement.

Recovery and retries:

- **Startup.** The server takes an exclusive lock on the data directory (a second instance exits with code 2). It removes only its own `.<uuid>.<12hex>.tmp` files, then replays pending notes before serving.
- **Crash after rename.** If replay finds the new bytes already in place, it fsyncs the file and the directory before marking.
- **Retry with the same input.** It returns the original result, finishing any pending placement. It is reauthorized against current grants and does not re-run the old base-revision check.
- **Retry with different input.** It returns 409 `request_id_reused`.
- **Plugin enable/disable.** These are receipted in the same way, so retrying an old enable cannot undo a later disable.

**External changes.** A changed, deleted or replaced note file is never overwritten. The note is blocked: updates return 409 with `details.reason = "external_change"`, the external bytes are preserved, and reads still serve the SQLite revision. There is no import and no unblock command tonight. The hash check detects changes that happened before a write. It does not lock out an external writer that races the check and the rename. Concurrent external editing is out of scope.

**Crash tests.** In debug builds only, `CALTERNAL_FAULT=after_commit|before_placement|after_placement|before_reply` aborts the process at that point. `tests/http.rs` kills the server at each point, restarts it, and checks the placed bytes, the idempotent retry, that history has no duplicates and that no temp files are left.

## Plugins

- **Registry.** Only entries in the compiled-in `trusted-plugins.json` load. Manifests are read once at startup, checked against the frozen shape, the core validators, the registry's id and version, and exact dependency versions. They stay immutable in memory.
- **Digest.** Each worker bundle's SHA-256 is pinned and verified both at startup and before every spawn. The browser can never supply code, paths or URLs.
- **Worker process.** Every invocation gets a fresh process:
  - `node --permission --allow-fs-read=<bundle dir> --disallow-code-generation-from-strings worker.js`
  - the environment is cleared except `LANG`, and the working directory is the bundle directory
  - limits: stdin 2 MiB, stdout 256 KiB, stderr 16 KiB (stderr content is never logged), timeout 3 s
  - the process is killed and reaped on overflow (immediately), timeout or a nonzero exit
- **Concurrency.** A global cap of 4 workers is enforced with a nonblocking permit. When it is full, calls get 503 with `reason: worker_capacity`.
- **Result checks.**
  - The output must be exactly one protocol object with a matching version and invocation id, and no trailing data.
  - Its GenUI view must pass `validate_extension_view`, so buttons may call only the plugin's declared read-only operations.
  - Before delivery, the host checks that the plugin is still enabled at the same generation. Otherwise it returns 409 `plugin_generation_changed` (`tests/race.rs`).
- **Preview.** Preview runs the real worker on fixed fixture text, works while the plugin is disabled, and never activates it.
- **This is not a security sandbox.** Node's permission model is defense in depth for repository-shipped trusted bundles only.

## Tests

`scripts/with-toolchain.sh cargo test -p calternal-server -j4` runs these suites. They need Node 22.13 or later on PATH.

- **Worker unit tests (`src/plugins.rs`):** cleared environment, filesystem confinement, malformed and hostile output, timeout and overflow, admission cap, digest tampering.
- **`tests/http.rs` (black-box tests of the real binary):** two principals, stale writes and history, retries, crash points, external edits, the lock, Host/Origin/bearer/CSP checks, static fallback, plugin receipts, temp-file cleanup.
- **`tests/race.rs`:** disabling a plugin while a call is in flight.

Timings from these tests are development-host smoke numbers, not performance gates.
