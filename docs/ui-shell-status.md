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

- Not built: swipe back as a navigation stack on a phone (the edge swipe reveals the list under the card and the card follows the finger; there is no history stack), interactive dismissal of the middle card, drafts and pending submissions for **threads** (only notes have server items to attach them to; the Notes app uses them).
- Built in this round: drafts and pending submissions on the server adapter (Notes), pull to refresh on the sidebar lists, the card following the finger on the edge swipe, sheet dismissal by dragging its grabber, paste-a-wallpaper-link, the storage list with sizes, pill-contrast hazards.

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

Open: the composer palette is built (see the table below).

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
| Composer on the palette's engine (highlighted words, @ picker, / skills, context chips, Comment quotes) | yes | `e2e/composer.mjs` (8 checks), `palette/mentions.test.ts` |
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

## Update 2026-11-17 (evening)

| Item | Done | Evidence |
| --- | --- | --- |
| Changes, Files and Run rows share one anatomy; four right tabs with icon and label | yes | `PanelRow.svelte`, `panel-*` shots |
| Default wallpaper EX_STserBZM; lighter renditions for the light scheme | yes | `scripts/wallpapers.mjs`, grids on #15 |
| Stray underline: the active-children bar drawn bare; now a glass pill | yes | #15 comment |
| First-run gallery reachable with `?firstrun=1`, tested | yes | `e2e/firstrun.mjs` |
| Auto-hide setting for the card switcher (off by default) | yes | `appearance.app-switcher-auto-hide` |
| Composer palette: plan, ported recogniser, search, palette surface | yes (the thread composer now uses the engine) | `docs/ui-composer-palette.md`, `e2e/palette.mjs` |
| Veil from the study's function, on rendered pixels | yes | `e2e/veil-pixels.mjs` 16 of 16 |
| Settings pages with stable ids, in the palette | yes | `apps/settings/registry.ts`, `settings-*` shots |
| Documentation site themed with the design system | yes | `apps/docs/`, `/docs/` |

Open: the ink flip makes a sidebar light next to a dark card in the dark scheme on bright photographs (Barrow, Hoefler): the owner should judge it on the preview; turning it off is one line (`solve` in `veil.ts`, the `other` argument). The card can be almost fully transparent on light generated looks (veil 0.02), because the floor is that low there.

## The real-server adapter (2026-11-17)

`apps/web/src/server/` is a client for what the server on `origin/dev` already offers: `describe`, the operations catalogue (`POST /api/v1/operations`: spaces, notes, plugins, jobs, secrets, drafts, pending), the live space stream (`GET /api/v1/live`, SSE) and the jobs stream
(`GET /api/v1/jobs/<id>/events`, SSE), with types from `packages/contracts`. The Notes app sits on a `NotesClient` interface with two implementations: fixtures (the default) and the server.

**Sign-in is not wired for browsers yet, so this uses the development token mode:** start `calternal serve --data-dir <dir> --development`, then open the app with
`?server=1&token=<contents of <dir>/dev-tokens/owner.token>` (the token is kept in localStorage and removed from the address; it is never printed). In development, `vite` and `vite preview` proxy `/api` to the server
(`CAL_SERVER`, default `http://127.0.0.1:3210`) and present the server's own Host and Origin, which its checks require. `?server=0` goes back to fixtures. The preview host serves the static build only, so it runs on fixtures.

Checked against a real server (`e2e/server-notes.mjs`, 7 of 7): the sidebar shows the server's space and notes, a new note is created, an edit is saved with `base_revision`, a note written by another client appears through the live stream, `describe` lists the 34 operations,
a wrong token is refused. The jobs stream is covered by unit tests on a mocked response (`server/server.test.ts`), not by a real job, because running a job needs the plugin worker.

**Fixture calls that still have no server equivalent** (the server has no orchestrator or thread operations yet; every one stays on fixtures):

| Fixture call (`ThreadsClient`) | Server today |
| --- | --- |
| `listThreads`, `watchThreads`, `messages`, `watch`, `send`, `stop`, `rename`, `archive`, `unarchive` | none: no thread operations |
| `setProvider`, `setPermission`, `setRunsOn`, `providers` | none |
| `machines` | none as an operation (machines are managed by the `calternal machines` CLI and the machine protocol) |
| `requests`, `watchRequests`, `answer` (questions, approvals) | none; secret requests have `secrets.request` / `secrets.list` / `secrets.revoke` and `POST /api/v1/auth/secrets/provide`, not wired yet |
| `changes`, `run`, files for the right panel | none |
| the thread composer's drafts (in memory) | `drafts.*` need an item UUID; threads are not server items yet. **Notes drafts are wired** (`apps/notes/input.ts`) |
| queued thread sends | `pending.*` need a server write operation to queue. **Notes updates are queued and sent** (`pending.queue`, `pending.submit`, the account feed) |
| palette search over spaces | no search operation (only `notes.list`); the palette searches fixtures |
| appearance, shortcuts and the other Settings values (localStorage) | no settings operation; `plugins.list` / `enable` / `disable` exist for the Plugins page, not wired yet |
| account, passkeys | `/api/v1/auth/*` exists; the web sign-in flow is not built |

Open: nothing in the adapter retries a dropped live stream beyond five attempts after a `reset`; a visible "reconnecting" state belongs with the real sign-in work.

## The editor port, stage 1 (2026-11-17)

Plan: `docs/editor-port-plan.md`. Built: `packages/editor` and `InlineEditor` on `FieldEditor`; `e2e/composer.mjs` (10 checks, now including undo and redo of a picked mention), `packages/editor/test` (16), `e2e/field-probe.mjs`. Not started, by instruction: stage 2 (the CodeMirror block editor in the Notes middle card) and stage 3 (live editing, which needs a server text-patch operation).

## The editor, stage 2 completed (2026-11-17)

Notes pickers for `@`, `[[`, `#` and `/` (one shared picker component with the composer); lazy frontmatter hiding, block ID markers (one module), table grid and code-block grammars; WebKit measured in the iOS Simulator (`docs/ui-performance.md`). Checks: `e2e/note-pickers.mjs` (8), `e2e/note-pieces.mjs` (12), `packages/editor/test/lazy.test.ts` (13 new). Details and what is not done: `docs/editor-port-plan.md`.

## Phone navigation (2026-11-17)

The sidebar and side panel are history entries on a phone; edge swipes, the card and the panel follow the finger and settle on the sheet tokens, and a touch mid-settle takes hold of them. `docs/ui-shell.md` "Phone navigation"; `e2e/phone-nav.mjs` (15). Fixture spaces are no longer called "Home" (a space has a UUID and a name such as "Personal"). The palette's "Ctrl+K opens the palette with the field focused" check failed once in three runs (a focus timing flake, not in this change).

## Audit against docs/ui-shell.md and docs/settings.md (2026-11-18)

Method: `e2e/shell-audit.mjs` drives a fresh browser profile (nothing stored) at desktop and phone widths and checks the decisions one by one (about 60 checks, all passing now); the faults the owner saw in the preview came first. A fix has its check in `shell-audit.mjs`, `first-visit.mjs`, `flat-sidebars.mjs` or `resize.mjs`.

| Decision | State found | Fix |
| --- | --- | --- |
| Sidebars are flat (no card, border, strip or edge beside the card) | The veil stopped at the sidebar, leaving a strip of bare wallpaper in the gap beside the card | The veil runs on under the gap (`Backdrop.svelte`); pixel check across and down the full height, both schemes, four bright wallpapers, plus the first-run state |
| A closed sidebar or side panel can be opened again | Only some pages had a button | A pill at the card's corner with its shortcut in the tooltip, in every app; the right sidebar has its own hide button, leading (the left one trails) |
| Account picture opens the Settings app (settings.md); no settings sheet on desktop | An account sheet, then a settings sheet for wallpaper and fonts | The picture opens `/settings/appearance`; wallpaper and fonts are a page of the Settings app (a phone keeps its bottom sheet) |
| App switcher: a three-label capsule at the bottom of the middle card | An experiment, off by default | Default; the Settings choice stays |
| The footer (account, switcher, search) is in every app | Settings pages taller than the window stretched the shell grid; the footer sat 1000 px below the window | The grid row is the window's height; settings pages scroll inside the card |
| Default wallpaper from the first paint, no stored setting | Appeared only after the async blur, and a first visit wrote its defaults to storage | `index.html` paints tiny pre-blurred renditions of the default photo from the defaults; nothing is stored until a change |
| Right sidebar: AI chat by default, then the app's panels | Changes first; Notes had no chat | Chat is the first tab in every app |
| Sidebars resize | No handles | A handle at the vertical centre of the gap (hover, pointer, keyboard, range, reset, remembered per device) |
| Top and bottom fades fade to nothing | A kinked curve whose end showed as a band | A smoothstep ramp over 1.5 times the bar height (GPU-backed Chrome before and after) |
| Wide chat (the plugin's) is the desktop default | A 44 rem column | Full-card column, 3.25 rem and 1.5 rem margins, your bubbles right with tails; Appearance > Chat layout switches back |
| First-run chooser | A bordered card in the right sidebar | Decided: "a short gallery of presets, then straight in" (settings.md) and onboarding "pick a look, name your space, sign in to a provider"; where it sits was not decided. Now a centred window-level dialog (a phone: bottom sheet) |
| Settings: User, Admin, the current space | User, "Admin: this space", "Admin: server" | Three sections; the third is named for the current space |
| Motion follows the system's reduce-motion setting, plus an off switch | No off switch | Appearance > Animations |
| Roles can go to a group as well as a person | No such control | Members and roles: Add a person, Add a group (fixture buttons) |
| Spaces (decisions 2026-11-18): dots, current space only, personal space | Not built | `SpaceDots`, `spaces.svelte.ts`; threads and notes carry a space; the palette searches all; opening an item of another space switches |
| Differs, not fixed | | |
| Admin shows only if you manage something | Always shown (the fixture person is the operator) | Not wired to a role |
| Destructive actions ask for a fresh passkey | A fixture alert | Needs the real passkey flow |
| Machines are added through a guided flow with the SSH option | A fixture button | Not built |
| Onboarding also asks to name the space, sign in to a provider and whether you are a developer | Only the look is asked | Not built (the server owns sign-in) |
| The default provider and model are per thread, remembering the last choice | Per thread; no "last choice" for a new thread | Threads lane |
| The composer doubles as the command palette | A separate Cmd-K palette on the same engine | Question for the owner |
| Phone: the app switcher is a capsule under the card | A pill in the sidebar footer (the bottom of the card is the panel tab bar) | Question for the owner |
| Bottom band / edge at the right sidebar's tab capsule (owner) | Not reproduced in headless Chromium, GPU-backed Chrome on macOS (3 window sizes, dark and light) or WebKit on an iPad simulator: the only edges are the capsule's own glass outline and the card's edge and shadow | Waiting for the owner's window size or zoom |

## Design review pass (2026-11-18)

Lenses: Emil Kowalski's design-engineering rules, Jakub Krehel's interface skills (accessibility, layout), Hallmark's floor; headed GPU-backed Chrome on the macOS VM (desktop 1500x970 and 1280x720, dark and light; Threads, Notes, Settings with Compare, closed sidebars, popover) plus the earlier iPad (WebKit) and iPhone checks. Fixed where the documents already decide:

| Flag | Where | Fix |
| --- | --- | --- |
| Animates width and height (Emil: transform and opacity only; Hallmark floor 2) | Space dots, resize handle | Scale and opacity only |
| Middle-dot meta strings (Hallmark floor 10) | Palette group "Settings · Appearance", Notes sidebar "Personal · server" | Plain commas and parentheses |
| Touch target under 44 px (Jakub, minimum hit area) | Space dots | 44 px wide on a coarse pointer |
| A control that overflows its container | The right sidebar's tab capsule at the narrowest width (and on an iPad) | Segments shrink and truncate; `resize.mjs` checks it |
| Square focus rings on rounded controls | The two resize grips, the side-chat field | The ring is drawn by the round bar and by the rounded box (`e2e/ring-shell.mjs`) |

Listed, not changed (not the shell's, or not decided): the composer's chip labels still join with a middle dot ("Codex · GPT-6.1 Sol", "calternal-dev-02-std · Sandboxed") (Threads lane); popovers do not yet grow from their trigger (Emil: origin-aware; the attempt is the saved patch); `left` and `right` are physical, not logical, properties (nothing decided about right-to-left); the Compare heading's colon is temporary.

## Settings against docs/settings.md (2026-11-18)

| Decided | State |
| --- | --- |
| Keyboard: shortcuts follow the account, remap in place | A page listing every shortcut with Change and Reset (the same list as the quick sheet) and the account's shortcuts file as it would be stored (`id: chord` lines); `e2e/settings-pages.mjs` |
| Machines are added through a guided flow, with the SSH option; sandboxed or direct | The list with a Sandboxed or Direct choice per machine, and a three-step flow (how, where, check) that adds one; fixtures |
| Plugins on one page: installed, available, what each may access; a plugin's own settings open from its row | Installed (switch, what it may reach, its options opening from the row) and Available (Install); a fixture catalogue |
| Appearance as decided; agents; account; the space and admin pages | Built earlier (see the audit above) |
| Waiting on a design round, left out | Skills, automations and onboarding beyond the look (their pages are not in Settings until decided) |
| Needs the real server or passkeys | Destructive confirmations, the passkey list, provider sign-in, members and roles (fixture buttons) |

## Simplify pass (2026-11-18)

Read `shell/`, `appearance/`, `palette/`, `lib/` and `packages/ui` for exports nothing uses (a scan of every export against the whole repository, tests included) and removed what was dead: `setLook` and the unread `lastVeils` (wallpaper), `settingById` (settings registry), `fmtTime`, `fmtDay`, `dayName`, `monthName` and four constants behind them (palette dates), `adaptiveCount` (material), `springEasing` (spring engine), `scrollToIndex` (VirtualList); and took `export` off seven things only their own file uses (`THIS_SPACE`, `VISIBLE`, `keyChips`, `resolveScheme`, `FLOORS` and others). The Settings pages' own controls (shortcuts, machines, plugins) load when their page opens, so they are not in the first load. Not changed, with the reason: the translucent fills (`rgb(var(--ink-rgb) / 0.05 ... 0.1)`, 40 uses in six values) cannot become one root token, because a token computed at the root would not follow a sidebar's flipped ink (`data-ink`); the two `isMac` helpers (the app's shortcuts and the editor's text keys) stay apart because the editor package must not import from the app. Tests: 172 and 49 pass. First load: scripts 299.6 KB before the Settings pages of this round, 301.4 KB after them and these removals (raw; gzip 108.3 and 109.3 KB), stylesheet 71.3 and 71.8 KB.

## Server mode beyond sign-in (2026-11-18)

With `?server=1` Settings shows only what a server operation backs. Real now: **Account** (name and role read only, passkeys add/rename/remove with the fresh-passkey step, installations, sign out, sign out everywhere), **Members** of the current space (list, role, remove, leave), **Secrets** (names and labels, never values), **Space plugins** (list, switch), **Delete space**, **Machines** (list, state, sandboxed/direct, pause, revoke, enrollment code). Appearance and Keyboard are this browser's own. Admin has no operations at all, so no Admin section shows even for an operator (the capability is read from `/auth/me`: `server.settings`). Fixtures show everything as before.

Shell states: **reconnecting** (any call or stream that cannot reach the server: a pill, probed every 3 s), **session ended** (a 401: a pill, then a reload into the sign-in screen at the same address), **version** (`describe.protocol_version` differs from this build's: a plain notice). Tested in `apps/web/e2e/sign-in.mjs`; the version notice is not (needs a server of another protocol).

### Controls hidden in server mode: the operation needed

| Page | Control | Operation needed |
| --- | --- | --- |
| Account | Connected provider accounts | an operation listing and disconnecting provider accounts |
| Agents | Every control (retry, snooze, sandbox idle time, decision service, defaults) | `agent.settings` exists but its fields do not map to these controls yet; needs the list of settings and their bounds |
| Plugins (user) | Installed / available catalogue, per-plugin options | `plugins.install`, `plugins.uninstall`, a plugin's declared settings read and write |
| Members and roles | Transfer ownership | none in the UI yet (`spaces.transfer` exists; no control built) |
| Secrets | Delete one, delete all | `secrets.revoke` exists; no control yet for space secrets by name |
| Instructions and defaults | Instructions (AI/AGENTS.md), default look | space file read/write, space settings |
| Space plugins | Turn off every plugin | none needed; not built (one `plugins.disable` per plugin) |
| Machines | Add by SSH (guided flow) | server-side SSH enrollment; the enrollment code replaces it |
| Admin | Machine pool | `pool.list`, `pool.drain` |
| Admin | Storage | `storage.status` for the three roots |
| Shell | Personal space | server creates it at sign-up (then delete `withPersonal` in `spaces.svelte.ts`) |

## First use, failures, accessibility, light wallpapers (2026-11-18)

- **Empty states (server mode):** Notes, Machines (what a machine is, Make a code, the enroll command), Secrets, Members, Plugins each say what the thing is and offer the one next action. Failures use `server/say.ts` (plain words plus the next step, by code, never the server's message). Screens for server unreachable, version mismatch and a wrong or used setup code.
- **Accessibility:** `e2e/a11y.mjs` (fixtures, every screen and Settings page, both schemes, desktop and phone) and `e2e/a11y-server.mjs` (real server: setup, sign-in, shell, every Settings page, failure screens; 56 checks) are clean. `e2e/a11y-walk.mjs`: keyboard walk with focus indicator and names on the threads, notes and three Settings pages in both schemes, the palette opens, keeps focus and returns it on Escape, reduced motion leaves no movement, 200% zoom (720 px) has no sideways scroll and no axe violation. Found and fixed: Plugins group headings jumped from h1 to h3; the sheet returned focus to a field that had gone (now to where focus was before any sheet).
- **Light wallpapers:** the eight built-ins checked in a headed GPU Chrome (screenshots on issue #15); nothing to fix. Phone width needs a real device or the WebKit simulator, not that Chrome.
- **Served by the server itself:** deep-link reload and `blob:` wallpapers work with the current server (CSP and fallback in dev); the personal space comes from `personal: true`.

## Notes on the real server (2026-11-18)

Through the same adapter interface as fixtures (`apps/web/src/apps/notes/client.ts`): list, open, create, edit and save (whole-note replacement checked against the revision), rename (the first heading is the title), and following the space journal (live stream, no polling). The state of the open note is said in the header: Not saved yet, Saving, Saved, Changed elsewhere, or "Can't reach the server. Your text is still here and saves when it is back". The editor's text is never replaced by a save, so typing during a save is safe; a conflict keeps the typed text on screen and offers Keep mine or Use theirs; drafts use the account's `drafts.*` operations. Tested end to end in `e2e/notes-server.mjs` (real server, virtual authenticator): first note, saving, rename, a change from elsewhere (appeared in about 100 ms), conflict, offline and back, and a 10,000-line note.

Development-host numbers (dev-02, debug server, Chromium): typing in the 10,000-line note while saves ran, 135 keystrokes, median 2.1 ms, p95 3.1 ms, worst 6.8 ms to paint; operations round trips seen by the page (saves, list refreshes, draft writes), median 5 ms, p95 118 ms, max 438 ms.

| Page | Control | Operation needed |
| --- | --- | --- |
| Notes | `@` people | a members-with-names operation (also needed for Members); only agents and threads are offered |
| Notes | Live co-editing | rooms; until then saves are whole-note with a revision check |

## Start-up, people, Notes lists, adapter clean-up (2026-11-18)

**Start-up in server mode (`apps/web/e2e/startup-server.mjs`, dev-02, median of 9 loads, ready = `data-ready`, the shell painted).** The shell now paints at once from a small record of what this device knows (account, capability names, spaces; `server/cache.ts`, under a day old, cleared by sign-out and by a refused session) and the server's answer confirms it a round trip later; a first visit, a signed-out device or an old record still waits for the server (no flash of the wrong screen). The four checks (session, protocol, who, spaces) run in parallel, not one after another, and begin before the fonts finish.

| | before | after |
| --- | --- | --- |
| warm, no added latency | 254 ms | 63 ms |
| cold, no added latency | 316 ms | 200 ms |
| warm, +100 ms each way | 851 ms | 62 ms |
| cold, +100 ms each way | 1428 ms | 943 ms |

Fixtures are unchanged (first paint 108 vs 108-112 ms, interleaved). Known limit: a device whose session was revoked elsewhere paints the app from its record for one round trip, then turns into the sign-in screen; signing out clears the record, so that case is a remote revoke only.

**People (`server/people.ts`, generated contract types).** Members with names and roles, role change, remove, ownership transfer, add a person or a group with a role, the Roles page, Invites (make with a role and a length of time, copy the link, revoke), the join page for an invited person (`/invites/<secret>`: name and passkey, lands in their own space with the invited one added; an existing account opens the link and joins), account name edit, Admin People (disable) and Groups (make, rename, delete, put people in and out), each by capability. Tested in `e2e/people-server.mjs` against a server built from this tree.

**Notes (server mode).** Delete to Trash with Undo, a Trash view with Restore and Delete for good, rename from the list, a tag filter, and the `[[` and `#` pickers asking the space's index (title and tag lookups, one request per pause).

**Typing cost found on the way.** After keys typed in a field stopped switching the page to keyboard mode, typing in a 10,000-line note cost about 7 ms a key more (median 3.4 to 10 ms): keyboard mode had been switching every animation off, hiding the editor caret's blink animation restarting at every key. The caret now holds still while keys are typed (`packages/ui/src/styles/base.css`, with the other typing pauses). Back to a 3 ms median.

**Adapter clean-up (`apps/web/src/server`).** One request path in `ServerClient` (`send`, `stream`, `json`) instead of eight `fetch` wrappers; one error type (`ServerError`, with `AuthError` removed) worded by `say.ts` everywhere; the account, passkey, machines and people routes all use `server.json`; `spaces.load()` is the only place the space list is read (the Threads adapter had its own); `capabilities` is reactive. Size: `auth.ts` 20 lines shorter, the machines panel 5, `say.ts` 1; `client.ts` gains the shared helpers (26 net), so the total is about the same with one path instead of eight, not a large cut.

Missing now: delete-for-everyone accounts, directory groups from outside (shown read-only), the invite link's address on the preview host (the server builds it from its own origin: `/invites/<secret>` at the root, which Caddy must route to the app under `/ai/`).

## Wallpapers from Unsplash through the server (decided 2026-11-18; not built)

Decision: wallpapers are fetched from Unsplash on demand with the key that already exists in the production secrets file; hosting them ourselves is a later discussion. Today the web build downloads the eight default photographs at build time (`apps/web/scripts/wallpapers.mjs`, cached in `~/.cache/calternal-ai/wallpapers`; Unsplash refuses some machines, which is why dev-01 needs the cache). This is how it should work instead; nothing here is built.

**The key never reaches the browser.** The server holds it as a server secret (the same secrets mechanism as the provider credentials, read at start-up from the production secrets file; never in `dist/`, never in a response, never logged). The browser asks the server for pictures and gets them from the server's own origin; it never talks to Unsplash and never sees the key (so the CSP stays `img-src 'self' blob:` and `connect-src 'self'`).

**What the server does** (a backend lane; operations in the usual envelope, `wallpapers.*`):
- `wallpapers.list`: the photographs on offer: the eight defaults (ids from `wallpapers.json`, photographer and Unsplash link for the credit tooltip) and, later, a search (`query`, cursor-paged, 30 a page) through the Unsplash API. Reads need a signed-in session; results are cached on the server (a day for lists).
- `GET /api/v1/wallpapers/<id>/<rendition>`: the picture itself, same origin. Renditions as today: `full` (1600 px, dark), `light` (the brightened 1600 px copy, made by the image host's parameters or by the server), `thumb` (360 px), `blur` and `blur-light` (96 px pre-blurred, for the first paint of the default). The server fetches from Unsplash on the first request with the key (the API's download endpoint, which Unsplash requires to be called when a photo is used), stores the renditions in its own cache directory (about 0.9 MB for a photograph, 7.3 MB for the eight), and serves every later request from there with long `Cache-Control` and an `ETag`; the browser caches them too. A photograph that is not cached and cannot be fetched (no key, no network) answers 404 and the client falls back (below). Unsplash's rate limit (50 requests an hour for a demo key, more for a production key) is spent only on first fetches, so it is not a per-visit cost.
- Attribution: the list carries the photographer's name and profile link; the client shows them in the existing credit tooltip.

**What the build and the fixtures use with no key.** Nothing reaches the network at build time: `npm run build` no longer runs `wallpapers.mjs`. The client has one generated look (the mesh the app already derives from a palette) that needs no picture; the default photograph's `blur` and `blur-light` renditions (4 KB each) are the only pictures the repository may carry, so a first visit still paints a wallpaper-coloured ground before anything is fetched (decided with the owner if they should be committed; they are small enough, and they are what index.html paints before any script runs). The preview host and the fixtures show the mesh plus those two, or the photographs when the server has them.

**What I need from the backend:** the three things above (`wallpapers.list`, the rendition route with its cache directory and the key read from the secrets file, and a `wallpapers.search` later); a decision on who makes the `light` rendition (the image host does it today with brightness, contrast and saturation parameters on the resize URL; the server cannot do that on a plain download, so either it keeps using the host's resize URLs with the key-authenticated download, or it renders `light` itself with an image library); and the cache's retention (the eight defaults stay; others age out). The client side (the picker asking `wallpapers.list`, the images' addresses, the fallback) is small and I can do it as soon as the operations exist.

## The canvas experiment (2026-11-18, `?canvas=1`, fixtures build only)

**What it is.** Inside the middle card, an endless horizontal strip of columns (niri's model: columns side by side, windows stacked inside a column) holding windows, each one an app's item. A new module `apps/web/src/canvas/` beside the shell; the shell changes in three places: `Shell.svelte` draws `<Canvas />` instead of `<app.Main>` when the flag is on, `appearance/wallpaper.ts` sets `--wall-sharp` (the picture itself, no new decode), and the flag is read once at load (`canvas/model.svelte.ts`). Without `?canvas=1` nothing changes (checked by `e2e/canvas.mjs`; the flag is remembered for the visit, `?canvas=0` leaves, and it is ignored with `?server=1`).

**How an address maps to a window.** A window's address is `/<kind>/<item>`, one window per address. Opening an address (a sidebar click, a link, Back) focuses its window or opens one in a new column right of the focused one. The address bar follows the focused window, so the left and right sidebars, which the shell already chooses by the address's kind, follow the focused window's app with no code of their own. The arrangement is kept in the browser (`cal.canvas`).

**Behaviour built** (all in `e2e/canvas.mjs`, about 45 checks, dark and light): threads open twice and a note beside them; tile to the side or below (the bar's two buttons); focus follows the press and slides the column into view; resize by the right edge (column) and by the boundary under a stacked window; maximise merges a window into the outer card (it fills it, its own surface is dropped and the card's material shows) and restores; the scroll rule (a wheel over the focused window never moves the canvas; over an unfocused window or a gap it does; a vertical wheel pans sideways); the input lock, a padlock on the window's top frame line at its middle (infraternal's D177: invisible until the pointer is near, 35 %, locked 90 % in the accent, 44 px hit area; shackle animates), with the canvas frozen while any window is locked; two fingers pan in any direction on touch with a short fling, one finger scrolls the window under it.

**Compositor-only panning.** The strip is one element moved by `transform: translate3d`, written straight to its style in an animation frame; no reactive state and no layout per frame. A column is mounted a viewport before it can be seen and unmounted only two viewports away (hysteresis, so back-and-forth never remounts), so windows off screen are not in the DOM at all: not laid out, not painted, not styled. **Measured** (dev-02, headless Chromium, 4x CPU throttle, 5 columns holding 6 windows, 120 wheel events at 16 ms, 365 frames): p50 16.7 ms, p95 16.8 ms, and on a quiet VM 0 late frames (> 24 ms). On a loaded VM (load average 14 to 24, other lanes building) the same run has 7 to 13 late frames of about 375, max 33 to 67 ms, and the same counts with the edge fade switched off (A/B with `CANVAS_NOFADE=1`), so they are the VM's load and the occasional column mount, not the fade. Before the hysteresis and the one-viewport lead there were 2 to 18 late frames, max 150 ms: mounting a thread view while panning. Frosted windows are not in this measurement (a live backdrop blur over a panning window is the expensive case; they are off by default).

**The look, with the measured values used** (from the owner's five Craft screenshots, 2992 px wide at 2x, so 1 css px = 1 image px / 2; pixel scans with a small decoder, not the DOM; Craft was not driven on the Mac Studio, the screenshots were enough):

| Thing | Measured in Craft | Used here |
| --- | --- | --- |
| Backdrop in the card | the photo at full sharpness around the page; sidebars stay tinted | `--wall-sharp` (the unblurred picture) fixed to the viewport behind the windows; the sidebars are unchanged |
| Document colour, light | `#e7deda` (OKLCH L 0.907 C 0.011 h 46) on a warm rug photo; `#fbf5de` (L 0.969 C 0.031 h 95) on a blue to pink gradient | L 0.935, C 0.022 at the wallpaper's hue, in sRGB gamut |
| Document colour, dark | `#191c1f` (L 0.225 C 0.007 h 248), `#262626` (L 0.269 C 0) on black and white, a very dark floral | L 0.22, C 0.02 at the wallpaper's hue |
| Text on it | Craft's text colour is its own preset choice (a deep red-brown on pale, a sand on dark) | solved: the scheme's ink if 7:1 on the exact colour, else the other; quiet inks moved toward it to 5.5:1 and 4.5:1 (veil.ts floor); unit-tested over 12 hues |
| Page to card edge, sides | about 52 pt (70 display px at 0.75 pt each) | not copied: the canvas uses a 12 px gap between windows |
| Page to card top | about 85 pt (cover and buttons live there) | the bar is 36 px |
| Window corner | about 12 pt | 14 px (`--canvas-r`) |
| Shadow | a soft drop, no hard edge | `0 1px 2px / 0.14` and `0 6px 22px / 0.16`; focus is a 1.5 px accent hairline |

How the page colour follows the backdrop across the five screenshots: it does not follow it continuously. Craft's document colour is part of each style preset (the same photo gets a cream page under the blue gradient and a pale pink one under the rugs); the lightness is always in the same two bands (light 0.91 to 0.97, dark 0.21 to 0.27) and the chroma is always low (0 to 0.03). The owner's words ("automatically accent coloured") are met here by taking the hue from the wallpaper and the two bands from Craft. The radii, the gap and the shadow are eyeballed to within a few points from scans, not exact.

**Edge fade** (the owner's request): minimal and visible only while a window enters or leaves through the left or right edge, never a standing band. Two 28 px strips at the edges paint the same sharp picture as the area around the windows (fixed to the viewport, so they match it exactly) under a static gradient; their opacity is 0 unless a window crosses that edge and rises to 1 over 24 px of crossing, so only opacity changes while panning (composited). A mask on the windows' parent was tried first and made the whole parent repaint per frame (p95 50 ms at 4x CPU). Top and bottom are not drawn: the canvas has no vertical scroll, so a window never crosses them. A window left half cut off at an edge keeps its fade while it is cut off.

**Frosted windows.** A per-window button and a global toggle in the canvas bar swap the opaque colour for the same adaptive material as the floating surfaces (`Glass` + `adaptive`). What I took from the old Calternal (`origin/review/switcher-a-pills` TASTE.md and mockup README, read with git only): real blur plus a light tint, legibility fixed with blur and material and not by making the glass nearly opaque (TASTE line 78); the "Paper / Frosted card / Clear on photo / Plain" presets as the model for a later per-window choice. Not taken: "text never sits on glass" (TASTE line 22) is not honoured here, because the contents are whole apps; a frosted window's ink is the scheme's and the adaptive tint is what keeps it readable. Open question for the owner: is that acceptable, or should frosted be reserved for frames and chrome?

**What did not work or is missing.**
- Notes keeps one open note in one store: only the focused Notes window is the live editor, the others show a read-only copy (`NotePreview`). Two live note editors need the store split per window.
- The Threads composer, `#composer-field` and the thread view's own header exist once per window: ids duplicate (a11y), the thread's own title pill sits under the window's bar, and on a phone the shell's bottom tab bar sits over the focused window.
- Opening a thread in a window does not stop a run or detach anything: the views are plain instances of the existing components.
- Vertical canvas scrolling is not built: stacked windows share the column's height (weights), so the canvas pans on one axis; a column taller than the screen would need it.
- Maximise remounts nothing but resizes the column once; the sharp backdrop fades out so the card's own material shows.
- The one-finger sidebar pull and the canvas's two-finger pan do not conflict on a phone, but the pull still starts on the first finger of a two-finger touch.
- No zen mode (as asked), no snapping, no overview, no per-window zoom.

**What the shell would need to adopt it.** (1) An app's `Main` that takes an item id and holds its own state (Threads already does; Notes does not). (2) The address model above, with the focused window as the address and an app-level "open this item" that may return an existing window. (3) A per-window slot for the header controls the apps draw into the card (so the bar and the thread's own header are one). (4) The tokens `--canvas-r`, the window shadow and `--wall-sharp` moved into the appearance module and the generated tokens. (5) A decision on where the canvas arrangement lives for a person (a file per account, like the shortcuts) and a per-device camera, as infraternal does (D178).

**Simplification opportunities.** The registry's `attention` field and `shell/Attention.svelte` have no user left; `Shell.svelte` is over 300 lines and its pull logic could be its own module before the canvas adds a second gesture owner.
