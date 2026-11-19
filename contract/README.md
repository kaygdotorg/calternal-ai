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

## Plugin v2 verifier

Run the pinned verifier alongside its Python modules and JSON Schemas. Use ordinary
Python so it can import adjacent modules. `python -I` is unsupported.

```sh
python3 contract/verify-description.py
python3 contract/test_verify_description.py
python3 contract/verify-description.py --package "$pkg" --event notes-app.json "$pkg/events/window.json"
python3 contract/verify-description.py --manifest "$pkg/manifest.json" --app "$pkg/notes-app.json" \
  --event notes-app.json "$pkg/events/window.json"
```

Repeat `--event DESCRIPTION FILE` for each event. `DESCRIPTION` identifies an
app template relative to the package root or by its submitted path. The event
uses the verified template for its authored identifiers and scopes. Each artifact
passes structural validation before its IDs are indexed or its semantic checks
run. Component events also pass the catalogue's route-specific request schema
before the originating template is indexed. The host
checks live authority over opaque references such as rows, leases, activations
and proofs. To verify a captured host-issued composer Invoke, submit
`{"view_open": <ViewOpenResult>, "event": <ResolveActivation>}` as the
event artifact. The verifier matches the composer and exact context against
the originating description and view-open reply. For an Invoke issued
on renewal, include the paired `renew_request` and `renew_result` in the
same capture. The verifier checks the earlier lease and the matching
outstanding renewal, then uses the replacement composer map and contexts.
The wrapper is a verifier input.

Directory mode finds replies in
`replies/`. Use `--reply FILE` to select replies and `--origins FILE` to override
`replies/origins.json`. The origins map names a verified description and Bind
for each reply. Events and app templates alone need no origins map. Use
`--installed-manifest FILE` for additional kind owners and provider
contributions. The verifier rejects provider IDs repeated across the
supplied installation manifests.

| Input | Bound or check |
| --- | --- |
| Paths and files | Directory mode opens every root component without following symlinks and retains the root descriptor for all artifact lookups and reads. Paths cannot use parent traversal, and root components and artifacts cannot be symlinks. Explicit manifest mode permits operator-selected files outside the manifest directory, while implicit template paths stay inside it. Files must be regular. Paths are capped at 4,096 bytes and 128 components. Templates must match their manifest's SHA-256. |
| Sizes and counts | 4 MiB per artifact and 32 MiB total per package invocation. Up to 16 apps, 32 additional installed manifests, 64 events and 64 replies. Reply discovery inspects up to 128 directory entries. Origins name only submitted replies, once each. |
| Encoding and parsing | Strict UTF-8 and JSON, including escaped strings that encode as UTF-8. Object keys are unique and at most 256 UTF-8 bytes. Decoding is bounded by 4 MiB per artifact and 32 nested containers. The verifier checks nesting before decoding and meters visits over decoded object pairs and the decoded tree. |
| Numbers | Tokens are at most 128 characters. Integer tokens are within ±(2⁵³−1) and decimal or exponent tokens must be finite binary64 values. Schemas enforce smaller ranges where declared. |
| Embedded schemas and references | The manifest passes the pinned manifest schema before indexing. Embedded schemas are checked against the vendored Draft 7 meta-schema and linked against the pinned offline registry and their own schema positions. There is no network fetch. Schema URIs and embedded patterns are capped at 2,048 UTF-8 bytes. Schema evaluation allows 128 nested evaluations. Each package verification or fixture check shares one 200,000-visit budget and one 32-diagnostic collector across decoded tree inspection, structure, indexing, schema linking, ownership and semantic checks. Collection checks preflight their minimum cost where the size is known, charge each inspected element and stop at the diagnostic cap. A reference must target a schema position, even when the target is a boolean. |
| IDs and namespaces | Duplicate declaration IDs are rejected before indexing, including manifest setting keys and secret names. Action IDs span Invoke and shell sites. Form and component IDs are checked in their respective view namespaces. Colliding installed package IDs, kind owners and reserved host kind or operation names are rejected. |
| Catalogue selection | Conformance always uses the checked-in host catalogue. The environment variable `CALTERNAL_HOST_CATALOGUE` is available only in `--fixture-testing`, which cannot be combined with package mode. |

The dev-01 null-array regression submits a full 4 MiB artifact. It requires the
post-decode visit-limit refusal within 5 seconds and 128 MiB peak RSS for the
verifier process, including startup and package validation.
