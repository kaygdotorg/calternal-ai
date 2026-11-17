# Web shell: status (lane `lane/ui-shell`, issue #15)

State on 2026-11-17. The shell from `docs/ui-shell.md`, the design system (`docs/ui-design-system.md`) and the Threads app
run in a browser on fixture data. Nothing here talks to a server. Performance: `docs/ui-performance.md`.

Run (on a dev VM, inside `nix develop`): `npm install --ignore-scripts`, then in `apps/web`: `npm run dev`
(or `npm run build && npm run preview`), `npm test` (and `npm test --workspace @calternal/ui`), and the browser scripts
`node e2e/shots.mjs <dir>`, `e2e/a11y.mjs`, `e2e/audit.mjs`, `e2e/perf.mjs` (Playwright; `npm i` is enough, browsers
download into `~/.cache/ms-playwright`, no system packages needed on `calternal-dev-02-std`).

## What exists

| Part | Where |
| --- | --- |
| Tokens (one JSON source), fonts, adaptive glass, 12 surfaces and icons | `packages/ui` |
| Shell: tinted background, sidebar (header, attention card, body, footer with account, app switcher, search), middle card, right card with tabs at the bottom, narrow-screen layers and the phone's bottom tab morph | `apps/web/src/shell` |
| Routes as plain paths (`/threads`, `/threads/<id>`, `/notes/<id>`; the open right-card panel in `?panel=`) | `apps/web/src/lib/route.svelte.ts`, `App.svelte` |
| App contract (what a plugin fills: sidebar body, middle card, panels, attention) | `apps/web/src/apps/registry.ts` |
| Threads: sidebar (grouped by workspace, children nested, running/waiting/failed/finished), thread view, virtualised list, folded steps, long-log viewer, streaming row, composer with provider control, sticky request bar (question, approval, secret), Changes/Run/Files tabs | `apps/web/src/apps/threads` |
| Typed fixture module and the one adapter interface (`ThreadsClient`) | `threads/fixtures.ts`, `generate.ts`, `client.ts`, `model.ts` |
| Generated-UI renderer over `packages/genui` (Card, Stack, Row, Text, Callout, Badge, Divider, Code, Details, Progress, KeyValue, Table, Form, Input, TextArea, Select, Choice, Radio, Toggle, Checklist, Button; a plain fallback line for the other catalog components, an error line for an invalid description) | `apps/web/src/genui` |
| Notes placeholder (narrow centred column, no inner card, left rail that opens a table-of-contents card) | `apps/web/src/apps/notes` |

## Screenshots

Taken by `e2e/shots.mjs` (Chromium, device scale 2, JPEG 90). Desktop 1440 by 900; phone 390 by 844 with touch.

| Screen | Image |
| --- | --- |
| Desktop light: thread with a running child, Changes tab, attention card | ![](assets/ui-shell/desktop-light-stream.jpg) |
| Desktop dark | ![](assets/ui-shell/desktop-dark-stream.jpg) |
| Request bar: question form | ![](assets/ui-shell/desktop-light-request-question.jpg) |
| Request bar: approval (second of two) | ![](assets/ui-shell/desktop-light-request-approval.jpg) |
| Request bar: secret, dark | ![](assets/ui-shell/desktop-dark-request-secret.jpg) |
| The 10,000 message thread | ![](assets/ui-shell/desktop-light-long-thread.jpg) |
| Notes: the second middle-card layout with the contents card open | ![](assets/ui-shell/desktop-light-notes.jpg) |
| App switcher expanded | ![](assets/ui-shell/desktop-light-switcher.jpg) |
| Phone light, dark | ![](assets/ui-shell/phone-light-thread.jpg) ![](assets/ui-shell/phone-dark-thread.jpg) |
| Phone: request bar | ![](assets/ui-shell/phone-light-request.jpg) |
| Phone: sidebar underneath, the middle card floating above it | ![](assets/ui-shell/phone-light-sidebar.jpg) |
| Phone: right card slid over, bottom tab morph | ![](assets/ui-shell/phone-light-right-card.jpg) |

## Spacing, alignment and sizing checklist

Per screen. "Script" items are checked by a script on every run (`audit.mjs`, `a11y.mjs`); "Eye" items were checked by
looking at the screenshot only, which is not a pixel measurement (the native-ui-study numbers are the missing reference).

| Check | Method | Thread (desktop) | Request bar | Long thread | Notes | Phone (thread, bar, sidebar, right card) |
| --- | --- | --- | --- | --- | --- | --- |
| Only the three families are drawn (Google Sans Flex, its Dark and Rounded, Maple Mono) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Only the token type scale and the three weights (440, 460, 620) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Every padding, margin and gap is on the `s-1`..`s-10` scale | Script | ✔ | ✔ | ✔ | ✔ | ✔ except two token-derived values (bottom inset 72, notes bottom 120, both built from tokens) |
| Hit targets at least 44 px on touch | Script | n/a (desktop 28) | n/a | n/a | n/a | ✔ |
| axe (WCAG 2 A/AA/21AA and best practice, contrast included), light and dark | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Radii nest (inner = outer minus padding) | By construction, eye | ✔ | ✔ | ✔ | ✔ | ✔ |
| Icons sit on the text's optical centre in rows, buttons and pills | Eye | ✔ | ✔ | ✔ | ✔ | ✔ |
| No fallback font flash | Head preload + paint gate; eye | ✔ | ✔ | ✔ | ✔ | ✔ |

Not verified: off-by-one-pixel alignment (no measurement tool yet), Safari and WebKit rendering (Linux Playwright WebKit
paints no backdrop blur), real iPhone hit targets and safe areas, reduced-motion and reduced-transparency by screenshot
(they are CSS media blocks, tested by reading, not by a run).

## Deliberate deviations

From `docs/ui-shell.md` there are none. From the mockup:

| Deviation | Why |
| --- | --- |
| No frame, no inner page card, no horizontal app strip; the sidebar sits directly on the background | `docs/ui-shell.md` |
| Middle and right cards run top to bottom of the viewport (rounded all round, gap 8 px between them and at the sides) | `docs/ui-shell.md`; the side gap is a choice (see Open) |
| Glass blur 20 px (was 28 in the mockup's pane glass); the mockup's floating glass is also 20 | Software compositors stop blurring above about 24 px, so larger values could not be verified here |
| No sky, photo or plain backdrop picker; one tinted mesh background | `docs/ui-shell.md` (one seamless background); the mesh is the adaptive material's data |
| Two schemes plus "System"; no Warm and Cool paper | Not asked for; tokens make it a small later change |
| Phone bottom bar is a plain flex capsule: the selected item widens and shows its name, the rest shrink to icons, on the sheet spring (the mockup's `Morph` is a measured, multi-mode bar) | Keeps the morph with far less code; the full Morph can replace `Capsule morph` later |
| No progressive blur strips, home-screen standalone handling, or Safari toolbar insets | Not needed for a first shell; to revisit with real-device work |
| Tokens are JSON, generated into CSS; springs are generated at build time, not installed by script | Native clients read the same tokens; no start-up script |
| Fonts: `block` display plus preload plus a paint gate instead of `optional`/`swap` | Owner rule: no fallback face may show |
| Message and request text are never HTML: Markdown parses to a typed tree | Agent output is untrusted |

## Fixture-only, and what the backend must provide

| Part | Fixture today | Needed from the backend |
| --- | --- | --- |
| Thread list, children, states | `FixtureClient.listThreads`, nine threads | `threads.list` with `parentId`, `state`, `workspace`, `updatedAt`, `messageCount`; a change feed (`watchThreads`) |
| Messages | Generated on demand, pages of 100 | `threads.messages(thread, from, to)` by index range with stable block IDs; steps with long output kept outside the file and linked (the viewer then fetches the log) |
| Streaming | A timer appends text | A live stream per open thread: stream start, deltas, and the final message (the existing live connection) |
| Send, steer, stop, provider switch | In memory; the provider switch changes only the thread's provider and model | `threads.send` (request ID), `threads.stop`, `threads.set_provider`; a handover that adds no visible message |
| Requests (question, approval, secret) | Three fixture requests | A `requests` operation and feed; `answer(request, answer)`; the 15-minute secret expiry and "ask again" enforced server-side (the client countdown is only a display); a question's form as a generated-UI source string |
| Attention card | Derived from the requests | The same feed across the space's threads (already the source) |
| Changes, Run, Files tabs | Fixture rows | The turn's recorded changes, run/machine/usage info, and the workspace tree |
| Generated UI | Compiled and evaluated in the browser | Nothing; an unknown catalog component shows a fallback, so each missing component is a small client addition |
| Search, account, scheme | Search filters the fixture thread titles; account is "Kay" | Search operation; the signed-in account's name and picture |
| Notes | Static text | The Notes plugin's items and outline |

## Open

Design choices the documents do not settle. I chose the first option in each case and did not decide for you.

- **Gap beside the cards.** Cards touch the viewport top and bottom; sides keep 8 px. Option: flush to the
  viewport at the right and join the sidebar at the left (more "window", less "floating"). Cost: corners at the
  viewport edge stop being rounded.
- **Right card on a desktop under about 1100 px.** Today it stays 420 px and the middle card shrinks. Option: slide
  over like the phone below a chosen width.
- **Attention card order.** Newest request first, at most three, a count in the heading when there are more.
  Option: waiting approvals before questions before secrets.
- **Where the app name's Rounded face is used.** The app name and avatar initials only. Say which other places the existing app uses it.
- **Panel in the address.** `?panel=changes` opens the right card on that tab, and the address follows the open
  panel (a transient parameter, per `docs/addresses.md`). Option: not in the address at all.
- **Composer.** No mentions, slash commands or attachments (the plugin API says it needs its own design round).
- **`docs/native-ui-study.md`** (issue #19) did not exist on `origin/dev` or its branch when this was written. The
  shell follows the mockup until it appears; its measurements will change tokens only.

## Unfinished

- Phone gestures (edge swipe to open the sidebar, drag to dismiss a sheet); the sidebar opens from the header button.
- Offline pending submissions (visible, editable, cancellable) and drafts that sync; drafts are kept in memory per thread.
- Editing a sent message and forking a thread from a message.
- The adaptive material is unit tested and wired to every glass surface, but with this background (calm on
  purpose) it barely moves; it needs a user wallpaper or a darker mesh to be seen.
- Skills, automations, plugin management screens.

## Simplification opportunities

- `Capsule`, `Toolbar` and `Pill` share one glass and one roving-focus helper; a `Group` base would remove the
  duplicated padding rules.
- The fixture generator, the Fenwick tree and the Markdown subset are three small pieces that the Notes app
  will also want (`apps/web/src/lib`); they should move into `packages/ui` when a second app uses them.
- `apps/web/src/genui/GenNode.svelte` is one file of 67 possible components; it will split by group when more are drawn.
