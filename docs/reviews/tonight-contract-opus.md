# Tonight contract v0: host/storage/security review (Opus 5.5)

2026-11-16. Reviewed `docs/tonight-contract.md` against `AGENTS.md`, the reconciliation section of `docs/early-use-plan.md`, `docs/morning-decisions.md` and the host/broker boundaries in `docs/architecture-proposal.md` and `docs/server-host-assessment.md`. The repo has no code yet. I implemented nothing and did not edit the contract.

**Verdict:** the scope is realistic for one night of localhost work with disposable data. Five gaps would cause lanes to diverge or would make the "durable"/"denied" proofs untrue. Each fix is a small edit to the contract text. All five are **mandatory before coding lanes start**. Everything else is listed at the end as later gates.

## 1. Mandatory: freeze contract rev 0 as code plus fixtures before the lanes split

**Problem.** The server lane may not edit `crates/core`, yet it serializes core types. Web and the plugin SDK need TypeScript types that nobody is assigned to produce. Several shapes are unspecified, so four lanes will each invent their own:
- the `plugins.*` operation names and inputs
- the `notes-insights.summarize` input (presumably `{space_id,id}`)
- the `notes.history` response
- the `updated_at` format
- whether read operations need `request_id`
- the mapping from error code to HTTP status
- the worker framing (see fix 4)

**Fix.**
- The Sol lane lands `crates/core` first. It should contain the serde types with `deny_unknown_fields` on inputs, the error codes, the GenUI types and validator, a generated JSON Schema, and generated TypeScript in a path that web and the SDK import.
- The parent owns `contract/fixtures/*.json`: one request/response pair per operation, plus error, conflict and hostile-GenUI cases. Every lane's tests load these fixtures.
- Pin these values:
  - timestamps are RFC 3339 UTC strings
  - `revision` is an opaque string
  - every request carries a `request_id`, but deduplication applies only to mutations
  - HTTP status: 401 unauthorized, 403 forbidden, 404 not_found, 409 conflict, 400/413 invalid_input, 503 unavailable
- Keep `crates/core` free of IO: no tokio, reqwest or uniffi. The server then depends only on core, the HTTP client lives in `crates/client`, and UniFFI lives in a separate `crates/ffi` (or a client feature), so the server never pulls it in.

## 2. Mandatory: make SQLite authoritative and Markdown a checked materialization

**Problem.** The contract's "best simple option" is right, but it leaves unspecified which store reads come from, whether receipts commit atomically with the revision, and how an external edit to a file is detected. Without those answers, "never silently overwritten" and "idempotent retry" cannot be proven.

**Fix.** Write this exact sequence into the contract.

1. A single writer handles all mutations. SQLite runs in WAL mode with `synchronous=FULL`.
2. One transaction inserts four things together:
   - the new revision row, holding the full Markdown, title and content hash
   - the head-pointer update, after checking `base_revision`
   - the idempotency receipt: (principal, request_id) → operation, a canonical hash of the input, and the result
   - `materialization = pending`
3. Before materializing, check the file currently at `space/Notes/<uuid>.md` against the hash stored at the last materialization.
   - If the file is missing or matches, write a temp file in the same directory, fsync it, rename it over the target, fsync the directory, then mark the row `materialized` with the new hash.
   - If the file differs (an external edit), do not write. Mark the note `external_change`. The ack still succeeds, because the revision is committed, but the response carries an explicit flag. Further updates fail `conflict` until an explicit resolution, which is a later gate.
4. Return the ack only after step 3 completes or flags the external change.
5. On startup, take an exclusive lock on the data directory (refuse a second instance), then redo every `pending` row through step 3.

All reads come from SQLite. Never parse the file back as truth tonight. A test-only crash-point hook covers four points (after commit, after temp write, after rename, before mark), with a kill-and-restart test at each. Document the guarantee: "acked ⇒ in SQLite and on disk, or flagged". This is a preview profile, not the inherited FO1/tiering guarantees.

## 3. Mandatory: pin the dev auth mechanism and block cross-origin and rebinding attacks

**Problem.** "Explicit connection UI or a deliberate local bootstrap flow" is still open. A cookie-based or URL-token flow would create CSRF and leak risks.

**Fix.**
- **Tokens.** On first start the server generates one random 256-bit token per dev principal into `data/dev-tokens/<principal>` with mode 0600, and prints only the file paths. Tokens are compared in constant time. Config maps principals to space grants. Request data never names a principal.
- **Transport.** Auth uses only `Authorization: Bearer`, with no cookies. The server sends no CORS headers at all.
- **Origin and Host.** If an `Origin` header is present it must be same-origin. `Host` must be in `{127.0.0.1:<port>, localhost:<port>, [::1]:<port>}`, which blocks DNS rebinding. The server refuses to bind a non-loopback address tonight.
- **Static assets and dev server.** Static assets are served same-origin with `Content-Security-Policy: default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'`. Vite dev proxies `/api` and binds to localhost only.
- **Browser.** The token is pasted into the connection UI and held in memory or `sessionStorage` only, never in a URL, fragment or `localStorage`.
- **CLI.** The CLI reads the token from `CL_TOKEN_FILE`/config or `CL_TOKEN`, never from argv.
- **Logs.** The server's logger redacts `Authorization`.

## 4. Mandatory: specify a one-shot worker protocol and validate its output on the host

**Problem.** The ownership split is unclear: Luna owns `scripts/plugin-worker.*` and the server lane owns the spawner. The framing is undefined. And if a warm worker is used, it brings in the authority-generation problem from the reconciliation.

**Fix.**
- **Process model.** Spawn one process per invocation, with no pool, so tonight no worker outlives an authority check.
- **Ownership.** The server owns the spawner. Luna owns the worker entry script and the SDK. The protocol lives in core types and fixtures.
- **Framing.** Stdin carries one JSON document: `{worker_protocol:1, request_id, plugin_id, plugin_version, bundle_sha256, operation, input, resource:{id,space_id,revision,markdown}}`, then EOF. Stdout carries one JSON document: `{ok:true,data}` or `{ok:false,error:{code,message}}`.
- **Process limits.**
  - Start with a cleared environment (`env_clear`, then only `PATH`/`NODE_*` as needed) and an empty temporary cwd.
  - Run in its own process group, and kill the whole group on timeout (for example 2 s) or when stdout exceeds 256 KiB.
  - Capture stderr up to a cap and log it with redaction.
  - Optionally add Node `--permission` as defense in depth, never claimed as a sandbox.
- **Before spawn:**
  - verify the bundle's sha256 against the host's registry of trusted bundles
  - check that the plugin is enabled
  - check that the caller is authorized for the resource
- **After return:**
  - check again that the plugin is still enabled
  - validate `data` with the Rust validator from core: stats are non-negative integers and `view` passes the GenUI limits
  - bind the result to the resource revision that was read

A crash, timeout or invalid output from the worker maps to `unavailable` and does not affect Notes.

## 5. Mandatory: confine GenUI actions and validate with one shared corpus

**Problem.** `button.action:{operation,input}` lets a plugin view make a user click call `notes.update` with input the plugin crafted, which is a confused-deputy risk. The validation limits are unnamed, and only the SDK is required to validate.

**Fix.**
- **Envelope.** The envelope is `{genui_version:1, root}`.
- **Fixed limits.** Write fixed numbers into the contract: depth ≤ 16, nodes ≤ 256, text ≤ 4 KiB per node, label ≤ 200, IDs matching `[A-Za-z0-9_-]{1,64}` and unique.
- **Button actions.** A button's `action.operation` must be one the contributing plugin declares in its manifest. In v0 these are read-only, so host mutations are never allowed. The client mints a fresh `request_id` for each click, and the host rechecks enablement and authorization when the action is dispatched.
- **Resource nodes.** `resource` and `document-editor` nodes are references only. The client loads their content with its own token, and an unauthorized reference renders as a visible error.
- **One corpus.** The Rust validator in core and the TS validator in the SDK must give identical accept/reject results on the shared `contract/fixtures/genui/` corpus.

## Later gates (not tonight; record in morning-decisions where product-facing)

- UniFFI: generating bindings on Linux plus a `cargo check` is enough. The cancellation handle should be designed in, but there is no Swift or Apple proof tonight.
- Preview semantics beyond "rendered against fixtures": summarize is read-only, so tonight's preview proves little about side-effect isolation.
- Authority-generation invalidation for warm workers or subscriptions, GenUI patches and streaming, native rendering.
- Resolving external-file conflicts, history retention, and an inherited storage-guarantee mapping before any real data.
- Real auth and sessions, an OS sandbox, credential isolation, and benchmark-host performance gates.
