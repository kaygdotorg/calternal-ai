# Chat first: the build plan · 2026-11-27

**Goal (round 34):** the shortest ordered list of build slices that ends
with a person talking to a Claude Code agent in Chat, in the browser, on the
new host on `calternal-dev-01-compute02`. Chat is the test of the host and
the plugin API, so every slice states what it proves about them.

**Inputs:**

- the format at revision 9
  ([kind-and-collection-format-2026-11-24.md](kind-and-collection-format-2026-11-24.md));
- the Chat plugin files under `plugins-v2/calternal-chat/` and
  `plugins-v2/calternal-chat-claude-code/`, which are the test vectors for
  the parser and the host.

Everything here was checked against the branches named, at their tips on
2026-11-27.

**Out of scope for Chat:**

- Money, Mail, Calendar and CalDAV/CardDAV;
- `extends:`, places other than Chat's two slots (composer, harness),
  themes and space profiles;
- outside addresses, `listens`/`on.after`, CalDAV mapping and native apps;
- sharing a chat with other people. A chat between people is slice 9,
  after the goal;
- the offline outbox. The browser is online for the goal; S1's "not sent
  yet" comes after.

## What exists to reuse

| Piece | Where (looked at) | Use |
| --- | --- | --- |
| Durable file writes, exclusive placement, fsync discipline | `crates/fsio` on `dev` (4.5k lines) | The host's file layer, unchanged |
| IDs, caller-safe error, time and hash helpers | `crates/types` on `dev` | Shared types (review gate item 1) |
| Sign-in, sessions, spaces' stored authority | `crates/auth` on `dev` (12.8k lines) | Sign-in for the browser; the one check's "who" |
| Secrets, opaque handles | `crates/secrets` on `dev` | The harness's provider sign-in stays the machine's (see slice 6); nothing else in Chat needs secrets |
| Item versions, per-space history, durable request publication | `crates/content` on `dev` (15.6k lines) | History ledger and receipts, where they fit the new item model (slice 2 decides, piece by piece) |
| Call route, call table, `authorize`, record append/delete, outside-change detection, index, end-to-end tests | `experiments/host-spike/` on `lane/spike-host` (`39016818`, 9.3k lines, own workspace) | The seed of the new host crate; its findings (hashes only cannot restore a changed record body) are already in the format |
| Record files: append flat at 2 flushes, no monthly segments | `perf/spikes/` on `lane/spike-records` (`1cf1172d`) | The record writer's measured shape |
| Format engine: parser, loader, dates, engine, Wasm and native benches | `spikes/engine-wasm-vs-ts/rust/` on `lane/engine-wasm-vs-ts` (`5e113206`) | The parallel engine lane's starting code |
| Plugin worker session (newline JSON frames, single-worker state machine) | `crates/plugin-host` on `dev` (`runtime.rs`, 5.2k lines) | Runs `operations.ts` in its own process; the frames carry the 14 calls |
| Plugin SDK (TypeScript) | `packages/plugin-sdk` on `dev` | Rewritten to the 14 calls; the transport stays |
| Machine runner, sandboxes (direct, podman, microVM with snapshots, seccomp worker) | `crates/machine` on `dev` (13.3k lines); microVM measured on `lane/vm-measure` | Runs the harness for `s.run`; podman first, microVM when slice 6 passes |
| Claude Code adapter (`claude -p --output-format stream-json`, resume, stop by SIGINT) | `plugins/calternal-claude-code/` on `dev` | Becomes `calternal-chat-claude-code`'s program, unchanged where possible |
| Chat UI pieces, records view drawn from declarations on web and iPhone | `apps/web/src/apps/threads/` on `dev`; `experiments/drawing-spike/` on `lane/spike-drawing` (`bd880283`) | The browser client's Chat window and the generic records component |
| Earlier v2 host lanes (`lane/v2-host-*`, `lane/api-v2-step0`, `lane/plugin-api-v2*`) | revision-14 contract, JSON manifests | **Not reused**: written against the contract the primitives-only host replaced. Read for ideas only |

## The slices

Each slice ends in a test that runs on the dev VM under the gate lock rules
(`~/bin/lock-full` or `lock-shared`, `flock -o`). The order is the
dependency order; §3 says what can run side by side.

### Slice 0. The format engine (the parallel lane, already starting)

- **Builds:** `crates/format`, a pure crate with no `tokio`, `rusqlite`,
  `regex` or time zone database (ninth set). It covers:
  - the line grammar (L1–L10, names, reserved words);
  - kind, collection, sidebar, `plugin.md` and agent-rules files;
  - item YAML;
  - checks, formulas and queries over items held in memory.
- **Reuses:** `spikes/engine-wasm-vs-ts/rust/` (`parse.rs`, `load.rs`,
  `engine.rs`, `dates.rs`).
- **Proves:** that the format is buildable from the document alone, with
  `plugins-v2/calternal-chat/` and the format's Appendix B as the test
  vectors. Every file parses, and every refusal names its line.
- **Leaves out:** `per` fields, `totals:` with `all()`, `lines:`, `ical:`,
  `extends:`, themes and profiles. Chat uses none of them; they come as
  later engine tasks.
- **Test:** `cargo test -p calternal-format`. Every file under
  `plugins-v2/` parses, and each of Appendix A's twenty-two inputs gives
  the result the table states.
- **What the engine lane should wait for**, everything else being stable
  at revision 8:
  - revision 9's `rights: read …` line and the `shared with me` source;
    Chat uses neither, so they can come last;
  - the `session()` scope, settled in revision 9 §2.20 and §8.1 as a run's
    `scope: session` value. It is not a formula function, so the parser
    does not change.

### Slice 1. The host skeleton: spaces, install, items

- **Builds:** `crates/host`, binary `calternal-host`, grown from
  `experiments/host-spike`. It has:
  - one personal space per person;
  - install of a plugin folder into `.calternal/plugins/<plugin>/`,
    checked by `crates/format`;
  - items as Markdown files in their kind's folder;
  - `s.write` with `add`, `change`, `remove`, all or nothing;
  - the index, rebuildable, and `s.query` for one kind with `where`,
    `sort` and paging;
  - the call route with the one check.
- **Reuses:** `fsio`, `types`, and `auth`'s session check. The spike's
  `call.rs`, `files.rs`, `query.rs` and `fm.rs` are rewritten on
  `crates/format`.
- **Proves:**
  - a plugin is data first: Chat installs with no code running;
  - an item is a Markdown file;
  - one call route serves the app, the CLI and the tests.
- **Leaves out:** records, live windows, other spaces, members, WebDAV and
  rights lines.
- **Test:** install `plugins-v2/calternal-chat/`. Add a chat with the
  built-in `add`. Query `collections/chats.md`, rebuild the index, and get
  the same answer. The file on disk is the expected Markdown.

### Slice 2. Records and the ledger

- **Builds:**
  - record files (§2.8 of the format): `append record`, `change record`
    within its window, and `remove record`;
  - the history ledger with author stamps, and header lines written by
    the host;
  - receipts with idempotency keys, kept 7 days (review gate item 7);
  - `state:` keys per person, and `s.state`;
  - formulas over `records` and `state.` (an unread count), computed per
    viewer.
- **Reuses:** `experiments/host-spike/src/records.rs`, the measured append
  path from `lane/spike-records`, and `crates/content`'s history where its
  item model fits. The lane writes down which parts it kept.
- **Proves:**
  - records are generic: nothing in the host says "chat";
  - authorship cannot be forged;
  - an append costs 2 flushes;
  - per-person state lives in the person's own space.
- **Leaves out:** outside (WebDAV) record edits beyond "refused" and the
  three-way merge (§9).
- **Test:**
  - two people append to one chat file;
  - a forged header over the disk is stamped `outside`;
  - `remove record` by a non-author is refused;
  - 1,000 appends show flat flushes under `count-flushes.sh`.

### Slice 3. Coded operations: the plugin process and the calls Chat uses

- **Builds:**
  - `operations.ts` running in its own process per plugin and space,
    started on demand and stopped when idle;
  - the frames for these calls: `s.query`, `s.read`, `s.write`, `s.state`,
    `s.call`, `s.run` with its handle, and `s.me`/`s.space`/`s.event`;
  - the events `on.call` and `on.change`;
  - the one error shape; event keys and replay.
- **Reuses:** `crates/plugin-host`'s worker session and frames;
  `packages/plugin-sdk`, rewritten to these calls.
- **Proves:**
  - the call surface is enough for a real plugin: Chat's `send` and
    `rename` are written in TypeScript against it;
  - the host never reads plugin code, it only routes calls;
  - a crash in the plugin is the plugin's error, not the host's.
- **Leaves out:** `s.fetch`, `s.connect`, `s.notify`, `s.job`, `s.share`,
  `s.cache`, `s.outside.mint`, `on.time`, `on.outside`, `on.connection` and
  `on.after`.
- **Test:**
  - Chat's `send` appends a record through `s.write`;
  - a replay with the same key writes nothing;
  - killing the worker mid-call leaves no half write;
  - a call to an undeclared operation is `not-allowed`.

### Slice 4. The live window

- **Builds:** a subscription over a query (collection rows, or one item's
  records from an anchor), pushed to the browser over one WebSocket per
  session as appends and changes. Writes made through the route show in
  every open window within one frame.
- **Reuses:** the spike's index; the browser end is written new.
- **Proves:** "one live window" is enough for a chat transcript and a
  chat list, with no chat-specific push.
- **Leaves out:** devices, the offline outbox and the sync journal.
- **Test:** two browser contexts on one chat. An append in one shows in
  the other, and the order survives a reconnect from the last anchor.

### Slice 5. The browser client, drawn from declarations

- **Builds:**
  - sign-in;
  - the shell with one sidebar: Chat's `sidebars/chat.md`;
  - a list window for a collection;
  - the records view for a chat, with the composer slot filled by
    `calternal-chat`'s own composer;
  - operation buttons and menus from the operations list.
- **Reuses:** `apps/web` (its shell, `apps/threads/` and the Svelte
  components), and `experiments/drawing-spike/web`'s records component and
  declaration reader.
- **Proves:**
  - plugins never draw: the whole Chat window comes from kind,
    collection, sidebar and records declarations, plus one composer;
  - nothing in the client names Chat.
- **Leaves out:** themes, places other than the composer, native apps and
  settings beyond sign-out.
- **Test:** Playwright on the dev VM. Sign in, make a chat, send a
  message, rename the chat. A screenshot goes on the issue, never into the
  repository.

### Slice 6. Agent runs: `s.run` and the first harness

- **Builds:**
  - `s.run(program, { scope, targets })`, backed by `crates/machine`'s
    runner in a podman sandbox on the dev VM;
  - the run's log, `send` (input from the run's actor only) and `stop`
    (anyone who may read the target), and the process family stopped
    together (G4, G8);
  - scope `session` (revision 9): the run reads the current chat session
    only;
  - the harness plugin `plugins-v2/calternal-chat-claude-code/`, whose
    program is the existing Claude Code adapter;
  - the reply appended as a record by the harness through the host, with
    partial text in the run log for the live window.
- **Reuses:** `crates/machine` (runner, podman sandbox, `provider_home`)
  and `plugins/calternal-claude-code/` (adapter, Containerfile). The
  microVM path in `crates/machine/src/sandbox/microvm/` comes after the
  goal.
- **Proves:**
  - an agent is a plugin;
  - the host knows nothing about Claude;
  - Stop works;
  - a run can only read its scope.
- **Leaves out:** agent rules beyond R1's personal-space default (free
  except reaching people, and a personal chat reaches no one), microVMs,
  machines a member offers, and other harnesses.
- **Test:**
  - with a fake provider (a script that echoes), the reply arrives as a
    record;
  - Stop ends the process family within two seconds;
  - a run asked to read another chat gets `not-allowed`;
  - then once with the real `claude` on the dev VM, signed in by the owner
    through the adapter's own sign-in. This step is marked manual.

### Slice 7. The goal, end to end

- **Builds:** nothing new. This slice wires slices 1 to 6 under one
  `calternal-host` on the dev VM, served on one port of the dev VM
  (which port is picked when the slice starts).
- **Proves:** the plugin API carries a real app. A person opens Chat in
  the browser, types, picks Claude Code, sees the reply stream in and
  stops a run.
- **Test:** a Playwright script ends in a reply record by the harness with
  the run's ID, and the files on disk are readable Markdown. Screenshots
  go on the issue.

### After the goal (in order, not part of it)

8. **Agent rules files and prompts** (§10 of the format): Allow once, Allow
   for this run, Always allow.
9. **A chat between people:** a shared chat in the starter's personal
   space, `s.share` from a live gesture, and `s.notify` with templates.
10. **The offline outbox** ("not sent yet", "not accepted") for the
    browser.
11. **The microVM sandbox** in place of podman, and machines a member
    offers.
12. **More harnesses** (Codex, Antigravity), each a plugin.

## 3. What can run side by side

| Lane | Touches | Can start | Waits for |
| --- | --- | --- | --- |
| A: slice 0, the engine | `crates/format` only | now | nothing |
| B: slices 1 and 2, the host core | `crates/host` | now, on a stub of `crates/format`'s API agreed with lane A in the first hour | lane A's parser for real files |
| C: slice 3, the plugin process | `crates/plugin-host`, `packages/plugin-sdk` | now, against a mock host speaking the frames | B's call route to run for real |
| D: slice 6, runs and the harness | `crates/machine`, `plugins-v2/calternal-chat-claude-code/`, `plugins/calternal-claude-code/` | now, with a fake provider | C's `s.run` frame, and B's records for the reply |
| E: slices 4 and 5, the browser | `apps/web` (a new Chat-on-host entry), a small `crates/host` module for the WebSocket | after B's query route exists; mock data before | B |
| Integration: slice 7 | none new | when A to E pass their tests | all |

Lanes B and E both touch `crates/host`. E owns one module (`live.rs`) and
nothing else, so their files never overlap.

## 4. Decisions this plan makes (for the coordinator to record)

- **The new host is a new crate, `crates/host`,** grown from the spike. The
  old `crates/server` is not extended: it carries the revision-14 contract
  and native Chat. It stays as it is until the host replaces it.
- **Podman first, microVM after the goal.** The microVM sandbox exists on
  `dev`, but the goal needs one harness answering, and podman is what the
  Claude Code adapter's Containerfile already targets.
- **Streaming replies go through the run log,** not through records
  changed per token. The reply becomes one record when the turn ends, so
  records keep their 2-flush append, and the transcript never holds
  half-written messages.
- **One WebSocket per browser session** carries every live window. It is
  generic, so it is not a chat channel.

## Simplification opportunities

- **Retire the revision-14 lanes.** `lane/v2-host-execution`, `-items`,
  `-registry`, `-integration`, `-harness`, `lane/api-v2-step0`,
  `lane/plugin-api-v2`, `-combined`, `-rev14-draft` and `-step1` all target
  the contract this host replaces. Archiving them removes about 300
  fixture files from every diff against `dev`.
- **Drop the old adapter's private sign-in mode** if the harness plugin
  can use the host's sign-in sheet (G12) once slice 8 lands.
- **Fold `apps/web/src/apps/threads/`'s native-chat code** into the
  generic records view once slice 5 passes, rather than keeping both.
