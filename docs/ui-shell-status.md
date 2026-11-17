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
| Tokens (one JSON source), fonts, adaptive glass, 13 components (surfaces, controls, icons) | `packages/ui` |
| Shell: tinted background, sidebar (header, attention card, body, footer with account, app switcher, search), middle card, right card with tabs at the bottom, narrow-screen layers and the phone's bottom tab morph | `apps/web/src/shell` |
| Routes as plain paths (`/threads`, `/threads/<id>`, `/notes/<id>`; the open right-card panel in `?panel=`) | `apps/web/src/lib/route.svelte.ts`, `App.svelte` |
| App contract (what a plugin fills: sidebar body, middle card, panels, attention) | `apps/web/src/apps/registry.ts` |
| Threads: sidebar (grouped by workspace, children nested, running/waiting/failed/finished), thread view, virtualised list, folded steps, long-log viewer, streaming row, composer with three separate chips (provider and model, permission mode, where it runs), sticky request bar (question, approval, secret), Changes/Run/Files tabs | `apps/web/src/apps/threads` |
| Typed fixture module and the one adapter interface (`ThreadsClient`) | `threads/fixtures.ts`, `generate.ts`, `client.ts`, `model.ts` |
| Generated-UI renderer over `packages/genui` (Card, Stack, Row, Text, Callout, Badge, Divider, Code, Details, Progress, KeyValue, Table, Form, Input, TextArea, Select, Choice, Radio, Toggle, Checklist, Button; a plain fallback line for the other catalog components, an error line for an invalid description) | `apps/web/src/genui` |
| Notes placeholder (narrow centred column, no inner card, left rail that opens a table-of-contents card) | `apps/web/src/apps/notes` |

## Screenshots

Evidence images live on issue #15 as attachments, not in the repository (project rule). Taken by `e2e/shots.mjs`
(Chromium, device scale 2, JPEG 90): desktop 1440 by 900, phone 390 by 844 with touch. Re-run the script to refresh them.

| Screen | Image |
| --- | --- |
| Desktop light: thread with a running child, Changes tab, attention card | [image](https://git.kayg.org/attachments/2a8d6f11-baca-4759-81b1-c77ef21d33e2) |
| Desktop dark | [image](https://git.kayg.org/attachments/b9762dc9-0114-431a-849c-b9040708d712) |
| Composer: permission mode menu (Least access, Approve for me, Full access) | [image](https://git.kayg.org/attachments/f1b0645a-d9ab-4d48-9040-774df56fe81e) |
| Request bar: question form | [image](https://git.kayg.org/attachments/7cc1903a-405e-47f1-b57d-d035109dbe19) |
| Request bar: approval (second of two) | [image](https://git.kayg.org/attachments/c4a631aa-fab7-4910-92b3-cd346da6d816) |
| Request bar: secret, dark | [image](https://git.kayg.org/attachments/a6a010ed-f070-48b4-894f-a5c6bffa5a1a) |
| The 10,000 message thread | [image](https://git.kayg.org/attachments/bd752037-6b70-4bd5-a595-7669d319cfbf) |
| Notes: the second middle-card layout with the contents card open | [image](https://git.kayg.org/attachments/de5a1817-5e4a-4f95-9114-0b8a50785195) |
| App switcher expanded | [image](https://git.kayg.org/attachments/29e8a406-4abf-4834-a677-f363ca35a58f) |
| Phone light | [image](https://git.kayg.org/attachments/9910ab09-7ff2-4bbe-8dba-44ffa621734d) |
| Phone dark | [image](https://git.kayg.org/attachments/e5b79205-a20b-400c-b5b3-a4c79f164114) |
| Phone: where-it-runs menu | [image](https://git.kayg.org/attachments/ea3eee76-7dc7-494c-a3f3-6d62b6546762) |
| Phone: request bar | [image](https://git.kayg.org/attachments/f479ef84-444d-48d6-a467-1ce82ad2c675) |
| Phone: sidebar underneath, the middle card floating above it | [image](https://git.kayg.org/attachments/289c93c6-5eb1-4720-8d2a-c65d4dfb56a1) |
| Phone: right card slid over, bottom tab morph | [image](https://git.kayg.org/attachments/958a74de-f041-442b-a0de-7fc9b01a9186) |

## Spacing, alignment and sizing checklist

Per screen. "Script" items are checked by a script on every run (`audit.mjs`, `a11y.mjs`); "Eye" items were checked by
looking at the screenshot only, which is not a pixel measurement (`docs/native-ui-study.md` gives the target sizes; the audit checks the scale, not each pixel against it).

| Check | Method | Thread (desktop) | Request bar | Long thread | Notes | Phone (thread, bar, sidebar, right card) |
| --- | --- | --- | --- | --- | --- | --- |
| Only the three families are drawn (Google Sans Flex, its Dark and Rounded, Maple Mono) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Only the token type scale and the three weights (440, 460, 620) | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Every padding, margin and gap is on the `s-1`..`s-10` scale | Script | ✔ | ✔ | ✔ | ✔ | ✔ except two token-derived values (bottom inset 72, notes bottom 120, both built from tokens) |
| Hit targets at least 44 px on touch | Script | n/a (desktop 28) | n/a | n/a | n/a | ✔ |
| axe (WCAG 2 A/AA/21AA and best practice, contrast included), light and dark | Script | ✔ | ✔ | ✔ | ✔ | ✔ |
| Radii come only from the four tokens (pill, card 14, pane 20, select 8); the selected-row 8 is the native value and is not concentric | By construction, eye | ✔ | ✔ | ✔ | ✔ | ✔ |
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
| Glass blur 20 px (was 28 in the mockup's pane glass); the mockup's floating glass and the native study's closest CSS (section 8) are also 20 | Software compositors stop blurring above about 24 px, so larger values could not be verified here |
| No sky, photo or plain backdrop picker; one tinted mesh background | `docs/ui-shell.md` (one seamless background); the mesh is the adaptive material's data |
| Two schemes plus "System"; no Warm and Cool paper | Not asked for; tokens make it a small later change |
| Phone bottom bar is a plain flex capsule: the selected item widens and shows its name, the rest shrink to icons, on the sheet spring (the mockup's `Morph` is a measured, multi-mode bar) | Keeps the morph with far less code; the full Morph can replace `Capsule morph` later |
| No progressive blur strips, home-screen standalone handling, or Safari toolbar insets | Not needed for a first shell; to revisit with real-device work |
| Tokens are JSON, generated into CSS; the one spring is generated at build time, not installed by script | Native clients read the same tokens; no start-up script |
| Motion and spacing tokens collapsed: one `--inset` (mockup: five gap names), three durations and two curves (mockup: twelve), the control tier is one token, `--pill-h` (38 desktop, 44 touch, which is also the touch hit target), radii collapsed to pill, card 14, pane 20 and select 8 | `docs/native-ui-study.md` Simplification opportunities |
| Native values everywhere the study differs (owner decision 2026-11-17): sidebar and list rows 32, selected row radius 8, desktop text 13, desktop toolbar band 52 with 38 buttons, phone bottom capsule 49 sitting 27 above the bottom, phone title 34 (desktop 22), settings rows 44 desktop and 52 phone | `docs/native-ui-study.md` section 7.2 |
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
- **Message text size on a desktop.** `text-ui` is the native 13; the reading size of messages and notes (`text-body`) stays 15 because
  13 is small for long reading. Say if it should follow the native Mac body.
- **Panes that abut.** The study found list and editor panes flush with only the sidebar floating; here the middle and right
  cards are both floating cards separated by one `inset`. Say if the right card should be flush with the middle card.
- A second study pass (motion, dark, tap behaviour) is pending; it will change tokens only.

## Unfinished

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
