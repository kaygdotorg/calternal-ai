# calternal-ai: server host assessment

Date: 2026-11-16
Status: recommendation to validate. This file is owned by the host assessment. `architecture-proposal.md` is owned and revised separately.
Scope: the server host language and runtime only. The shared client core and plugin-authoring language are separate decisions (proposal §5). This report covers them only where they constrain the host.

Labels used below:
- **[F]** fact checked in source or official documentation (citations in §11).
- **[R]** a measurement recorded earlier by someone else. It was cited, not re-run here.
- **[H]** hypothesis or judgement. No performance or development-time numbers are claimed. None were measured for this report.

## 1. Recommendation

**Rust is the main server. TypeScript plugins run out of process in a sandboxed JavaScript worker runtime. Node is a worker runtime, not the main server.**

- The **Rust host** owns the protected foundation (proposal §4):
  - session and identity verification, grants, sharing enforcement
  - secret custody
  - the storage, journal and idempotency layer
  - durable admission and run records, leases and scheduling
  - transport
  - the sandbox supervisor that starts, limits and kills workers and CLI harness processes
- **Tenant plugins** run in TypeScript workers:
  - space-replaceable orchestrator, app backends, automations, provider adapters, search providers and generated personal plugins
  - each worker runs in an OS-sandboxed process
  - a worker reaches the world only through capability RPC to the host
- **Keep two layers separate:**
  - **Trusted broker (never replaceable by tenants).** Storage commit, authorization checks, journal and idempotency, secret custody, sandbox. Every implementation, default or tenant, writes through it.
  - **Semantic engines (default native, replaceable per space).** The default engines are compiled-in Rust services where real reuse exists: Notes format, merge and index, plus search. `auth/WebAuthn` and `fsio` belong to the broker, not this layer. Defaults are not the same as "no tenant may ever replace an engine". A space manager may install an alternative implementation of a **versioned engine contract** (for example a TS merge or index engine in that space's worker). It still commits only through the broker and cannot widen authority.

  Product behaviour on top, including a forkable Notes app, is TypeScript plugin code that calls host-brokered operations. This keeps ADR 0048 (one format implementation shared by server and native clients) for the default path. The Rust release cycle is not a bottleneck for tenant customisation.
- **No Wasm runtime in v1**, neither in the browser nor for server plugins (user steering, 2026-11-16). Any later exploration needs its own measured justification.
- **Worker runtime:** Node LTS is the default because the official Claude Agent SDK and npm ecosystem target it. Deno is the alternative to test in validation step V1. Neither runtime's permission system is the security boundary (§3).

**Why the host language matters less than it looks [H]:** untrusted plugin code must run in a separate OS-isolated process whatever the host language is (§3). So a TypeScript host does not let tenant plugins share its process safely. What a TypeScript host would actually buy is one language for trusted first-party code and native use of the TS-only agent SDKs. Against that, it gives up working, gated Rust storage and auth code, and the format sharing with the Rust native client core.

## 2. Criteria

### 2.1 Fast TypeScript plugin authoring and dynamic install

- **[F]** Node runs `.ts` files by stripping types. This is on by default since v22.18.0/v23.6.0 and stable since v24.12.0/v25.2.0. It does no type checking, rejects enums, runtime namespaces, parameter properties and `.tsx`, and ignores `tsconfig.json`. Deno runs TypeScript natively.
- **[H]** The authoring loop is the same under either host: edit the plugin, then the host bundles and type-checks it and validates its manifest. A new worker version starts in preview, then activation swaps the pinned version (proposal §5). No Rust toolchain or host restart is involved.
  - The host only needs a bundler and type checker as tools. These are invoked by the supervisor, not linked into it.
  - Under a Rust host, plugin authors never touch Rust.
- **[H]** A TypeScript host's real authoring advantage is for *first-party trusted* code, which could share types with the host in one process. With a Rust host, that sharing comes from generated contracts.
- **[F]** That contract mechanism already exists: `crates/calternal/src/plugins.rs` writes `contracts/actions.json` (JSON Schema via `schemars`), and the web app generates `actions.gen.ts` from it.

### 2.2 Many isolated tenants and untrusted generated plugins

- **[F] Node:**
  - The permission model "does not protect against malicious code".
  - Malicious code "can bypass the permission model".
  - `process._debugProcess(pid)` is not gated. The docs advise "different OS users, or … OS-level isolation".
  - `node:vm` "is not a security mechanism. Do not use it to run untrusted code."
- **[F] Deno:**
  - Code on one thread shares one privilege level.
  - `--allow-run` and FFI are "equivalent to `--allow-all`".
  - For untrusted code it recommends OS sandboxing (`chroot`, `cgroups`, `seccomp`) or a VM/microVM (gVisor, Firecracker).
- **[F]** Linux Landlock lets an unprivileged process restrict its own filesystem and network rights irreversibly, and children inherit the restriction.
- **[H] Consequence:** in both host designs, the boundary is an OS sandbox around each worker: a separate uid or user namespace, mount namespace, Landlock, seccomp and a cgroup, with no ambient network. Runtime permission flags are defence in depth only.
  - **Isolation domain, not a process per install.** The unit is a logical domain keyed by space, plugin code version and **effective authority context**. That context means the requester's effective grants and visibility, plus any credential or account handle in scope, not just space and plugin version. A process belongs to exactly one domain. Mutually untrusted plugins, or callers with different authority or credential contexts, never share a process or retained state.
    - **Why authority is in the key:** a worker may keep data between calls (caches, module state, temporary files). If it handled a privileged requester's call, it must not later serve a less-privileged requester, or one using a different account binding, and leak what it kept.
    - The host stamps authority on each call, so the worker never carries authority itself.
    - **The exact pooling policy is a validation result, not fixed here (V1).** Candidates, from coarsest to finest:
      - pool by identical authority context
      - require plugins to declare themselves stateless, with the host clearing state between calls
      - one process per request for the most privileged contexts only

      Costly per-request processes should not become the default before measurement shows they are needed.
  - **Lifecycle:** domains are started on demand and stay warm for an idle window. They are then reaped, with eviction under a host memory budget. An installed plugin that is not running has no process and uses no CPU. It still costs its metadata, its stored bundle and its data on disk. The idle window, the warm-pool size and whether pre-warming is needed (for example when an app opens) are measured in V1, not assumed.
- **[F]** The existing cross-user isolation gate (`crates/calternal/src/isolation.rs`) has a second user run every registered action against the first user's real ids. It can be carried over to worker-hosted actions, and it is language-neutral at the action-pipeline level.
- **[H] Existing deployment baseline:** Calternal has a rootless Podman image (`deploy/Containerfile`, `deploy/cloud/calternal.container`). Whether nested user namespaces, Landlock and seccomp work inside that container on the target kernel is unverified. It is the first sandbox fact to test (V1). If they do not work, the deployment shape changes regardless of host language.

### 2.3 Reuse of existing Rust Calternal code

- **[F] What exists** (`origin/dev` `ea57b84b9`), as Rust lines per crate including tests, counted here:
  - `notes` 58,294
  - `auth` 19,310 (WebAuthn via `webauthn-rs`)
  - `tasks` 11,346
  - `calternal` 9,138
  - `search` 5,111 (SQLite FTS5)
  - `store` 4,792 (per-user SQLite journal, single writer thread, idempotency records)
  - `fsio` 4,151 (`write_atomic`, `write_unless_changed`, `beneath`, enforced by a clippy rule)
  - `server` 3,184 (axum, WebSockets)
  - `engine` 2,290 (executor with idempotency)

  The repo also has perf gates (ADR 0005), a 1.99.0 toolchain pin and the `-D warnings`/no-`unsafe` policy.
- **[F] Reuse boundaries:**
  - `Caller.user: UserId` keys everything: one Home and one `index.db` per user (`crates/core/src/action.rs`, `crates/store/src/lib.rs`, `crates/engine/src/lib.rs`).
  - calternal-ai's unit is a **space** shared by several accounts. The engine and store need re-keying, with membership and authority checks added. They cannot be adopted as they are.
  - The plugin model is compile-time: `trait Plugin { fn id() -> &'static str; fn actions() }`, a static registry built in `crates/calternal/src/plugins.rs`, and `plugin-hello` as a 107-line example. No dynamic loading or out-of-process plugins exist.
  - `crates/client` (ADR 0072, the shared Rust client) is **not present** on `origin/dev`. That ADR is a decision, not code.
- **[H] Reusable with modest change:**
  - `fsio`, `netguard`, `memory`, `markdown-view`
  - the notes format and Markdown/Yrs pieces
  - the auth/WebAuthn core
  - the search index
  - the store's writer/journal pattern
  - the executor's idempotency logic
- **[H] Needs redesign:** the registry and plugin trait, the user-keyed runtime, and the sharing model (proposal §2/§7).
- **[H]** A TypeScript host reusing this code would need napi bindings, which means two languages in the host process plus FFI. Otherwise it would reimplement the code.
- **[R]** Calternal's repo memory records that an earlier Rust rewrite "never finished" and ended "slower than the TypeScript version" (`ops/orchestrator/memory/feedback-wasm-boundaries.md`). That is the main execution risk for a Rust host. The mitigation is to keep the host small and push product behaviour into TS plugins.

### 2.4 CLI harness orchestration (Codex, Claude Code)

- **[F] Codex app-server:**
  - JSON-RPC 2.0 over stdio JSONL by default. WebSocket is "experimental and unsupported".
  - It has an `initialize` handshake, `thread/start`/`resume`/`fork`, `turn/start` and `turn/interrupt`.
  - `codex app-server generate-json-schema` / `generate-ts` emit schemas matching the exact installed version.
- **[F] Claude Agent SDK:**
  - TypeScript and Python only. It "runs the Claude Code binary".
  - Other languages are told to "run the CLI as a subprocess" with `-p` and `--output-format json`.
- **[H] Split:** the Rust host owns supervision:
  - spawning the harness in its sandbox with only the authorised account's credentials
  - lifetime, cancellation, output capture and durable run records
  - restart and reconciliation

  The provider *adapter* runs in a worker and is replaceable per space (proposal §4). It translates normalized events, steering and handoff, and may use the TS Agent SDK there. The Codex protocol is language-neutral, so its adapter could be either. A TypeScript host would avoid one RPC hop for the Claude adapter. That advantage is real but small next to the sandboxing it still needs.
- **[F] Release gate, not a reopened decision:** the Agent SDK overview states, "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products, including agents built on the Claude Agent SDK." Record credential and distribution compliance for each provider path (Agent SDK, CLI subprocess, user-supplied accounts, CLIProxyAPI pooling) as a release gate, and validate it against current terms. The settled AGPLv3 decision is unaffected.

### 2.5 Durable server-side work

- **[F]** The existing store already has single-writer per-Home transactions and idempotency records committed with their events in one transaction (`crates/engine/src/lib.rs` module docs). ADR 0014 requires that no job blocks another.
- **[H]** Durable admission, run state, leases and the uncertain-outcome reconciliation (proposal §9) belong in the host. Workers stay disposable: they can be killed at any time, and their progress counts only when it is recorded through host operations with stable action ids. This holds for either host language. A Rust host gets it by extending code that already exists and is gated.

### 2.6 Operational complexity and plugin-worker overhead

- **[H]** Rust host plus TS workers means two runtimes in production: one static Rust binary, plus a pinned Node (or Deno) shipped in the image, a bundler and a sandbox launcher. A TypeScript host still needs the same worker fleet and sandbox for untrusted code. So the extra operational cost of a Rust host is mainly the second toolchain in CI and development, not a second fleet.
- **[H]** Worker overhead (cold start, idle memory, RPC round trip, workers per busy space) has **not** been measured. It dominates user-visible latency for plugin-backed operations in either design, so it is measured in V1 before anything else.
- **[H] Mitigations if it is too high:**
  - warm pools per space
  - snapshotting
  - coarse-grained operations: batch payloads that cross the process boundary instead of sending one call per item or per line

  Cross-process payload batching and end-to-end responsiveness are benchmark gates. JS is not assumed to be fast, and IPC is not assumed to be cheap. An embedded V8 or Wasm plugin runtime is out of v1 scope.

### 2.7 Swift native clients via UniFFI, and the browser

**Native (Swift + UniFFI + Rust):**
- **[F]** UniFFI exposes Rust async functions as Swift async. Per the UniFFI docs, "the foreign bindings supply the executor" and no Rust event loop is required.
- **[F]** Cancellation is not built in: "There's no builtin way to cancel a future", and UniFFI cannot raise Swift's `CancellationError`. The documented workaround is an explicit `cancel()` that sets a flag the library checks.
- **[F]** Async callback traits (Swift implementing Rust traits) are supported.
- **[H] Boundary:**
  - The Rust core is mostly sans-IO: state machines that emit effects and consume results.
  - Swift adapters own URLSession/WebSocket, Keychain, NSFileCoordinator and File Provider hydration, and app lifecycle (ADR 0072 open question).
  - Long operations take an operation id and return a cancel handle. Swift `Task` cancellation calls `cancel(op)`, the core stops at the next check, and server-side work is cancelled only by an explicit cancel operation.
- **[F] Portability audit item:** `calternal-core` depends on `tokio` (`sync`, `rt`, `time`), and `fsio`'s `rustix` code is gated to Linux/Android/Apple. A client core that uses tokio timers would bring its own runtime into the app, so client-core crates should avoid tokio-specific types.

**Browser (updated after the 2026-11-16 Wasm steering):**
- **[F]** The shipped web sync client is TypeScript Yjs over y-protocols (`packages/editor/src/sync/protocol.ts`), interoperating byte-for-byte with the Rust `yrs` server. No Rust runs in the browser today.
- **[F, follow-up source inspection 2026-11-16]** Offline note editing is already implemented: service-worker shell caching, per-note IndexedDB Yjs state, hide/close WAL recovery, cached-note reopening, reconnect/merge recovery, and offline preferences. See `apps/web/e2e/pwa.spec.ts`, `packages/editor/src/offline/local-copy.ts`, `packages/editor/src/offline/replay.ts`, `packages/editor/test/sync.test.ts` and `apps/web/e2e/prefs-offline.spec.ts`. Tests were inspected, not rerun. Reuse these paths with account/server/resource namespacing and space-grant adaptations; this does not establish first-ever offline startup, all uncached content, or arbitrary offline plugin actions. The standalone `action-outbox.ts` has no located production consumers.
- **[R]** Earlier measurements (`ops/orchestrator/memory/feedback-wasm-boundaries.md`, 2026-10-19, citing `calternal-research/wasm-bench/RESULTS.md`; not re-run here):
  - The per-keystroke gain was under 1.3 µs, with about 29% of Wasm time spent at the string boundary.
  - Wasm cold start was recorded as 240 ms in Chromium, 8.7 s in WebKit and 1.4 s in the iPhone simulator, with a 273 KB brotli size.
  - Only whole-file batches favoured Wasm.
- **[F]** UniFFI ships no Wasm bindings generator, and its Wasm feature is `wasm-unstable-single-threaded`.
- **v1 position (parent decision after user steering, 2026-11-16):** no browser Wasm. The browser editor and sync path stay TypeScript. Browser parity comes from shared contracts and conformance fixtures that the Rust core, the Rust server and the TS client all run.
- **The server stays the single sync and merge authority** (ADR 0045/0048). So there are not two independent sync authorities, just two client implementations checked against one oracle.
- Day-one code sharing is real for native, CLI and MCP (ADR 0072). For the browser it is contract-level.
- The user's Zellij experience is a responsiveness preference, not evidence that Wasm caused that slowness. Reopening browser Wasm later needs a measured end-to-end case, including WebKit startup.
- **None of this changes the host choice. [H]** A TS browser client works with any server language. A Rust host does let the server validate merges with the same format crates the native core uses.
- **[H] Engine replacement and native clients:** if a space installs a replacement engine, native clients might not have its implementation. They then follow the ADR 0048 open-question default: leave the bytes untouched, skip the derived local index, and defer merges to the server.

### 2.8 File-first content with protected authoritative state

- **[F]** Today: files are the truth and indexes can be rebuilt (ARCHITECTURE.md invariants). Every user-file write goes through `calternal_fsio::write_atomic`, and `beneath` confines a path to one Home.
- **[H]** In the new design, tenant workers never see host paths. They get brokered file operations scoped to their space's content tree, implemented with `fsio`.
  - Protected state (sessions, credentials, grants, membership, audit, run journals) lives in host-only storage outside any tenant-visible tree.
  - That state is authoritative and crash-consistent, not a rebuildable index (proposal §7).
  - Readable exports of portable configuration are fine, but writes to authority go through protected operations only.
  - This is the same in either host language. The Rust host has the existing, gated implementation.
- **[H] Account-local data.** Personal layout, Activity review state and credentials belong in a host-protected **account profile store**, not in an ordinary shareable space. Identity already requires that store. A "default space" pointer is not an isolation boundary, so private account data must not depend on it. This does not add a second user-visible space type.

## 3. Host and plugin boundaries

```
clients (web TS, Swift+UniFFI core, CLI/MCP) ──typed ops/sync──▶ Rust host
  Rust host: authn/session · grants/sharing · secrets · contract registry+validation
             · executor/idempotency · journals/run records · scheduler/leases
             · fsio broker · network egress broker (netguard) · sandbox supervisor
     │ capability RPC (versioned, JSON-Schema contracts; host stamps Caller/space)
     ▼
  per isolation-domain worker (space, plugin version, authority/credential context), started
     lazily, reaped when idle: Node LTS (or Deno), OS sandbox
     · plugin code only sees: ctx.ops.call(), ctx.files.*(scoped), ctx.net.fetch(allowed),
       ctx.accounts.handle(id) — never raw credentials, host paths or other spaces
  per-run harness process (codex app-server / claude CLI): OS sandbox, isolated
     working area, only the authorised account's credentials; supervised by host,
     driven by its adapter worker through host RPC
```

- **[H]** The host stamps the caller, space and grants on every RPC. A worker cannot name another space or raise its own authority. Manifests only request capabilities (proposal §5).
- **[H]** Every plugin action goes through the same executor as Rust actions, so idempotency, the journal and the isolation gate apply to it too.

## 4. Serious downsides of the recommendation

1. **Two languages in the server stack** (Rust host, TS plugins). Contract drift has to be prevented with generated schemas and conformance tests.
2. **Rust execution risk.** There is a recorded history of an unfinished Rust rewrite. Host changes have Rust build costs; for example, the workspace uses fat LTO and one codegen unit for release builds (`Cargo.toml`), and fast builds are a budget (ADR 0008).
3. **Re-keying cost.** The existing code is user-keyed and its plugins are compiled in. Space-keyed reuse is real work, not a lift-and-shift.
4. **Worker overhead is unknown.** If cold start or idle memory per worker is high, many spaces with many plugins will be expensive.
5. **Agent SDK mismatch.** The Claude Agent SDK is TS/Python only. Rust supervises the process while TS drives the session, which adds an RPC hop and a split failure domain.
6. **Sandbox inside rootless Podman** is unverified and may force a deployment change.

## 5. What would change the recommendation

Switch to a **TypeScript/Node main server** (keeping the same out-of-process sandbox for untrusted plugins) if:
- the re-keying spike (V2) shows the reusable Rust surface is small, or that adapting it costs about as much as writing the host in TS
- most first-party product logic turns out to need fine-grained, in-process calls into the host that RPC makes too slow or too chatty, and that logic is trusted
- the team decides it will not maintain Rust beyond the native client core

Reconsider the **worker runtime**:
- Deno instead of Node if V1 shows simpler, equally safe sandboxing and npm compatibility for the Agent SDK.
- If per-process overhead fails the budgets even with warm pools and batching, the next step is a coarser worker granularity within one tenant's trust domain, never mixing tenants. A different plugin runtime would be a separately justified, post-v1 decision.

Browser Wasm is out of v1 (§2.7). Reopening it would not change the host.

## 6. Responsiveness gates

Responsiveness is a design and release gate, not later polish (user requirement, 2026-11-16). The interaction and animation target is 120 Hz, a frame interval of about 8.33 ms, on displays that support it. On 60 Hz hardware the limit is its real refresh, about 16.67 ms. "No lag" is the goal the gates enforce; it is not an unconditional guarantee, and nothing here has been measured yet.

**[F] Existing baseline to build on:**
- ADR 0005 makes perf a merge gate: a dedicated quiet perf VM, the median of several runs, a noise band per metric, and regressions block the merge.
- `perf/budgets.json` already gates the server: `cold.startup`, `open.note`, `sync.save_ack`, `journal.catch_up`, `mem.idle`, `idle.cpu_pct`, `idle.wakeups_per_s`, `storm.*`, among others.
- It also gates the web client through `perf/web.sh`: `web.chromium.type.*`, `scroll.*`, `boot.cold_tti`, `long_task_max`, `missed`, `blank_frames`.
- Those web gates use Chromium with its CPU slowed 4x. WebKit is reported but not gated, because on Linux it draws the glass in software (`docs/perf/web.md`).
- The proposal's palette gates (§11) carry over.
- **Regression rule:** where calternal-ai overlaps existing Calternal (notes typing, scrolling, list search, boot, sync acknowledgement, idle), the same scenarios must not regress against an `origin/dev` baseline on the same machine.

**Metrics.** Each is recorded as a distribution (p50/p95/p99/max) per scenario, on both web and native:

| Area | What is measured |
| --- | --- |
| Input to paint | Per interaction type: editor keystroke, palette keystroke, sidebar app switch, pane open and close, send message, approval tap, cal handoff. Split into input delay, processing and presentation. |
| Frame deadlines | During animations and scrolling (sidebar transitions, Activity, conversation streaming, history, Collections): frames over the device's real refresh interval, the longest frame, and hitch time per second. The actual refresh rate is recorded from frame timing, never assumed. |
| Main-thread work | Tasks over 8 ms and over 4 ms inside interaction windows, plus total blocking time. Long tasks over 50 ms are a failure, not the threshold. |
| Startup | Cold and warm for the server, the web app (time to interactive, first note) and native launch to first interactive frame. |
| Search | Palette first results, parser time per keystroke, backend p95 at 10k and 100k (proposal §11). |
| Live rendering | Streamed conversation tokens coalesced per frame, history loading and scrollback, Collections with large live membership, multi-space Activity feeds. |
| Memory and idle | Client heap growth over a soak, server RSS, per-domain worker RSS, idle CPU and wakeups with many installed but idle plugins. |
| Acknowledgement | Time from provisional to confirmed, kept separate from input to paint. |
| Under load | The same UI traces while cals and harnesses stream, plugin domains are busy, indexing runs and other spaces generate sync storms. |

**Durable acknowledgement.**
- Optimistic UI is explicitly provisional: it is visibly pending and reconciles when the durable acknowledgement arrives.
- The existing editor protocol already refuses to claim saved before the server's durable acknowledgement (messages `100`/`101` in `packages/editor/src/sync/protocol.ts`). That rule is kept for every operation, including plugin and cal actions.
- Fast paint never means a fake confirmed write.

**Architecture rules the host enforces [H]:**
- **Interactive and background lanes.** Interactive operations are scheduled ahead of plugin, model, index and harness work. Background work runs in yielding batches with per-space fairness (ADR 0014). No model, index or plugin-heavy work sits on an interactive request path unless it is declared and budgeted.
- **Coarse operations.**
  - One RPC per user action or batch across the client→host and host→worker boundaries, never one per item or per line.
  - Client presentation updates are coalesced at most once per frame. Durable events and action outcomes are still recorded and processed individually; rendering batches must not drop history, authority changes or acknowledgements.
  - Backpressure, not buffer growth.
- **Off the UI thread.** Expensive client work (parsing large files, indexing, layout of large lists) runs off the main thread or is virtualised. Model and plugin work always runs on the server.
- **End-to-end tracing.** Request, run and action ids (the existing `RequestId`, extended) propagate client → host → worker → harness. Spans are emitted at each hop so a slow frame can be traced to its server cause. Instrumentation stays on in perf builds and is sampled in production.

**Datasets.**
- Existing `calternal-fixtures` Homes at 10k and 100k notes.
- New fixtures: multi-space (several members, shared and narrowly shared resources), long conversations with attachments, large Activity feeds across spaces, large live Collections, many installed plugins with few active.
- Fixture sizes are written into the gate spec before testing.

**Device matrix.** Categories to fill in with an exact, versioned list:
- 120 Hz Apple: ProMotion iPhone, iPad and MacBook, running the native app and Safari.
- 120 Hz or faster non-Apple displays with Chromium and Firefox.
- The oldest supported phone and 60 Hz laptops, judged against their real refresh.
- CI proxies: Chromium with 4x CPU slowdown, and WebKit on the perf VM. These are proxies, not substitutes for real devices.

Every result records the device model, OS and browser version, measured display refresh, power source and low-power mode, thermal state at the start and end, and background load. Runs are repeated under warm thermal conditions, not only from cold.

**To specify before testing** (not invented here): the exact supported-device list, percentile budgets per metric and device class, noise bands, and fixture sizes.

Packaging recommendation: central server as an OCI image for rootless Podman; execution machines use a separate native daemon and outbound authenticated transport. See the proposal’s distribution section. This does not settle the worker sandbox topology; V1 must prove it without exposing a general host container-engine socket.

## 7. Small discriminating validation plan

Each step has a pass/fail criterion set in advance. No numbers are assumed. The [validation plan](validation-plan.md) supplies ordering, native start gates and the recovery fault matrix.

- **V1. Worker sandbox, lifecycle and responsiveness under load.** Use a Rust host stub, the production container image, the target kernel and a designated benchmark host (allocation still to be confirmed).
  - Launch TS plugin workers under a uid or user namespace, Landlock, seccomp, a cgroup and no network, once with Node LTS and once with Deno.
  - **Lifecycle measurements** (distributions, not single numbers):
    - cold start when a user interaction triggers a call, from host RPC sent to first result
    - warm RPC p50/p95/p99, both for one call and for one batched call carrying many items
    - idle RSS per warm domain
    - reap and restart cost
    - the maximum number of warm domains within the host memory budget
    - host idle CPU and wakeups with N installed but idle plugins, against the existing `idle.*` budgets
  - **Isolation and granularity test:** a privileged requester calls a plugin that keeps state. Then a less-privileged requester, or one using a different account binding, calls the same plugin. Check that nothing from the first call is reachable from the second (module state, caches, temporary files, environment, open handles).
    - Measure each candidate pooling policy against the budgets: identical-authority pooling, stateless reset between calls, and per-request processes for privileged contexts.
    - Choose the cheapest policy that passes the leak tests. Never merge domains across tenants or authority contexts to save memory.
  - **Under-load test:** run §6 interactive UI traces (typing, scrolling, app switch, palette) while K domains are busy, M harness turns stream output and indexing runs. Compare against the same traces with no load. Interactive operations must stay within the noise band and their budget, and background work must yield (ADR 0014).
  - **Escape tests:** host paths, `/proc` of other processes, `process._debugProcess`, ptrace, environment secrets, raw sockets, another space's or principal's ids, and data cached by a domain leaking to a principal with less access.
  - **Pass:** escape tests fail closed, and every distribution meets budgets set before the run. If a cold start on an interaction path exceeds its budget, the design needs pre-warming or a visibly provisional UI on that path. It is a failure if the UI waits silently.
- **V2. Re-keying spike, judged by existing budgets.** On a branch of the existing crates, key `engine` and `store` by space, with membership checks. Port the isolation gate to two accounts in two spaces, plus one shared resource.
  - Run the existing server perf suite on the designated benchmark host against an `origin/dev` baseline: `open.note`, `sync.save_ack`, `sync.round_trip`, `journal.catch_up`, `storm.*`, `mem.idle`, `idle.*`, at 10k and 100k notes. Add a new multi-space fixture.
  - **Outcome:** the actual diff, the tests broken or kept, any regression outside the noise band, and the remaining unknowns.
    - Reuse is economical only if the re-keyed code keeps the existing gated budgets.
    - A TypeScript host would have to meet the same budgets from scratch. That is the concrete bar for §5's switch criterion.
- **V3. Dynamic plugin slice** (proposal §12 step 3).
  - Install, preview, activate, upgrade and roll back a TS plugin in space A, without restarting the host.
  - Its action goes through the executor (idempotency and journal) and reads a file through the broker.
  - Space B cannot reach it. The isolation gate enumerates the plugin's actions.
- **V4. Harness supervision.**
  - The host spawns `codex app-server` (stdio) and `claude -p --output-format stream-json` in sandboxes, with adapter workers driving them.
  - Verify `turn/interrupt` and the Claude interrupt, host restart in the middle of a turn, and that reconciliation does not re-run a recorded external action.
- **V5. Native and browser client boundaries** (shared with the client-core work).
  - Build a minimal Swift harness over UniFFI that calls one async operation, with a Swift `Task` cancel mapped to `cancel(op)`.
  - Run a TS browser client and the Rust core against the same conformance fixtures (operation, conflict, reconnect). The server is the single merge authority.
- **V6. Engine replacement.** A space installs a TS replacement for one engine contract (for example a simple index engine). Verify:
  - it commits only through the broker
  - attempts to write outside its scope or skip the version check are refused
  - other spaces keep the default Rust engine
  - native clients degrade as §2.7 describes

V1 and V2 are the steps that discriminate between host choices: V1 holds for both hosts, and V2 measures the reuse argument. V3–V6 validate the boundaries.

## 8. Authoring loop under this recommendation

1. Template → edit Markdown, config, TS and declarative views (personally or by a cal).
2. The host runs the bundler and type checker against the generated SDK types and validates the manifest and requested capabilities.
3. Preview worker in the user's space, with errors mapped to source.
4. Activate: pin the version, check grants and migrations, keep the last working version.
5. Upgrade or roll back without restarting the host.

Authors need no Rust toolchain. Rust changes are only for host and foundation work.

## 9. Node: main server or worker runtime?

**Worker runtime.** Node, pinned in the image, runs:
- sandboxed tenant and first-party product plugins
- provider adapters, including the TS Agent SDK

It does not hold sessions, grants, secrets or the authoritative journal. If §5 triggers a switch, Node becomes the main server and still needs the same separately sandboxed worker processes for untrusted code.

## 10. Defaults and remaining decisions

**Defaults this report recommends:**

| Area | Default |
| --- | --- |
| Main server | Rust host (axum/tokio), extending existing `fsio`, `store`/`engine` patterns, auth and search after re-keying to spaces |
| Trusted broker | Rust, never tenant-replaceable: authn, grants and sharing, secrets, storage commit, journal and idempotency, sandbox, scheduling |
| Semantic engines | Default implementations are compiled Rust (Notes format, merge and index, search). Spaces may replace them through versioned engine contracts, committing via the broker |
| Product and tenant plugins | TypeScript, out of process, OS-sandboxed workers per isolation domain (space, plugin version, effective authority/credential context), lazy and reaped when idle. Pooling policy is decided by V1. Node LTS by default, Deno tested |
| Harnesses | Host-supervised sandboxed processes. Provider adapters are replaceable TS workers |
| Browser | TypeScript editor and sync, no Wasm in v1. Parity via conformance fixtures, server is the merge authority |
| Apple | Swift UI plus Rust core via UniFFI, with explicit operation ids and cancel handles, and Swift-owned effects |
| Account-local data | Host-protected account profile store, not a shareable space |

**Decisions still open:**

1. **Engine fork depth — resolved by the user.** Product layers are immediately customizable; compatible engine contracts open incrementally, starting with Notes, after V6 passes. The default engine remains available as a service. All non-foundation behavior remains a plugin-API replacement target.
2. **Responsiveness spec (team-owned, before V1).** The exact device list, percentile budgets per metric and device class, noise bands, fixture sizes, and worker cold-start, idle-memory and density budgets (§6).
3. **Sandbox mechanism inside the deployment container (engineering).** Decided by V1. If it fails inside rootless Podman, the deployment shape is reopened.
4. **Provider credential and distribution compliance (release gate).** Covers each Claude and Codex path (§2.4).
5. **Host switch trigger.** If V2 shows little economical Rust reuse, re-run this assessment in favour of a TS main server with the same sandboxed workers.

## 11. Sources

**Inspected read-only:** `/home/kayg/Developer/calternal`, `origin/dev` @ `ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (2026-11-15)
- `ARCHITECTURE.md`; `Cargo.toml`; `rust-toolchain.toml`
- `crates/*/Cargo.toml`, and line counts of `crates/*/**/*.rs`
- `crates/core/src/plugin.rs`, `crates/core/src/action.rs`
- `crates/engine/src/lib.rs`, `crates/engine/src/execute.rs`
- `crates/store/src/lib.rs`, `crates/fsio/src/lib.rs`, `crates/fsio/Cargo.toml`
- `crates/calternal/src/plugins.rs`, `crates/calternal/src/isolation.rs`
- `packages/editor/package.json`, `packages/editor/src/sync/protocol.ts`, `apps/web/package.json`
- `deploy/` file list
- ADRs 0005, 0009, 0010, 0011, 0013, 0014, 0048, 0072
- `perf/budgets.json`, `docs/perf/web.md`
- `ops/orchestrator/memory/feedback-wasm-boundaries.md`
- Absence of `crates/client`, `uniffi` and `wasm-bindgen` in the tree, checked with `git ls-tree` and `git grep`.

**Official documentation, fetched 2026-11-16:**
- Node permission model: https://nodejs.org/api/permissions.html
- Node `vm` warning: https://nodejs.org/api/vm.html
- Node TypeScript type stripping: https://nodejs.org/api/typescript.html
- Deno security: https://docs.deno.com/runtime/fundamentals/security/
- Linux Landlock: https://docs.kernel.org/userspace-api/landlock.html
- UniFFI async and cancellation: https://mozilla.github.io/uniffi-rs/latest/futures.html
- UniFFI Wasm configuration: https://mozilla.github.io/uniffi-rs/latest/wasm/configuration.html
- Codex app-server (redirected from developers.openai.com): https://learn.chatgpt.com/docs/app-server
- Claude Agent SDK overview (redirected from docs.claude.com): https://code.claude.com/docs/en/agent-sdk/overview
