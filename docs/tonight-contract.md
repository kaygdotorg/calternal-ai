# Overnight contract v0 — draft for review

Goal: testable localhost development preview with disposable data; real file-backed Notes, trusted TypeScript plugin extension, portable GenUI renderer, Rust shared client/CLI. No fake AI responses, production auth, hostile plugin sandbox claims, full offline sync claims or production migration. UI is Claude-only.

## Wire contract

- HTTP JSON `/api/v1/describe` GET and `/api/v1/operations` POST. Loopback by default, dev session bearer required for operations and describe; browser receives token via explicit connection UI or a deliberate local bootstrap flow (review safe mechanism). No wildcard CORS. Static web asset serving same origin preferred.
- Request `{request_id: string, operation: string, input: object}`. Server binds caller from the dev bearer, never trusts caller IDs in request data. Config supplies two explicit dev principals and space grants; never real accounts. Distinct tokens never printed in logs.
- Response success `{request_id, ok:true, data}`; failure `{request_id, ok:false, error:{code,message,details?}}`, proper HTTP status. Unknown operations/fields fail clearly; structured codes include unauthorized, forbidden, not_found, conflict, invalid_input, unavailable.
- Resource shape `{id, space_id, title, markdown, revision, updated_at}` (list omits markdown); revision opaque server token. Stable UUID resource ID, title never a filesystem path. notes.list `{space_id}`, notes.get `{space_id,id}`, notes.create `{space_id,title,markdown}`, notes.update `{space_id,id,base_revision,title?,markdown}`, notes.history `{space_id,id}`. UUID filenames inside space/Notes; title visible in metadata. This storage path is disposable-preview only, not a superseding production layout.
- Create/update require stable request_id deduplication bound to principal+operation+input; reuse with different input is conflict. Server returns same completed result on retries and preserves revision checks. Enforce maximum request/body/document sizes, title validation, path traversal denial. No delete required overnight.
- Persistence: Markdown files plus transactional state/revisions/receipts using SQLite or proven existing primitives. Never acknowledge before durable commit. Ensure crash consistency of content/state with a small replayable pending-write journal or single authoritative committed revision plus explicit materialization state; no dual authoritative stores. Document precise preview guarantees, fault tests. Current acknowledgement policy (2026-11-17, ADR 0062): transactions persist full version data, intent and receipt before ack on SSD; checked Markdown materialization follows within one second, and spaces recover on first use. This is not inherited full file-only recovery. External concurrent editors not supported yet and must not be silently overwritten.

## Plugin/GenUI v0

- Repo-shipped `notes` app and `notes-insights` extension, TypeScript. JSON manifest with id/version/api_version/dependencies/capabilities/operations/views. Stable known extension point `notes.detail`. Backend host operations remain protected; TypeScript contributes UI definition plus composed/read-only extension action `notes-insights.summarize` (word/line/character statistics, never an LLM claim).
- Host plugin list/enable/disable use public operations. Preview accepts only registered trusted plugin IDs, renders fixtures/disposable data and does not activate. No arbitrary uploaded code/path/URL loading from browser. Immutable registered bundle/version validation.
- A bounded Node worker runs the real compiled TypeScript extension with JSON stdin/stdout, timeout and output cap. Host first reads authorized canonical resource; worker receives only that text. Child process is NOT an OS security sandbox; only shipped trusted bundles run. Do not pass ambient secrets/environment. Disable extension prevents its operation/views; failed worker returns error without breaking Notes.
- GenUI uses a discriminated JSON tree: `{type:'stack',id,children}`, `{type:'text',id,text}`, `{type:'button',id,label,action:{operation,input}}`, `{type:'stat',id,label,value}`, `{type:'resource',id,resource_id,space_id,label}`, `{type:'document-editor',id,resource_id,space_id}`. No eval, arbitrary HTML, remote fetch or script; stable IDs, max depth/nodes/text length and duplicate-ID validation. Typed inputs are data, not executable expressions. Client renders unknown node as explicit unsupported fallback. Permissions rechecked at action dispatch. Native schema compatibility is versioned `genui_version:1`.
- Read-only plugin action response includes `{stats:{words,characters,lines},view:{genui_version:1,root:...}}`; SDK unit tests validate schema/hostile trees. Web document editor uses native textarea for this proof if existing editor extraction cannot fit; explicitly no CRDT/production editor claim. Do not wire arbitrary innerHTML.

## Shared core / Apple boundary

Rust crate exports wire/resource/GenUI models, validation, typed errors, optimistic revision and pending request identity state; async HTTP Rust client and CLI use these types. CLI `cl describe`, `cl notes list/get/create/update`, `cl plugins list/enable/disable`, `cl invoke OP --input JSON`, with JSON output. Base URL/token via env/config; avoid tokens in argv/logs.

UniFFI optional feature exports a small opaque JSON contract/validation facade or real client operations if practical, with explicit cancellation boundary documented. Generate usable bindings if toolchain available; check Rust compilation. Do not call a generated file alone an Apple execution proof. Swift client logic must not fork protocol semantics. No SwiftUI/UI code by non-Claude lanes.

## UI owned by Claude

Svelte 5/Vite web shell, one left sidebar with app heading strip, Activity and app nav, main Notes pane, optional closable AI pane. Actual Notes selection/edit/save/history, visible revision conflicts and recoverable draft on failed save, plugin preview/activation/recovery, extension GenUI result. Real stable note deep links. Clear development-preview state; absent harness integration is shown as unavailable/setup pending, not simulated chat. Keyboard/accessibility, reduced motion, responsive layout and clean empty/loading/error states. Token entry is private and not persisted into URLs.

## Lane ownership

- Opus 5.5 medium: crates/server/**, Rust host persistence/HTTP/plugin host integration. May read core types, coordinate changes; no shared crate edits.
- Sonnet 5.5 medium: apps/web/**, all UI and visual verification/fixes, frontend package dependencies.
- GPT-6.1-Sol medium: crates/core/**, crates/client/**, crates/cli/**, contract models/code generation/UniFFI. Own root Cargo.toml initially; coordinate dependency changes.
- GPT-6-Luna max: packages/plugin-sdk/**, plugins/**, scripts/plugin-worker.*, backend/SDK integration tests and docs. No UI implementation or visual design.
- Parent: contract review/reconciliation, shared root build scripts/README, integration and non-UI fixes, final verification. No UI code.

## Required proof

Start server on loopback; web and CLI list/open/edit same note, preserve across restart; stale revisions produce conflict without losing editor draft; two principals cannot cross unauthorized space; request retry is idempotent; plugin preview has no live side effects; actual TS worker returns GenUI statistics; disabled plugin denied; malformed GenUI rejected; full build/tests pass. Measure development-host smoke timings only with methodology, not production perf gates. Leave run instructions and testable running preview, no public exposure. No unverified cleanup outside this task.

## Frozen review corrections (override draft ambiguities)

Both reviews are in docs/reviews/tonight-contract-{opus,sol}.md. Core lane publishes contract/v0.schema.json, contract/fixtures and packages/contracts/src/index.ts from Rust types FIRST. Core crate has no HTTP/async/UniFFI; crates/client owns HTTP; crates/bindings owns optional UniFFI. No UI code outside Claude.

### Exact envelope/results

All POST operations including reads carry request_id (1..128 chars). Revision is opaque string; updated_at UTC RFC3339. Lists: {notes:[NoteSummary]} with title/id/space_id/revision/updated_at. Get/create/update: {note:Note}. History: {versions:[NoteVersion],next_cursor:null|string}; full markdown/title/revision/updated_at in each version. History input {space_id,id,limit?:1..100,cursor?:string}; default20. List limit100 for preview.

plugins.list input {space_id}, result {plugins:[{manifest:PluginManifest,enabled:boolean,generation:integer}]}. enable/disable input {space_id,plugin_id}, result {plugin:{manifest,enabled,generation}}, manager role required. Preview input {space_id,plugin_id}, result {view:GenUiView,preview:true}, fixed fixture text only, works while disabled. notes-insights.summarize input {space_id,id}, host reads authorized note, result {stats:{words:integer,characters:integer,lines:integer},view:GenUiView}. notes enabled by default; notes-insights requires enablement.

GET /api/v1/describe result {protocol_version:1,genui_version:1,principal:{id:string,spaces:[{id:string,role:reader|editor|manager}]},operations:[{id,description,mutates:boolean,input_schema:object,output_schema:object}]}. Requires bearer + Host/Origin checks. Catalog may include disabled operations; invocation checks enablement. No query token. Malformed envelope and pre-dispatch authentication/Host/Origin failure responses use request_id:null; otherwise echo. Clients accept null IDs on failure while retaining their pending identity, reject mismatching nonnull IDs, and require every successful POST response to echo request_id. Error/status: unauthorized401 forbidden403 not_found404 conflict409 invalid_input400 too_large413 unavailable503 internal500. Safe optional error.details object; no paths/stacks/tokens. Require application/json POST. Reject unknown envelope/input fields. Do not log request bodies or tokens.

PluginManifest {id,version,api_version:1,name,description,dependencies:[{id,version}],capabilities:[string],operations:[{id,description,mutates:false}],views:[{id,extension_point:notes.detail,view:GenUiView}]}. Notes may have zero operations (host owns protected Notes methods). notes-insights declares notes.read and summarize. Trusted registry validates exact API/dependency version and compiled bundle digest; never browser-supplied code/paths.

### Worker

One UTF8 JSON object then EOF: {protocol_version:1,invocation_id,operation:notes-insights.summarize,resource:{id,space_id,revision,title,markdown}}. Exactly one output object {protocol_version:1,invocation_id,ok:true,data:{stats,view}} or {protocol_version:1,invocation_id,ok:false,error:{code:invalid_input|internal,message}}. stdout protocol-only, stderr diagnostics. stdin2MiB stdout256KiB stderr16KiB timeout3s; kill/reap on timeout/overflow/nonzero. Reject trailing output/version/ID mismatch. Fresh process, verified absolute Node executable, explicit env allowlist without secrets/NODE_OPTIONS. Server owns process; SDK lane supplies compiled worker. Bind call to plugin generation and reauthorize result delivery.

### GenUI

Exact draft catalog; stat.value string, button.input object. v1 host rejects unknown fields/types and duplicate IDs. Max128 nodes, depth8 root=1, combined string UTF8 bytes32KiB, single text8KiB, serialized view64KiB. IDs 1..80 ASCII [a-zA-Z0-9_.:-]+; labels1..256 Unicode scalars. No arbitrary URL/HTML/eval/style/fetch. Resource/editor references validated IDs, authorized at load. Extension buttons invoke only its declared read-only operations; host validates. Future unknown node client fallback never dispatches actions.

### Durability

Preview SQLite authoritative for committed revisions/receipts; Markdown checked portable copy, not file-only recovery. Transaction full revision + canonical request fingerprint/result + pending-materialization; serialize resource writes. Check last-known file BLAKE3 before atomic temp write/fsync/rename/directory sync; acknowledge durable SSD history/receipt, then mark materialized under the one-second policy. Retry reauthorizes and returns the original durable result without requiring userdata placement or reapplying the old base check. External changed/deleted files block overwrite, no external-edit import tonight. Startup takes exclusive root locks and completes protected authority recovery; each content space replays its pending work before first use; preserve ambiguous external bytes. Test after DB commit/before placement/after placement/before HTTP reply; retries no duplicate history. Document1MiB body2MiB title256 Unicode scalars/no controls.

### Dev sessions

Random separate tokens: owner manager in personal/shared; guest reader in shared only. Owner-only token file; print path only. Loopback server, exact Host allowlist, Origin absent allowed authenticated CLI or exact same-origin; foreign/null Origin denied. No CORS/cookies. Compiled static assets same-origin. CSP: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'. nosniff, referrer no-referrer. Browser explicit token entry, memory only, clear state on change. CLI token file/env, not argv. Default port3210.
