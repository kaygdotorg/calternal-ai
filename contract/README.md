# Preview wire contract

This is contract revision v0, with `protocol_version:1`, `api_version:1` and
`genui_version:1`. It describes the disposable-data localhost trusted-plugin
preview, not production storage/authentication, hostile-code isolation or offline sync.

`crates/core/src/models.rs` is the model source. Generate artifacts with:

```sh
scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract
scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -- --check
```

The second command compares all three generated files without writing them:

- `v0.schema.json`: schema root groups named contract types; it is **not** an
  HTTP body. Use its `definitions` or the standalone discovery schemas.
- `operations.json`: operation descriptions with independently usable input and
  output schemas, also returned by `operation_catalog()`.
- `../packages/contracts/src/index.ts`: generated TS types, exported by
  `@calternal/contracts`; booleans and version markers are literal types.

Fixtures are shared hand-authored conformance inputs checked by Rust tests and
SDK tests. They cover valid requests/views, unknown fields, null optional
inputs, traversal, malformed worker frames, duplicate IDs and unsupported types.
Schema validation does not replace semantic validation: UTF-8 byte budgets,
combined string limits, tree depth/duplicate IDs, declared action permissions,
resource IDs, titles and current authorization also need runtime checks.
GenUI combined string bytes count string **values**, including type names,
IDs and nested action input values, not object keys.

`calternal_core` exports all models and synchronous validators. Important APIs:
`parse_request`, `validate_request` (host known-operation validation),
`validate_client_request` (envelope plus known input validation; leaves unknown
operation schemas to the host), `validate_genui` (version and size of a
generated-UI view; the host also compiles its source with `calternal-genui`),
`canonical_request_input`, and `PendingRequest`. Recursively sorted object keys
make equivalent input key order compare identically without value coercion.

`calternal_client::Client` owns async reqwest effects. Successful discovery is a
bare `DescribeResult`; POST uses `OperationResponse<T>`. Errors carry typed
`ErrorCode`, with null response identity allowed before dispatch; mismatching
nonnull IDs and successful POSTs without the correct identity are rejected.
The adapter accepts only an HTTP loopback origin, disables proxy/redirects and
redacts transport errors. Response bodies are bounded to 128 MiB; unusually large history pages that exceed
that cap fail visibly, and callers can request a smaller history limit.

Use `Client::pending(...)` and retain its request before `execute`/`invoke`.
Typed note create/update and plugin enable/disable methods require an explicit
borrowed request ID. Unknown outcomes retain the same ID, operation and input;
changing input requires a fresh ID. Neither transport failure nor cancellation
proves the server did not commit. No automatic retry or durable local outbox is
claimed. Dropping an async future only stops this caller's wait.

The `cl` executable supports `describe`, `notes list/get/create/update/history`,
`plugins list/enable/disable/preview`, and `invoke OP --input JSON`. `note` and
`plugin` are aliases. Set `CALTERNAL_URL` or `--base-url` (default
`http://127.0.0.1:3210`). Tokens use `CALTERNAL_TOKEN` or `CALTERNAL_TOKEN_FILE` /
`--token-file`; the file must be a regular owner-only file, not a symlink.
There is no token-value CLI flag. Mutation retry identities use `--request-id`.
Use `--markdown-file -` to read Markdown from stdin, or a UTF-8 file path.
POST success JSON is printed to stdout; failure envelopes including request IDs
are printed to stderr with a failing exit status. Successful describe is bare JSON.

```sh
cl --token-file .dev-data/dev-tokens/owner.token describe
cl --token-file .dev-data/dev-tokens/owner.token notes list --space personal
cl --token-file .dev-data/dev-tokens/owner.token --request-id draft-1 notes create --title 'Preview note' --markdown-file draft.md
```

Tests and real-CLI smoke (build server and CLI first):

```sh
scripts/with-toolchain.sh cargo test -p calternal-core -p calternal-client -p calternal-cli -p calternal-bindings
scripts/with-toolchain.sh cargo build -p calternal-server -p calternal-cli
python3 contract/verify-cli.py
python3 contract/verify-cli.py --running-data-dir .dev-data
```

Default smoke creates its own temporary data directory/server and verifies
restart persistence before removing only that directory. Running-server mode
creates disposable notes, never stops that server, and namespaces request IDs
per run so it can be repeated. It never prints tokens.
