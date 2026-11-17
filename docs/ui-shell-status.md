# Web shell: status (lane `lane/ui-shell`, issue #15)

State on 2026-11-17, after the second pass (owner review). The shell from `docs/ui-shell.md`, the design system (`docs/ui-design-system.md`) and the Threads app
run in a browser on fixture data. Nothing here talks to a server. Performance: `docs/ui-performance.md`.

Run (on a dev VM, inside `nix develop`): `npm install --ignore-scripts`, then in `apps/web`: `npm run dev`
(or `npm run build && npm run preview`), `npm test` (and `npm test --workspace @calternal/ui`), and the browser scripts
`node e2e/shots.mjs <dir>`, `e2e/a11y.mjs`, `e2e/audit.mjs`, `e2e/perf.mjs` (Playwright; `npm i` is enough, browsers
download into `~/.cache/ms-playwright`, no system packages needed on `calternal-dev-02-std`).

## What exists

| Part | Where |
| --- | --- |
| Tokens (one JSON source), fonts, adaptive glass, 13 components (surfaces, controls, icons) | `packages/ui` |
| Shell: the wallpaper behind both flat sidebars (left: header, attention card, body, footer with account, app switcher, search; right: the app's panels with tabs at the bottom), the middle card (the only card: the wallpaper's blurred copy, tinted and dimmed), narrow-screen layers and the phone's bottom tab morph | `apps/web/src/shell` |
| Appearance: wallpaper pipeline (blurred once, brightness and colours read once, wash derived for contrast), the wallpaper library adapter (built-ins, upload, Unsplash) and the font library adapter, settings sheet (wallpaper, glass, colour, fonts), `/settings/appearance` | `apps/web/src/appearance` |
| Routes as plain paths (`/threads`, `/threads/<id>`, `/notes/<id>`; the open right-card panel in `?panel=`) | `apps/web/src/lib/route.svelte.ts`, `App.svelte` |
| App contract (what a plugin fills: sidebar body, middle card, panels, attention) | `apps/web/src/apps/registry.ts` |
| Threads: sidebar (grouped by workspace, children nested, running/waiting/failed/finished), thread view, virtualised list, folded steps, long-log viewer, streaming row, composer with three separate chips on one row (provider and model, permission mode, where it runs; labels shorten with the composer's width; where it runs is a machine then Sandboxed or Direct, a bottom sheet on a phone), sticky request bar (question, approval, secret), Changes/Run/Files tabs | `apps/web/src/apps/threads` |
| Typed fixture module and the one adapter interface (`ThreadsClient`) | `threads/fixtures.ts`, `generate.ts`, `client.ts`, `model.ts` |
| Generated-UI renderer over `packages/genui` (Card, Stack, Row, Text, Callout, Badge, Divider, Code, Details, Progress, KeyValue, Table, Form, Input, TextArea, Select, Choice, Radio, Toggle, Checklist, Button; a plain fallback line for the other catalog components, an error line for an invalid description) | `apps/web/src/genui` |
| Notes placeholder (narrow centred column, no inner card, left rail that opens a table-of-contents card) | `apps/web/src/apps/notes` |

## Screenshots

Evidence images live on issue #15 as attachments (before and after for each point of the second pass are in its comments), not in the repository. Taken by
`e2e/shots.mjs` (Chromium, device scale 2, JPEG 92): desktop 1440 by 900, phone 390 by 844 with touch. Re-run the script to refresh them.

| Screen | Image |
| --- | --- |
| Desktop light | [image](https://git.kayg.org/attachments/7d855849-ec93-479b-977e-52fe64e134e7) |
| Desktop dark | [image](https://git.kayg.org/attachments/22e5d3c5-2f1b-4d01-a11a-9b5893bad95e) |
| Wallpaper: Ember, dark | [image](https://git.kayg.org/attachments/fe1e00dc-7a16-4a6a-a27d-7997dd3998f7) |
| Wallpaper: Slate, light | [image](https://git.kayg.org/attachments/62a28558-317c-4ac4-afbe-32080b36b6cc) |
| Settings: wallpaper and glass | [image](https://git.kayg.org/attachments/5bb92227-50d3-4754-b29e-9e9a3acb8e99) |
| Settings: fonts per role | [image](https://git.kayg.org/attachments/29e4eb0f-3d68-4fda-8690-95b898a926fb) |
| Composer: permission menu | [image](https://git.kayg.org/attachments/0c8e98d0-3123-4898-91af-9ca2c0a5a9a5) |
| Request bar: question | [image](https://git.kayg.org/attachments/1a41aa53-e775-43a5-830a-bd39165103aa) |
| Request bar: approval | [image](https://git.kayg.org/attachments/21b32368-c37e-4648-bc58-efdd29483c3a) |
| Request bar: secret, dark | [image](https://git.kayg.org/attachments/16b327e0-3596-4fe2-9ca7-373364068ae9) |
| The 10,000 message thread | [image](https://git.kayg.org/attachments/8abe1ecd-e8e9-44fe-9240-6036e31e8146) |
| Notes | [image](https://git.kayg.org/attachments/3b2ec682-536a-4c3d-8c57-d5fed0eacb69) |
| App switcher | [image](https://git.kayg.org/attachments/f3188148-f8d3-425a-a422-4fcbaa73f777) |
| App name (Rounded, ROND 100), crop | [image](https://git.kayg.org/attachments/b2e3d7ed-dd46-4b4f-90fd-35a941f46525) |
| Font proof: existing app vs this app, light | [image](https://git.kayg.org/attachments/e1a94e2f-eba1-4eba-9f81-914d73b7ed60) |
| (this app) | [image](https://git.kayg.org/attachments/2582e9a5-c05a-4e03-bd12-4e6bde25b4bb) |
| Font proof, dark: existing app | [image](https://git.kayg.org/attachments/d7c46f45-adfc-4c3a-b7f4-2e7923e3fd1d) |
| (this app) | [image](https://git.kayg.org/attachments/53a855b3-ecb7-4193-bc99-0d8dbf3b3abb) |
| Rounded family with ROND | [image](https://git.kayg.org/attachments/688e4a4e-29ed-4886-8717-4a4ca9e6e388) |
| Phone light | [image](https://git.kayg.org/attachments/1c1be62d-9fdc-4444-9ba8-92905850a8de) |
| Phone dark | [image](https://git.kayg.org/attachments/a9cf1f30-7800-4626-80a9-3f543fda8485) |
| Phone: where it runs, a bottom sheet | [image](https://git.kayg.org/attachments/ec6b5613-cae4-412b-a27e-a1edfce97c1c) |
| Phone: settings | [image](https://git.kayg.org/attachments/708f8d57-851f-40b1-8b09-3de01f0f414e) |
| Phone: settings, dark | [image](https://git.kayg.org/attachments/82f227c9-35c4-4422-ab25-bdce984c4188) |
| Phone: request bar | [image](https://git.kayg.org/attachments/3fb16110-0be0-4a4d-822d-949a44a140c8) |
| Phone: sidebar underneath | [image](https://git.kayg.org/attachments/9a64886e-8b79-4fe0-8c2c-f91cb73e66db) |
| Phone: right sidebar slid over | [image](https://git.kayg.org/attachments/aaa5b1e5-f484-43a8-ae69-cef0c3c2056a) |

## Preview

Deployed at https://preview.calternal.cloud/ai/ (NetBird only) by `apps/web/scripts/deploy-preview.sh`, run on `calternal-dev-02-std`: it builds with
`CAL_BASE=/ai/`, replaces the contents of `~cua/preview/ai` (tar, since rsync is not installed there) and checks the page, a deep link and a font file.
`CAL_URL=http://127.0.0.1/ai node e2e/preview-check.mjs` on that host checks the deployed copy in a browser.

## Spacing, alignment and sizing checklist

Per screen. "Script" items are checked by a script on every run (`audit.mjs`, `a11y.mjs`); "Eye" items were checked by
looking at the screenshot only, which is not a pixel measurement (`docs/native-ui-study.md` gives the target sizes; the audit checks the scale, not each pixel against it).

| Check | Method | Thread (desktop) | Request bar | Long thread | Notes | Phone (thread, bar, sidebar, right card) |
| --- | --- | --- | --- | --- | --- | --- |
| Only the three families are drawn (Google Sans Flex, its Dark and Rounded, Maple Mono) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Only the token type scale (desktop 11, 12, 13, 15, 22; phone 11, 13, 15, 17, 20, 34) and the three weights (440, 460, 620) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Every padding, margin and gap is on the `s-1`..`s-10` scale | Script | ✔ | ✔ | ✔ | ✔ | ✔ except two token-derived values (bottom inset 72, notes bottom 120, both built from tokens) |
| Hit targets at least 44 px on touch | Script | n/a (desktop 28) | n/a | n/a | n/a | ✔ |
| axe (WCAG 2 A/AA/21AA and best practice, contrast included), light and dark | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Radii come only from the four tokens (pill, card 14, pane 20, select 8); the selected-row 8 is the native value and is deliberately not concentric | By construction | ✔ | ✔ | ✔ | ✔ | ✔ |
| Icons sit on the centre of their text block (one line, or title plus quiet line) within 1 px, in rows, buttons and pills | Script (`e2e/align.mjs`) | ✔ | ✔ | ✔ | ✔ | ✔ |
| A nested rounded box is within 2 px of its parent's radius minus the gap (pills exempt) | Script (`e2e/align.mjs`) | ✔ | ✔ | ✔ | ✔ | ✔ |
| Ink stays at 4.5:1 on every built-in wallpaper, both schemes | Script (`e2e/wallpapers.mjs`) | ✔ | ✔ | ✔ | ✔ | ✔ |
| No fallback font flash | Head preload + paint gate; eye | ✔ | ✔ | ✔ | ✔ | ✔ |

Not verified: Safari and WebKit rendering (Linux Playwright WebKit
paints no backdrop blur), real iPhone hit targets and safe areas, reduced-motion and reduced-transparency by screenshot
(they are CSS media blocks, tested by reading, not by a run).

## Deliberate deviations

From `docs/ui-shell.md` there are none. From the mockup:

| Deviation | Why |
| --- | --- |
| No frame, no inner page card, no horizontal app strip; the sidebar sits directly on the background | `docs/ui-shell.md` |
| Middle and right cards run top to bottom of the viewport (rounded all round, gap 8 px between them and at the sides) | `docs/ui-shell.md`; the side gap is a choice (see Open) |
| Glass blur 20 px (was 28 in the mockup's pane glass); the mockup's floating glass and the native study's closest CSS (section 8) are also 20 | Software compositors stop blurring above about 24 px, so larger values could not be verified here |
| No sky, photo or plain backdrop picker; one tinted mesh background | `docs/ui-shell.md` (one seamless background); the mesh is the adaptive material's data |
| Two schemes plus "System"; no Warm and Cool paper | Not asked for; tokens make it a small later change |
| Phone bottom bar is a plain flex capsule: the selected item widens and shows its name, the rest shrink to icons, on the sheet spring (the mockup's `Morph` is a measured, multi-mode bar) | Keeps the morph with far less code; the full Morph can replace `Capsule morph` later |
| No progressive blur strips, home-screen standalone handling, or Safari toolbar insets | Not needed for a first shell; to revisit with real-device work |
| Tokens are JSON, generated into CSS; the one spring is generated at build time, not installed by script | Native clients read the same tokens; no start-up script |
| Motion and spacing tokens collapsed: one `--inset` (mockup: five gap names), three durations and two curves (mockup: twelve), the control tier is one token, `--pill-h` (38 desktop, 44 touch, which is also the touch hit target), radii collapsed to pill, card 14, pane 20 and select 8 | `docs/native-ui-study.md` Simplification opportunities |
| Native values everywhere the study differs (owner decision 2026-11-17): sidebar and list rows 32, selected row radius 8, desktop text 13, desktop toolbar band 52 with 38 buttons, phone bottom capsule 49 sitting 27 above the bottom, phone title 34 (desktop 22), settings rows 44 desktop and 52 phone | `docs/native-ui-study.md` section 7.2 |
| The right side is a flat sidebar on the wallpaper, not a card; the middle card is made of the wallpaper's blurred copy plus a tint and dim, not a backdrop filter | Owner decision, 2026-11-17 (second pass) |
| The background is a wallpaper (eight painted built-ins, uploads, Unsplash) blurred once, not the colour mesh (the mesh remains as the no-wallpaper fallback) | `docs/ui-shell.md` "Backgrounds and fonts" |
| Pill glass is the plugin's: 16 px blur, saturate 180 %, brightness 1.12 (dark 0.72), clear fill, wash up to 0.35 where needed | The owner's daily look |
| Text is 13 px on a desktop everywhere, messages included; the heavier 620 weight only for emphasis | Owner, second pass |
| Rounded Google Sans Flex only for the app name, with `"ROND" 100`, as the existing app does | Owner, second pass |
| Kept from the mockup on purpose: the narrow centred Notes column (a Calternal choice, not native) | Owner decision |
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
| Permission mode and where it runs | Two chips in the composer and the Run tab: `Least access` / `Approve for me` / `Full access`, and `<machine> · Sandboxed` or `Direct` (fixture machines, in memory) | `threads.set_permission`; a list of the machines the person may use; `threads.set_runs_on` (applies to the next message, never moves running work); the real one-line explanations for the three modes (mine are placeholders) |
| Wallpapers, uploads, Unsplash search | Painted built-ins; uploads in IndexedDB; search returns generated pictures with fake credits | Storage per account; Unsplash search with the operator's access key and a "save" that downloads and stores the photo (pages never load from Unsplash); credits kept with the photo |
| Font catalogues and storage | Short hand-written lists; only the shipped families and the person's own uploads can be drawn | Google Fonts, Fontsource and Fontshare search; download once, store and serve the files; own-font storage; the per-account font choices |
| Appearance settings | `localStorage` | Stored per account on the server, with the files they refer to |
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
- **The dark scheme over a bright wallpaper.** Ink must stay readable, so the sidebars get up to a 90 % black wash and the picture nearly disappears
  (Dune, Lagoon, Ember, Orchid, Moss, Paper in dark). Option: brighter secondary ink in dark, so a lighter wash passes; cost: a second ink set to maintain.
- **Panes that abut.** The study found list and editor panes flush with only the sidebar floating; here the middle and right
  cards are both floating cards separated by one `inset`. Say if the right card should be flush with the middle card.
- A second study pass (motion, dark, tap behaviour) is pending; it will change tokens only.

## Unfinished

- The plugin's edge fades (scroll-driven per-row fades under floating bars), the title pill, the wordmark row, paste-a-wallpaper-link, the storage list
  with sizes, and pill-contrast "hazards" (media scrolling under pills) are not built.
- Phone gestures (edge swipe to open the sidebar, drag to dismiss a sheet); the sidebar opens from the header button.
- Offline pending submissions (visible, editable, cancellable) and drafts that sync; drafts are kept in memory per thread.
- Editing a sent message and forking a thread from a message.
- The adaptive material is unit tested and wired to every glass surface, but with this background (calm on
  purpose) it barely moves; it needs a user wallpaper or a darker mesh to be seen.
- Skills, automations, plugin management screens.

## Simplification opportunities

Done in this lane (after `docs/native-ui-study.md`): one `--inset` instead of five gap names; three durations and two curves instead of
twelve tokens; one control tier (`--pill-h`) instead of `hit-min`, `tier-top`, `bar-h` and `pill-h` variants; four radii (pill, card, pane,
select) instead of seven; nine unused tokens removed. Still to simplify:

- Two glass alphas (`glass-a`, `card-a`) and two shadows (`shadow-card`, `shadow-float`): the study found one material and one soft
  shadow; the role argument of `material()` can go if a card and a pill share them.
- Four ink greys (`ink`..`ink-4`): `ink-4` is used only for disabled and placeholder text; native secondary labels are one lowered alpha.
- Ten spacing steps (`s-1`..`s-10`): the audit shows which are drawn; unused steps can go.
- `--row-h` and `--row-h-settings` and `--bar-h` are three heights for what native does with two (row 32, settings 44) plus the 52 band.

In this lane's code:


- `Capsule`, `Toolbar` and `Pill` share one glass and one roving-focus helper; a `Group` base would remove the
  duplicated padding rules.
- The fixture generator, the Fenwick tree and the Markdown subset are three small pieces that the Notes app
  will also want (`apps/web/src/lib`); they should move into `packages/ui` when a second app uses them.
- `apps/web/src/genui/GenNode.svelte` is one file of 67 possible components; it will split by group when more are drawn.

## Update 2026-11-17 (owner list 4)

Done: one-line thread rows (title, provider mark, one compact state, time on hover); state never icon plus word; no automatic section labels;
toasts ported from the existing app and mounted (Archive offers Undo, fixture `unarchive`); warm tooltips (existing engine kept, comparison in
`docs/ui-design-system.md`); pill bounce (pointer only); double-click the title pill to rename; `InlineEditor` behind composer, side chat and rename fields;
phone composer hit target fixed to 44 px. `e2e/interact.mjs` checks these (10 of 10 pass).

Fixture-only: `unarchive` and `rename` are fixture client methods; the backend must provide both (undo of archive needs a stable, reversible archive).

Open: whether deleting a section should offer Undo (not done); the composer palette (`docs/ui-composer-palette.md`) is planned, not built.

## The owner's four lists, item by item (2026-11-17)

Screenshots are attached to issue #15 (review set: `desktop-light`, `desktop-dark`, `phone-light`, `phone-dark`, `sidebar-sections`, `context-menu`, `model-picker`, `toast-archive-undo`, `request-bar`, `shortcuts-sheet`, `selection-actions`, `phone-machine-sheet`, `phone-sidebar`). Tests are `npm test`, `e2e/interact.mjs` (10 checks), `e2e/a11y.mjs`, `e2e/audit.mjs`, `e2e/align.mjs` (now self-tested), `e2e/motion.mjs`, `e2e/perf.mjs`, `e2e/paint-cost.mjs`, `e2e/preview-check.mjs`.

| Item | Done | Evidence |
| --- | --- | --- |
| Flat right sidebar (no card) | yes | `desktop-light` |
| Fades at the top and bottom of both sidebars and under the middle card title | yes | `.edge-fade` in `Sidebar`, `RightSidebar`, `VirtualList`; `desktop-light` |
| Wallpaper blurred once, paint cost measured | yes | `docs/ui-performance.md` (0 late frames against 47 to 60 % with a live blur), `e2e/perf.mjs`, `e2e/paint-cost.mjs` |
| Accent inferred from the wallpaper, guarded, overridable | yes | `appearance/accent.ts`, `theme.test.ts`, `wallpapers.mjs` contrast |
| Cursive wordmark, no icon, name and font configurable | yes | `desktop-light` (top left), `SettingsSheet` |
| "Needs you" plain (same font, not accent-tinted) | yes | `desktop-light` |
| Nothing draws out of bounds (inset rings, placed menus) | yes | `lib/place.ts`; a regression where sheets lost `position: fixed` (Panel's own rule out-ranked `.sheet`) was found in the review shots and fixed |
| Focus rings for the keyboard only; no animation from the keyboard | yes | `interact.mjs`, `motion.mjs` (key mode: 0 s) |
| No streaming by default (setting) | yes | `prefs.streamText = false` |
| Shortcut sheet on Cmd-/ and ? | yes | `shortcuts-sheet`, `shortcuts.test.ts` |
| "Comment" and "Ask in Side Chat" on text selection | yes | `selection-actions` |
| Model picker with provider icons and inline search | yes | `model-picker`, `Composer.test.ts` |
| Machine picker: two steps, a sheet on a phone | yes | `phone-machine-sheet`, `RunsOnPicker.test.ts` |
| Thread rows one line, one state signal, provider mark | yes | `sidebar-sections`, `interact.mjs` |
| Toasts ported; Undo for Archive and for deleting a section | yes | `toast-archive-undo`, `toast-*.test.ts`, `layout.test.ts` |
| Warm tooltip, pill bounce, title rename, InlineEditor | yes | `interact.mjs`, tooltip comparison in `docs/ui-design-system.md` |
| Composer as a command palette | no | planned in `docs/ui-composer-palette.md` (next) |
| Settings is an app (User and Admin sections, empty pages) | yes | `settings-app`, `apps/settings/` |
| Materials: three checkboxes (Blur, Dim, Tint), strength computed from the wallpaper | yes | `settings-appearance-sheet`, `SettingsSheet.test.ts`, `washFor` for the sidebars and for the card |
| One UI size slider scales type, spacing and controls | yes | `--ui-scale` in `build-tokens.mjs`, `SettingsSheet.test.ts` |
| Default wallpapers: the eight Unsplash photos by ID, fetched at build time, credited in a tooltip | yes | `apps/web/scripts/wallpapers.mjs`, `wallpapers.json`; images are gitignored |
| First run: a preset gallery | yes | `FirstRun.svelte` (CAL_FIRSTRUN=1 in the e2e scripts shows it) |
| App switcher experiment (sidebar footer or capsule under the middle card; `?switcher=card`) | yes | `switcher-card`, `switcher-card-dark`; Appearance has the same choice |
| Icon controls sit in pills (sidebar toggle, search, close) | yes | `desktop-light` |
| Font squish: cause (optical size follows font size, 5 % wider than bb) and fix (`opsz` pinned at 18) | yes | issue #15 comments, `e2e/font-squish.mjs` |
| Message actions (copy, reply in side chat, more) beside the time, fade in on hover/focus, tooltips | yes | `message-actions-hover` |
| Hover row actions, one calm pulse for running things, active-children bar | yes | `children-bar`, `base.css` `.pulse` |
| "Threads" section for unsectioned threads, renamable | yes | `desktop-light`, `sidebar-sections` |
| App capsule: labels only, three apps, pill glass | yes | `switcher-card` |

Notes (2026-11-17): the default photographs are fetched when the app is built (`npm run build` runs `scripts/wallpapers.mjs`), so a build needs network access to unsplash.com once; offline, the generated looks stand in. The earlier committed stand-in photos were removed from the tree (they remain in older commits). Open: the auto-hide setting for the card switcher is not built (always visible); the Settings sections list awaits the study (issue #22).
