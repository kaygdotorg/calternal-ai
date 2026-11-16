# calternal-server: localhost development-preview host

**Development preview only.** It runs on loopback with disposable data and two fixed dev principals. It is not production auth, not an OS sandbox for plugins, and does not provide the inherited Calternal storage, tiering or recovery guarantees. Do not point it at real data.

## Run

```sh
scripts/with-toolchain.sh cargo build -p calternal-server
target/debug/calternal serve --data-dir .dev-data --port 3210 \
  --web-dir apps/web/dist --plugin-root plugins   # optional: --node /abs/node
```

- The server binds `127.0.0.1` only. `--port 0` picks a free port.
- When it is ready, it atomically writes `<data-dir>/startup.json` (mode 0600). The file contains `profile`, `base_url`, `pid`, `data_dir`, `token_files.{owner,guest}` and `web_dir`. It is removed on clean shutdown.
- The dev tokens are in `<data-dir>/dev-tokens/{owner,guest}.token`. Each is a raw 256-bit hex token with no newline. The files are 0600 and the directory is 0700. Token values are never printed or logged; only the paths are printed.
- Principals:
  - `owner` is manager of `personal` and `shared`.
  - `guest` is reader of `shared` only.
- `calternal plugin-digest --plugin-root plugins` prints worker bundle digests and whether each matches `trusted-plugins.json`.

## HTTP

- `GET /api/v1/describe` returns a bare `DescribeResult`.
- `POST /api/v1/operations` takes `{request_id, operation, input}` and returns the `OperationResponse` envelope. All types come from `calternal-core`.
- A failure that happens before the request body is parsed (Host, Origin, auth, content type, malformed envelope) has `request_id: null`.
- Every request, static assets included, must use an exact Host from `127.0.0.1:PORT`, `localhost:PORT` or `[::1]:PORT`.
  - An `Origin` header may be absent, or must equal `http://<Host>`. `null` and foreign origins are denied, as is `Sec-Fetch-Site: cross-site`.
  - The API requires `Authorization: Bearer`, compared in constant time.
  - There are no cookies and no CORS headers.
- Every response carries the frozen CSP, `nosniff`, `no-referrer` and `X-Frame-Options: DENY`. API responses are `no-store`.
- Static files come from `--web-dir`.
  - Client routes such as `/s/<space>/notes/<id>` get `index.html`.
  - A missing `/assets/*` path, or any final path segment containing a `.`, returns 404.
  - Any `/api/*` path returns a JSON envelope, never HTML.
- Limits: body 2 MiB (413), document 1 MiB, title 1..256 characters with no control characters, list 100, history limit 1..100 (default 20). Unknown fields are rejected.

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

`scripts/with-toolchain.sh cargo test -p calternal-server` runs these suites. They need Node 22.13 or later on PATH.

- **Worker unit tests (`src/plugins.rs`):** cleared environment, filesystem confinement, malformed and hostile output, timeout and overflow, admission cap, digest tampering.
- **`tests/http.rs` (black-box tests of the real binary):** two principals, stale writes and history, retries, crash points, external edits, the lock, Host/Origin/bearer/CSP checks, static fallback, plugin receipts, temp-file cleanup.
- **`tests/race.rs`:** disabling a plugin while a call is in flight.

Timings from these tests are development-host smoke numbers, not performance gates.
