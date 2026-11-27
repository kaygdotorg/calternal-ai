# Browser for Chat on the new host (slices 4 and 5) · 2026-11-27

Lane E ([Forgejo #118](https://git.kayg.org/kayg/calternal-ai/issues/118)).
References:
[Build plan](../proposals/chat-first-build-plan-2026-11-27.md) ·
[Primitives-only host](../proposals/primitives-only-host-2026-11-21.md) ·
[Kind and collection format r9](../proposals/kind-and-collection-format-2026-11-24.md) ·
[Host call route](host-call-route-2026-11-27.md) ·
[Live window wire protocol](live-window-protocol-2026-11-27.md).

## Lane E first pass

- **Slice 4 (live window):**
  - Wire protocol: [live-window-protocol-2026-11-27.md](live-window-protocol-2026-11-27.md).
  - Host module: `crates/host/src/live.rs` wired to `GET /v1/spaces/{space}/live` on `calternal-host`, with per-space bounded broadcast channels (`Hub`), `auth` handshake, collection/query subscriptions, and record subscriptions with `after` anchor delta catch-up and lag recovery.
  - Browser client: `LiveClient` in `apps/web/src/host/client.ts` (one WebSocket per browser session, auto-reconnect resuming each open records view from its `lastAnchor`).
- **Slice 5 (declaration-driven browser entry):**
  - New `/host` entry in `apps/web/src/main.ts` mounting `apps/web/src/host/HostApp.svelte` (all existing app entries left intact).
  - Zero plugin- or Chat-specific strings in `apps/web/src/host/`: sidebar, collection list, item records view, composer slot, and item/record operation menus and forms are generated strictly from the declarations returned by the host.
  - Reused `@calternal/ui` tokens and components (`Backdrop`, `Card`, `Button`, `Callout`, `Icon`, `Menu`, `Sheet`) and adapted `Records.svelte` and `FormSheet.svelte` from `experiments/drawing-spike/web` (`origin/lane/spike-drawing`) to the approved `apps/mockup` (`origin/lane/threads-ui`) bare-canvas look.

## Gaps found in the Lane E first pass

| Section | What the screen needed | What we did meanwhile |
| --- | --- | --- |
| `host-call-route-2026-11-27.md`, Format §1.1 | A host call or route returning the space's installed plugin declarations (manifests, kinds, collections, sidebars, slots/fills) so the browser can draw from them. `crates/host` currently serves only `POST /v1/spaces/{space}/call` for data calls. | Served `GET /v1/spaces/{space}/declarations` from the local dev/test server (`apps/web/scripts/host-mock.mjs`) by parsing `plugins-v2/calternal-chat/*.md` directly from disk. |
| `crates/host/src/main.rs` | Browser sign-in / sign-out HTTP endpoints returning a bearer session and personal space UUID. `calternal-host` currently expects a pre-issued bearer token. | Added `POST /v1/auth/session` and `DELETE /v1/auth/session` on the local dev/test server. |
| Format §2.20 (`operations:`) | Input parameter names and types for coded operations (`rename: code, menu`, `send: code`). `kinds/chats.md` lists no inputs, while `operations.ts` expects `{ chat, title }` and `{ chat, text, quote? }`. | Derived single-field forms from the kind's primary text field (`title`) and passed both `item` and the singular kind ID (`[kind.id.replace(/s$/, "")]`) so no plugin vocabulary enters the client. |
| Format §7.4 (`slots:` / `fills:`) | Mapping from a slot fill (`calternal-chat:composer: operation send`) to draft persistence (`state.draft`), text input key, and optional quote input key. | Rendered the generic composer whenever a plugin fills a slot with an `operation`, bound the text input to `text`, enabled quoting when `quote` is declared in `kind.records`, and persisted drafts per item in `localStorage`. |
| `host-call-route-2026-11-27.md`, Format §2.9 (`state:`) | Per-viewer `state` keys (`state.pinned`, `state.muted`) on `s.query` / `s.read` rows so the client can evaluate `when` predicates (`pin: ..., when not state.pinned, menu`). | Returned `state` on query rows in the mock and checked `row.state?.[k] ?? row.computed?.["state." + k]` in `apps/web/src/host/format.ts`. |
| `docs/native-ui-study.md` & `docs/ui-craft-and-settings-study.md` | Exact numbers for record bubble max-width, composer inset margins inside a window card, and bottom-centre strip padding (not measured in either study). | Followed `apps/mockup` (`origin/lane/threads-ui`) values (`max-width: min(82%, 38rem)`, `--s-5` desktop / `--s-4` phone margins, `--tier-cap: 48px`) instead of inventing new numbers. |

## Lane F host integration

- Browser passkey setup and sign-in use `calternal-auth`, and session status returns the current account and personal space. The strict cookie is checked before HTTP mutation or WebSocket upgrade. Sign-out revokes the installation and closes its live socket. The first-frame token path was removed.
- `s.declarations` projects the installed format catalog into typed inputs, placement and other browser metadata. It exposes collections that this personal-space host can query. The row renderer returns declared viewer state and operation-name booleans in `when`. Both WebSocket query rows and live record metadata carry the evaluated values.
- A live invalidation for private state is queued only for that account. Outgoing rows and records pass through the same authorized reads as the call route. The tests cover a second viewer's private queue and a second account attempting to read the personal space.
- `item` means a required UUID link to the operation's own kind. Harness `start` is a targetless operation on the run kind, so its `chat` argument still names the chat the harness should use.

### Item input fixture changes

- [`plugins-v2/calternal-chat/kinds/chats.md` lines 28–39](../../plugins-v2/calternal-chat/kinds/chats.md#L28-L39) declare the item and the typed rename and send parameters.
- [`plugins-v2/calternal-chat/operations.ts` lines 4–44](../../plugins-v2/calternal-chat/operations.ts#L4-L44) pass the declared item through the rename and append handlers. The optional harness call still receives the chat as its own input.
- [`crates/host/tests/coded.rs` lines 119–208](../../crates/host/tests/coded.rs#L119-L208) cover the fixed key, a foreign or missing target, send replay and rename. The relay, crash and harness cases are updated at [lines 234](../../crates/host/tests/coded.rs#L234), [335](../../crates/host/tests/coded.rs#L335) and [459](../../crates/host/tests/coded.rs#L459).
- [`crates/format/tests/fixtures.rs` lines 443–518](../../crates/format/tests/fixtures.rs#L443-L518) exercise compiled coded inputs, and the slot and refusal fixture at [line 558](../../crates/format/tests/fixtures.rs#L558) uses the revised Chat input declaration.

### Browser lane follow-up

- Apply the browser logout lifecycle from ADR 0030, including pending local changes and clearing account-scoped client data across tabs. The host revokes the installation, expires the cookie and requests cache clearing.
- Replace the preview name field in `HostSignIn.svelte` with the passkey flow. Use `sessionStatus()` on startup, add first-owner setup and display returned recovery codes once. The current submit handler can trigger a passkey prompt.
- Build coded forms from `operation.inputs` and send `item` only. The singular-kind alias in the existing rename and composer handlers is now rejected. Use the slot's installed filler metadata to select the target.
- Use `row.when[operation.name]` in the menu and propagate live row metadata in the open window. Use `FieldDecl.computed` for computed field presentation.
- Composer text binding, draft binding and how a quoted anchor becomes a link still need an explicit slot contract. The real host exports the declared input names and types. The Playwright test exercises the browser data client while these screen changes are pending.

### Questions for the coordinator

- **Operator bootstrap.** Which local operator channel should issue a first-owner code on a fresh standalone deployment? The browser needs a code before it can enroll its first passkey. A local terminal command is smaller, while the existing private admin socket keeps issuance inside the running process and supports the operator's established workflow. Reuse the private admin flow when the standalone host takes on deployment. The Playwright fixture passes its disposable credential over an anonymous pipe.
- **Shared-space admission.** How should an open live window clear content when a folder or kind grant changes? For example, someone who loses read rights while a chat is open could retain the last window in memory. Rechecking on each refresh limits delivery but leaves that old window visible. An explicit revocation event and client reset add a protocol action and close the visible window immediately. Use explicit revocation before enabling non-owner live access. The personal-space route currently denies that access.

## Comparison against the mockup (`http://10.69.69.237:4313/`)

- **Canvas & chrome:** Bare wallpaper backdrop (`Backdrop.svelte`), floating glass left sidebar (inset 8 px, `--r-card` radius, 32 px desktop / 44 px touch rows), rounded window cards (`--r-pane: 28px`, `--shadow-card`), no top bands, floating top-left title pill and top-right actions pill, and floating bottom-centre glass strip (`--tier-cap: 48px`).
- **Records view:** Own messages right-aligned in `--accent-soft`, others left-aligned in `rgb(var(--ink-rgb) / 0.07)`, 5-minute author grouping, day separator pills, straight 3 px accent bar on quotes, hover-only record action buttons, rounded `--r-card` composer at the bottom, and jump-to-latest button.
- **Motion & accessibility:** All transitions use `@calternal/ui` tokens (`--dur-fast`, `--dur-base`, `--ease-out`, `--press-scale`, `--enter-scale`), interruptible, with `prefers-reduced-motion: reduce` disabling/shortening motion; full keyboard focus rings and ARIA roles/labels.

## Verification on the development VM

Ran in `~/Developer/lanes/browser-chat` on `claude@calternal-dev-01-compute02`
under the shared build lock.

```sh
~/bin/lock-shared nix develop -c bash -c '
  set -e
  cargo fmt --all --check
  npm run build --workspace @calternal/plugin-sdk
  cargo test --locked -p calternal-host -p calternal-format -p calternal-auth
  cargo build --locked -p calternal-host
  node apps/web/e2e/host-chat.mjs
  (cd apps/web && ../../node_modules/.bin/svelte-check --tsconfig ./tsconfig.json)
'
```

The focused Rust run passed 160 tests, including all 30 host tests and
all five Bubblewrap tests, with zero ignored. The web type check reported
zero errors and five warnings in existing Svelte files. The workspace
documentation gate passed under
`~/bin/lock-shared nix develop -c ./scripts/check-docs.sh`
and checked 463 generated documentation pages.

The Chromium test starts `calternal-host` with disposable data and a
same-origin loopback proxy. It enrolls and signs in with a virtual passkey,
checks cookie attributes and the HTTP and WebSocket origin guards, and
installs the real Chat plugin. It creates a chat, sends and renames,
then verifies a second context sees the live append and receives exactly
the two missed records after reconnecting from its last anchor. It also
checks that signing out the second context revokes its open socket.

### Review gate notes

- The host reuses the auth service, shared actor, UUID and error types,
  compiled format definitions and existing per-space SQLite ledger.
- The space lock orders durable writes with nonblocking invalidation
  publication. SQLite and filesystem work run on blocking threads. Socket
  serialization and writes run after the space lock has been released.
- Auth and viewer permissions are checked at dispatch. The browser catalog
  is cached for the authorized personal-space shell and invalidated after
  installation or recovery. Query, read and live refresh reuse the same
  compiled expressions and viewer state.
- Channels have capacity 256 per connected viewer and sockets accept at most
  64 subscriptions. Disconnected viewer channels are pruned on subscription
  or publication. Existing receipt and trash retention applies. A one-time
  atomic ledger conversion assigns earlier personal state to its verified
  owner and leaves one schema for reads and writes.
- Conditional coded dispatch and other operation forms outside the current
  host slice remain constrained by their existing execution rules.

### Read and flush counts

These are host syscalls on the development VM, scoped from a browser request to
its response or live delivery. The trace used
`strace -D -f -qq -y -ttt -s 0 -e trace=read,pread64,preadv,readv,fsync,fdatasync`.
The query returned one Chat row with one record. The append was the first live
append after rename, with one socket subscribed to both collection and records.

| Path | Item reads | Ledger reads | Auth reads | Item flushes | Ledger flushes | Auth flushes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Warm collection query | 0 | 1 | 0 | 0 | 0 | 0 |
| Append and live delivery | 7 | 0 | 0 | 1 | 1 | 0 |

The query uses the cached heads and one indexed state and record-metadata
lookup per candidate. The append includes the record snapshot read for the
subscriber and the publication reads after rename. The SQLite page cache
can satisfy a logical lookup without a read syscall.

## Simplification opportunities

- Give the format engine typed presentation nodes for sidebar and collection view directives so the host can project them without splitting their already-validated display strings.
- Share the caller-safe HTTP and live error response adapter when the call route becomes the workspace default.
- Deliver record-tail reads from the ledger after an anchor. The current window rereads the authorized snapshot on a live append, which can dominate I/O for longer chats.
