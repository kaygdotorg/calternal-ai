# Web UI preview (apps/web, `@calternal/web`)

Svelte 5 + Vite + TypeScript. Development preview of the single-sidebar shell on disposable data. Not production, not an AI product.

## Run
- Build: `npm run build --workspace @calternal/web` → `apps/web/dist` (served same-origin by the server; needs SPA fallback to `index.html` for non-`/api`, non-asset paths).
- Dev: `API_TARGET=http://127.0.0.1:3210 npm run dev --workspace @calternal/web` (Vite proxies `/api`).
- Check/tests: `npm run check --workspace @calternal/web`, `npm test --workspace @calternal/web` (unit + jsdom renderer tests).
- `tests/mock-server.mjs` is a TEST-ONLY mock of the frozen contract used for early browser checks. It is never shipped and is not evidence for the real server.

## What it does
- Token entry: password field, token kept only in a module variable (not URL/localStorage/cookies/DOM after submit); reload forgets it; unauthorized responses return to the gate. Requested route survives the gate.
- Routes (plain paths, History API; ADR 0056): `/s/<space>/notes`, `/s/<space>/notes/<id>`, `…/<id>/history`, `/s/<space>/notes/new`, `/s/<space>/plugins[/<id>]`, `/activity`.
- Spaces come from `GET /api/v1/describe` principal grants; switcher lists only granted spaces; ungranted space deep link shows "Space not available".
- Notes: list, open, edit (native textarea, explicit limited proof; no CRDT/rich editor), save with `base_revision`, history with "load into draft". Failed/conflicting save keeps the draft; conflict shows the latest server copy and offers "keep my draft on top of latest" or "discard draft". Identical retries reuse the same `request_id`.
- Plugins: list, details, preview (fixture view, works while disabled, actions disabled), enable/disable (manager only in UI; server authoritative). Enabled `notes-insights` adds a read-only panel on note detail.
- GenUI renderer: stack/text/button/stat/resource/document-editor; text via text nodes only (no `innerHTML`); client-side limits/duplicate-ID/version validation (defence in depth; server is authoritative); unknown node → explicit unsupported fallback that never dispatches; extension buttons may invoke only operations the plugin declares read-only in its manifest, others are shown disabled. `document-editor` renders a read-only preview with "Open to edit" (the real editor is the Notes pane).
- AI pane: closable, honest "setup pending"; no simulated chat.
- Activity: this browser session's events only (saves, conflicts, plugin changes). Cross-space persistent Activity is not implemented.
- Accessibility: skip link, landmarks, focus moves to main on navigation, labelled controls, `aria-live` for results, visible focus rings, keyboard menu (arrows/Esc), Ctrl/⌘+S saves, `prefers-reduced-motion`, light/dark, drawer sidebar ≤820px, AI pane overlays ≤1100px.

## Assumptions to confirm against the server
- Operation names: `notes.list|get|create|update|history`, `plugins.list|enable|disable|preview`, `notes-insights.summarize`.
- Extension buttons' `action.input` gets host context (`space_id`, `id` of the open note) filled in only where the plugin did not supply them; the host revalidates.
- `describe` may be a bare `DescribeResult` or enveloped; both are accepted.
- Unsaved drafts are retained in tab memory per note (and per new-note space), so navigation, back/forward and space switches keep them; a retained draft is restored on its original base revision, so a moved-on server yields a conflict rather than an overwrite. Reload warns (`beforeunload`); Disconnect confirms and discards drafts.

## Reuse from the existing mockup (`/home/kayg/Developer/calternal`, `origin/dev`)
Adopted design decisions (values copied, not imported; that repo's `packages/ui` is a Bun workspace with its own fonts/primitives):
- `packages/ui/src/styles/tokens.css`: neutral paper `#f7f7f7`/dark `#1f1f1f`, ink scale, `--line`/`--hover` alpha tints, amber accent `#c26a12` with derived soft tint, card radius 20px, `--shadow-wcard`, `--ease-out`, `--type-px` text scaling.
- `apps/mockup/src/components/Sidebar.svelte`: flat sidebar, 32px rows with 9px radius, selected row = accent-soft tint, rounded brand weight 650, footer identity row.
- `apps/mockup/src/components/Shell.svelte`: drawer sidebar + scrim on narrow widths, no icon rail, Esc closes.

Deliberate deviations: one sidebar + one main card + optional AI pane (accepted architecture) instead of the mockup's sidebar + list pane + editor; system font stack instead of Google Sans Flex/Maple Mono (fonts not vendored); no glass/backdrop material, bottom bar, sheets, resizable panes or localStorage-persisted widths (storage avoided on purpose); hash routes replaced by plain paths.

## Not done / limits
Textarea editor only; no search, trash, deletion, offline, real Activity or AI.
## Verification log (2026-11-16, development host, built static UI served by the real server on :3210)
- Deep-link reload `/s/personal/notes/<id>` after token entry; create/edit/save; history shows both revisions; SPA fallback works.
- Back/forward and Activity round-trips kept an unsaved draft; an external (API) update then produced a conflict that kept the draft and showed the latest server copy.
- Guest principal: `/s/personal/…` shows "Space not available", only `shared · reader` is selectable, editor/title read-only, no Save/New; Disconnect leaves no note titles in the DOM.
- Stale-response safety: every async result is tagged with a session epoch and the API client discards responses to a replaced token; covered by `tests/store.test.ts`. Plugin list/preview/enable and summarize on the real server are pending the plugin registry/worker bundle (the server currently returns an empty plugin list); they were exercised against the test mock only.
- Token never appears in URL, DOM after submit, storage or screenshots (verification scripts injected it from the owner-only file without printing).

- Real plugin path (worker bundle pinned): preview (fixture stats, works while disabled) → enable → open note → "Summarize statistics" returns real worker stats (3 words / 14 characters / 1 line for the test note) → disable: panel disappears and the API denies `notes-insights.summarize` ("plugin is disabled in this space").
- Accessibility pass: mobile drawer moves focus in, makes the page inert, Esc closes and returns focus to the menu button; AI pane focuses its close button, Esc closes and returns focus; sr-only status announces Saving/Unsaved/Saved; h1 per view; token field autofocus; app menu closes on outside click. GenUI: host context (space_id/id) overrides plugin-supplied input, actions are re-checked at dispatch, malformed buttons/references and over-long labels are rejected client-side.
- Screenshots from the real server (no token visible): `docs/assets/ui-preview/plugin-preview.jpg`, `note-extension-stats.jpg`, `mobile-note.jpg`.
