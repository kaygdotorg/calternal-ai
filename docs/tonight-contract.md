# Overnight contract v0 — draft for review

Goal: testable localhost development preview with disposable data; real file-backed Notes, trusted TypeScript plugin extension, portable GenUI renderer, Rust shared client/CLI. No fake AI responses, production auth, hostile plugin sandbox claims, full offline sync claims or production migration. UI is Claude-only.

## Wire contract

- HTTP JSON `/api/v1/describe` GET and `/api/v1/operations` POST. Loopback by default, dev session bearer required for operations and describe; browser receives token via explicit connection UI or a deliberate local bootstrap flow (review safe mechanism). No wildcard CORS. Static web asset serving same origin preferred.
- Request `{request_id: string, operation: string, input: object}`. Server binds caller from the dev bearer, never trusts caller IDs in request data. Config supplies two explicit dev principals and space grants; never real accounts. Distinct tokens never printed in logs.
- Response success `{request_id, ok:true, data}`; failure `{request_id, ok:false, error:{code,message,details?}}`, proper HTTP status. Unknown operations/fields fail clearly; structured codes include unauthorized, forbidden, not_found, conflict, invalid_input, unavailable.
- Resource shape `{id, space_id, title, markdown, revision, updated_at}` (list omits markdown); revision opaque server token. Stable UUID resource ID, title never a filesystem path. notes.list `{space_id}`, notes.get `{space_id,id}`, notes.create `{space_id,title,markdown}`, notes.update `{space_id,id,base_revision,title?,markdown}`, notes.history `{space_id,id}`. UUID filenames inside space/Notes; title visible in metadata. This storage path is disposable-preview only, not a superseding production layout.
- Create/update require stable request_id deduplication bound to principal+operation+input; reuse with different input is conflict. Server returns same completed result on retries and preserves revision checks. Enforce maximum request/body/document sizes, title validation, path traversal denial. No delete required overnight.
- Persistence: Markdown files plus transactional state/revisions/receipts using SQLite or proven existing primitives. Never acknowledge before durable commit. Ensure crash consistency of content/state with a small replayable pending-write journal or single authoritative committed revision plus explicit materialization state; no dual authoritative stores. Document precise preview guarantees, fault tests. Best simple option: transactions persist full version data and intent, atomic markdown materialization before ack, recover pending materialization on start. This is not inherited full file-only recovery. External concurrent editors not supported yet and must not be silently overwritten.

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
