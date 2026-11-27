# Browser for Chat on the new host (slices 4 and 5) · 2026-11-27

Lane E ([Forgejo #118](https://git.kayg.org/kayg/calternal-ai/issues/118)).
References:
[Build plan](../proposals/chat-first-build-plan-2026-11-27.md) ·
[Primitives-only host](../proposals/primitives-only-host-2026-11-21.md) ·
[Kind and collection format r9](../proposals/kind-and-collection-format-2026-11-24.md) ·
[Host call route](host-call-route-2026-11-27.md) ·
[Live window wire protocol](live-window-protocol-2026-11-27.md).

## What was built

- **Slice 4 (live window):**
  - Wire protocol: [live-window-protocol-2026-11-27.md](live-window-protocol-2026-11-27.md).
  - Host module: `crates/host/src/live.rs` wired to `GET /v1/spaces/{space}/live` on `calternal-host`, with per-space bounded broadcast channels (`Hub`), `auth` handshake, collection/query subscriptions, and record subscriptions with `after` anchor delta catch-up and lag recovery.
  - Browser client: `LiveClient` in `apps/web/src/host/client.ts` (one WebSocket per browser session, auto-reconnect resuming each open records view from its `lastAnchor`).
- **Slice 5 (declaration-driven browser entry):**
  - New `/host` entry in `apps/web/src/main.ts` mounting `apps/web/src/host/HostApp.svelte` (all existing app entries left intact).
  - Zero plugin- or Chat-specific strings in `apps/web/src/host/`: sidebar, collection list, item records view, composer slot, and item/record operation menus and forms are generated strictly from the declarations returned by the host.
  - Reused `@calternal/ui` tokens and components (`Backdrop`, `Card`, `Button`, `Callout`, `Icon`, `Menu`, `Sheet`) and adapted `Records.svelte` and `FormSheet.svelte` from `experiments/drawing-spike/web` (`origin/lane/spike-drawing`) to the approved `apps/mockup` (`origin/lane/threads-ui`) bare-canvas look.

## Format and route gaps recorded

| Section | What the screen needed | What we did meanwhile |
| --- | --- | --- |
| `host-call-route-2026-11-27.md`, Format §1.1 | A host call or route returning the space's installed plugin declarations (manifests, kinds, collections, sidebars, slots/fills) so the browser can draw from them. `crates/host` currently serves only `POST /v1/spaces/{space}/call` for data calls. | Served `GET /v1/spaces/{space}/declarations` from the local dev/test server (`apps/web/scripts/host-mock.mjs`) by parsing `plugins-v2/calternal-chat/*.md` directly from disk. |
| `crates/host/src/main.rs` | Browser sign-in / sign-out HTTP endpoints returning a bearer session and personal space UUID. `calternal-host` currently expects a pre-issued bearer token. | Added `POST /v1/auth/session` and `DELETE /v1/auth/session` on the local dev/test server. |
| Format §2.20 (`operations:`) | Input parameter names and types for coded operations (`rename: code, menu`, `send: code`). `kinds/chats.md` lists no inputs, while `operations.ts` expects `{ chat, title }` and `{ chat, text, quote? }`. | Derived single-field forms from the kind's primary text field (`title`) and passed both `item` and the singular kind ID (`[kind.id.replace(/s$/, "")]`) so no plugin vocabulary enters the client. |
| Format §7.4 (`slots:` / `fills:`) | Mapping from a slot fill (`calternal-chat:composer: operation send`) to draft persistence (`state.draft`), text input key, and optional quote input key. | Rendered the generic composer whenever a plugin fills a slot with an `operation`, bound the text input to `text`, enabled quoting when `quote` is declared in `kind.records`, and persisted drafts per item in `localStorage`. |
| `host-call-route-2026-11-27.md`, Format §2.9 (`state:`) | Per-viewer `state` keys (`state.pinned`, `state.muted`) on `s.query` / `s.read` rows so the client can evaluate `when` predicates (`pin: ..., when not state.pinned, menu`). | Returned `state` on query rows in the mock and checked `row.state?.[k] ?? row.computed?.["state." + k]` in `apps/web/src/host/format.ts`. |
| `docs/native-ui-study.md` & `docs/ui-craft-and-settings-study.md` | Exact numbers for record bubble max-width, composer inset margins inside a window card, and bottom-centre strip padding (not measured in either study). | Followed `apps/mockup` (`origin/lane/threads-ui`) values (`max-width: min(82%, 38rem)`, `--s-5` desktop / `--s-4` phone margins, `--tier-cap: 48px`) instead of inventing new numbers. |

## Security questions for the coordinator

1. **WebSocket authentication for browsers (`GET /v1/spaces/{space}/live`):**
   - *What it is:* Browsers cannot attach an `Authorization: Bearer` header to a `WebSocket` handshake.
   - *Why now:* Slice 4 ships `crates/host/src/live.rs` for real browser sessions.
   - *Options:* (A) First-frame `{"type":"auth","token":"..."}` over WSS (never in the URL, works across origins/ports during dev, implemented in `live.rs`); (B) `HttpOnly; SameSite=Strict` session cookie checked on the HTTP upgrade.
   - *Recommendation:* Keep (A) as the protocol-level fallback and add (B) once `calternal-host` owns browser cookie sign-in, so tokens never touch `sessionStorage` in production.
2. **Per-viewer `s.state` notifications in shared spaces:**
   - *What it is:* `s.state` writes (`pinned`, `muted`, `read through`) are private to one person. Today `live.rs` broadcasts a revision tick to the space's channel, and each socket re-evaluates through its own `Actor` (so private state never leaks).
   - *Why it matters for slice 9:* In a multi-person space, one member marking a chat read would wake other members' sockets unnecessarily.
   - *Recommendation:* Tag `SpaceEvent` with `Option<AccountId>` (when `Some`, only that account's sessions refresh) before shared spaces land.

## Comparison against the mockup (`http://10.69.69.237:4313/`)

- **Canvas & chrome:** Bare wallpaper backdrop (`Backdrop.svelte`), floating glass left sidebar (inset 8 px, `--r-card` radius, 32 px desktop / 44 px touch rows), rounded window cards (`--r-pane: 28px`, `--shadow-card`), no top bands, floating top-left title pill and top-right actions pill, and floating bottom-centre glass strip (`--tier-cap: 48px`).
- **Records view:** Own messages right-aligned in `--accent-soft`, others left-aligned in `rgb(var(--ink-rgb) / 0.07)`, 5-minute author grouping, day separator pills, straight 3 px accent bar on quotes, hover-only record action buttons, rounded `--r-card` composer at the bottom, and jump-to-latest button.
- **Motion & accessibility:** All transitions use `@calternal/ui` tokens (`--dur-fast`, `--dur-base`, `--ease-out`, `--press-scale`, `--enter-scale`), interruptible, with `prefers-reduced-motion: reduce` disabling/shortening motion; full keyboard focus rings and ARIA roles/labels.

## Simplification opportunities

- Add a single `s.declarations` call to `crates/host` returning the already-parsed `Space.plugins` tree as JSON, eliminating the need for any client-side or gateway Markdown parser.
- Standardize the target item key on all item-scoped coded operations as `item` (or declare typed operation inputs in `kinds/*.md`) so clients never guess between `item` and `chat`.
- Include per-viewer `state` (or pre-evaluated `when` flags) in `s.query` rows so clients do not need a separate `s.state` call per row to render operation menus.
- Once `calternal-host` mounts browser auth and `s.declarations`, `apps/web/scripts/host-mock.mjs` can be retired completely and Playwright can run directly against `calternal-host`.
