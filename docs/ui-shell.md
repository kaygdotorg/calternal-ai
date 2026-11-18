# App shell

Decided by kayg on 2026-11-17. This is the target for all UI work and replaces the layout
described in `docs/ui-preview.md`. Visual language comes from the mockup in
`/home/kayg/Developer/calternal` on branch `review/switcher-a-pills` (`apps/mockup`,
`packages/ui`), except where this document says otherwise. UI is implemented by Claude only.

## Principles

- Simplify wherever possible.
- Apple-native feel throughout. Icon sizes, spacing, and transparency/blur are tuned
  separately for desktop and for mobile, not shared values scaled down.
- Built from reusable surfaces: pill, capsule, toolbar, card. No one-off chrome.
- Transparency and blur are adaptive: a surface responds to what is behind it instead of
  using one fixed tint.
- One seamless tinted background runs behind the whole app.
- The shell must be extremely performant; measured on real devices, not assumed.

## Three regions

All three change with the current app and are exposed to the plugin API, with good defaults.

### Sidebar (left)

Not a card: it sits directly on the tinted background.

- Header: the app's name as a wordmark in a cursive font, with no icon. The name and its
  font are set in Settings → Appearance. No horizontal scrolling app strip.
- Attention card: things that need the user's attention, at most 3 items. It is a card
  because it asks for attention. With nothing to show, the card is absent.
- Body: app-specific. In Threads the person makes their own sections and drags threads
  freely between them; nothing is grouped under an automatic project label. Double-click
  renames a thread; right-click opens a context menu.
- The sidebar's content fades out at its top and bottom edges; no hard cut-off.
- Footer: account picture on the left (it opens the Settings app, never a sheet); a row of
  dots for the spaces in the middle (decided 2026-11-18, below); a search icon on the right.
  Switching always happens at the bottom: apps in the capsule under the middle card (on a
  phone, the pill in this footer), panels as tabs at the bottom of the right card, the tab
  bar on phones. The header's app name is a label, not a control; discoverability wins over
  removing the switcher.
- The sidebar shows only the current space's threads and notes (2026-11-18). The palette
  still searches every space and says which one a result is in. Addresses do not change:
  items are addressed by ID, never by space, and opening an item of another space makes that
  space current.
- **Space switcher** (2026-11-18): a row of dots at the bottom centre of the footer, like the
  Aside browser's profile switcher: one dot per space, the current one larger and filled,
  the name on hover, focus or long-press, a click or tap switches. Everyone has a personal
  space from sign-up, so there is always at least one dot. Motion from the tokens: press
  scales the dot at once (`--press-scale`); the marker moves between dots as size and opacity
  over `--dur-base` (220 ms) on `--ease-out`; the list under it fades in over the same; only CSS
  transitions, so a second tap redirects them (interruptible). Code: `shell/SpaceDots.svelte`,
  `shell/spaces.svelte.ts`.
- A closed sidebar or side panel always has a way back: a button at the card's top corner on
  that side, with its shortcut (Mod+\ and Mod+Shift+\) in the tooltip. The side panel has its
  own hide button in its header, as the sidebar has.
- Both sidebars are flat: no card, no border, and no strip or edge between a sidebar and the
  area around the card. The wallpaper's veil is one continuous surface that runs on under the
  gap beside the card (`e2e/flat-sidebars.mjs` measures it on rendered pixels).

### Middle card

- Where the app lives. Blurred and dimmed over the background.
- Its top and bottom edges touch the viewport.
- Notes default: a narrow centred column with no inner card, and a thin rail at the card's
  left edge that expands into a small table-of-contents card.

### Right sidebar

- Flat, directly on the tinted background like the left sidebar. It is not a card; only
  the middle is a card.
- AI chat by default, in every app (the first tab).
- Tabs at the bottom switch between AI chat and the current app's panels. The app's
  plugin decides which panels exist (for Notes: the inspector and style sheet).

## Phone navigation (2026-11-17)

On a phone the list (the sidebar) and the side panel lie under or over the middle card, and each is a **history entry**: showing one pushes an entry (the address does not change), hiding it goes Back, and Forward shows it again. The button, a swipe, a tap on the strip of card that stays visible, the scrim and the platform's own back gesture are all the same step. Choosing something in the list opens it, closes the list without a history step, and Back from there brings the list back over the note or thread you came from. Code: `shell.over`/`shell.dismiss` in `shell/shell.svelte.ts`, gestures in `shell/Shell.svelte`, proof in `e2e/phone-nav.mjs` (15 checks).

**Dragging the card away.** A touch starts a drag from the left edge (24 px) with nothing showing, from the visible strip of card or anywhere on the list while the list shows, from the right edge to bring in the side panel, and from the scrim or the panel to send it away. A row swipe, a sheet drag, a text field and a code block are not panel gestures.

Motion, in order, from the motion tokens (`touch` set):

1. **Following the finger**: the card (or the panel) is at the finger's position one to one, no transition; the panel's scrim fades with its progress and the card scales to 0.96 under it. Frame budget: transform and opacity only.
2. **Release**: it settles to the rest state the position and speed point at: faster than 0.4 px/ms (measured over the last 80 ms) decides by direction, otherwise past halfway commits. The state changes through `shell.over`, so history is updated at the same moment.
3. **Settle**: `transform` over `--dur-sheet` (340 ms) on `--ease-sheet` (cubic-bezier 0.45, 0.6, 0.2, 0.8), starting from wherever the finger left it. The scrim uses `--dur-base` (220 ms) on `--ease-out`. Reduced motion shortens these through the tokens.

**Interruptible.** A touch while the card or panel is still settling takes hold of it where it is (its current transform, read from the computed style), stops the transition, and carries on as a drag; releasing decides again. `e2e/phone-nav.mjs` checks the hand-over does not jump.

Measured: drag and settle at 4x CPU in Chromium: 56 frames, every one 16.7 ms (no frame over 33 ms), three runs. **Not yet checked on a real phone** (the Simulator cannot be driven with touch from here): the fluidity rule asks for that before this counts as done.

## Sidebar width, wide chat (2026-11-18)

- Each sidebar has a handle at the vertical centre of the gap beside the card: a slim bar that shows on hover, focus or drag. Drag it, or focus it and use Left/Right (16 px, Shift 64 px), Home/End, Enter or a double-click to reset. Left sidebar 200 to 420 px, right 260 to 560 px (unscaled); the width is kept per device. Code: `shell/ResizeHandle.svelte`, `e2e/resize.mjs`.
- Chat is wide on a desktop by default (the calternal-ui plugin's wide chat): the column fills the card with a 3.25 rem left gutter (room for a contents rail) and a 1.5 rem right margin; your messages are bubbles on the right with an iMessage tail, the agent writes from the left across the full width. Appearance > Chat layout returns to a centred column.
- The look chooser is step 2 of the onboarding (Look); there is no separate first-visit dialog (owner decision #28, 2026-11-18).

## Backgrounds and fonts

- The background behind both sidebars is a wallpaper: the person's own picture, one from
  a good built-in set, or one from Unsplash. It is blurred once when chosen, never per
  frame, so scrolling stays smooth.
- Fonts are a personal choice with a polished default (Google Sans Flex, Maple Mono):
  from Google Fonts, Fontsource, Fontshare, the person's own files, or the system font.
  Chosen fonts and wallpapers are stored on the server; pages never load from those
  services while in use.
- The reference for both, and for the overall polish, is kayg's bb plugin `calternal-ui`
  (`/home/kayg/Developer/bb-plugins/calternal-ui`). Calternal must be at least that good.

## Interaction rules

Decided by kayg, 2026-11-17, from the first live preview.

- Focus rings appear only for keyboard focus, never from a mouse or touch.
- Controls animate when used by pointer or touch, never when invoked from the keyboard.
- Motion is non-linear (springs or eased curves) and never blocks input; a surface never
  appears before its material is ready (no transparent popover that then blurs).
- Blur is cheap: wallpapers are blurred once when chosen; live blur is limited to small
  surfaces. Performance decides every material choice.
- The top of the middle card fades content out; no solid band.
- A caret sits exactly on the text baseline of its placeholder and text.
- Icons come from one set, at one optical size and stroke per tier, aligned to the text.
- State indicators say something or are absent: no dot for idle.
- A thread shown in the attention card is not also listed below it.
- Selecting text in a chat offers two actions: **Comment** (puts the selection in the
  composer as a quote, a blank line, then the caret) and **Ask in Side Chat** (opens the
  right sidebar's chat with the selection quoted).
- Interaction craft follows Emil Kowalski's and Jakub Krehel's published guidance and the
  Hallmark skill; visual presets follow the Craft study in the existing app's repo.

- Nothing draws out of bounds: rings, menus and popovers stay inside the window and their
  container.
- The accent colour is inferred from the wallpaper by default, never a fixed brown; it can
  be overridden.
- One design system defines every surface (pill, capsule, toolbar, card, menu) with its
  transparency and blur, and those adapt to the contrast of what is behind them, as the
  existing app's primitives do.
- Sidebars fade at top and bottom; no solid bands anywhere.
- Pickers (model, machine) show an icon per provider and have an inline search field.
- Message text is not streamed by default: a turn shows that it is working and its steps,
  and the finished message appears at once. Streaming is a setting.
- Keyboard shortcuts are discoverable with Cmd-/ or ? and configurable, as in the
  existing app.
- The composer doubles as a command and search palette that understands plain language,
  after the existing app's unified composer.

- No lone standard-sized icon: an icon that is a control sits in a pill. Deliberately small
  inline icons (a message's actions) are the exception and stay bare.
- Each message has small actions beside its time (copy, reply in side chat, more) that
  fade in on hover or focus, with tooltips.
- Threads outside any section sit in a "Threads" section below the attention card.
- The app switcher capsule shows labels only, three apps by default, on the pill surface.

## Preview

The web UI is deployed for review at `https://preview.calternal.cloud/ai/` (served from
`~cua/preview/ai` on `calternal-dev-02-std`, NetBird only).

## Order of work

No UI lane starts until the foundation is solid and break-tested. The orchestrator is the
first app built, so that development can move onto it.

## Open

- Narrow screens, to be drawn out: the sidebar and right card slide over; the current view
  slides into them; the mockup's bottom tab morph is kept. The middle card must visibly
  float above the sidebar, which it does.
- Real-device measurement of the frame cost of backdrop blur. The adaptive technique is decided and built:
  `docs/ui-design-system.md` (no pixel is read; brightness comes from the background's own colour mesh).

Implementation and status: `docs/ui-shell-status.md`.

## Sidebar pull (2026-11-18)

A horizontal swipe anywhere over the middle card brings a closed sidebar into view, following the finger one to one: the sidebar's visible width is the distance the finger has travelled, clamped to the sidebar's width. The opposite swipe closes it. One recogniser (`apps/web/src/shell/Shell.svelte`, rules and helpers in `shell/pull.ts`, tested in `pull.test.ts` and `e2e/sidebar-pull.mjs`) serves the phone, a touch screen of any width (not a trackpad); the phone's edge swipe is the same gesture started at the edge.

**What a swipe does.** Finger right: closes the right sidebar if it is open, else opens the left one. Finger left: closes the left sidebar if it is open, else opens the right one (when the app has one). Otherwise nothing (swiping toward a sidebar that is already open does nothing).

**What moves.** Phone: the card slides aside over the sidebar underneath, to the right for the list and to the left for the side panel (the mirror, one motion; it first read "the side panel slides over the card" and was changed on 2026-11-18 after the owner saw the overlay on a real iPhone); both follow the finger. There is no scrim: the strip of card that stays visible closes what is open. Wide screens: the sidebar's content slides in over the card's edge (`translate` only) and the card moves by the same amount (`transform` only); the columns do not change during the pull, so nothing is laid out per frame. At release the sidebar's state changes (the layout switches once) and the pieces ease from where the finger left them to rest.

**Release.** Speed over the last 80 ms decides when it is a flick (0.4 px/ms or more): that way. Otherwise more than half showing opens it. Motion: the sheet tokens, `--dur-sheet` on `--ease-sheet` (Web Animations API on a wide screen, CSS transitions on the phone), no spring. On a phone the result goes through `shell.over`, so it is a history entry (Back, Forward and the platform's back gesture do what the swipe did); on a wide screen through `shell.setOpen`, which remembers it.

**Interruptible.** A touch or a swipe while the card or sidebar is still settling takes hold of it where it is (its current offset gives how much of the sidebar shows) and carries on from there.

**Rules: what a swipe does not take.**
- A swipe that starts on something that scrolls sideways and can still scroll that way (a code block, a table, a wide text field, the CodeMirror scroller) belongs to that element for the whole swipe; at its end the next swipe is the pull's.
- A swipe that starts on a thread row's own swipe actions (`.ra`), in a dialog or sheet, or on anything marked `data-no-pull`.
- A touch while text is selected (the selection handles drag the selection).
- Movement that is not clearly sideways: it must be 10 px across and 1.5 times more across than down before it is a pull; a vertical scroll is never one.
- A mouse or a trackpad. Only touch pulls (one finger); a mouse drag keeps selecting text, and a trackpad's horizontal swipe never moves a sidebar (owner decision, 2026-11-18, reversing the first version: on a desktop the sidebars have buttons, shortcuts and resize handles). Two fingers on a touch screen belong to the canvas when it is on.



*(2026-11-18, later)* The trackpad pull was removed: `WheelSwipe` and the `wheel` listener are gone; `e2e/sidebar-pull.mjs` checks that a horizontal wheel, in pixels or lines, small events or large, in either direction, moves nothing.

## Safe areas (2026-11-18)

Ported from the existing app (`lib/home-screen.ts`, `page-chrome.ts`, `index.html` on origin/dev, read with git) and fitted to the shell: apps and plugins get safe-area padding from the shell and never read `env()` themselves.

**Page.** `viewport-fit=cover` (the page is drawn under the status bar, the notch and the home indicator), `interactive-widget=resizes-content`, `color-scheme`, and Apple's home-screen tags (`apple-mobile-web-app-capable`, `status-bar-style=black-translucent`, title, touch icon) plus a web manifest (`display: standalone`, icons in `public/icons`, made by `scripts/icons.mjs`; the manifest uses relative addresses so it works under `/` and `/ai/`).

**Tokens.** `--safe-top`, `--safe-bottom`, `--safe-left`, `--safe-right` (`env(safe-area-inset-*)`, 0 where there is none).

**Who applies what.**
- The shell (`Shell.svelte`): the grid is padded left and right by the side insets, so the sidebars, the card and the side panel sit between the notches; the backdrop (`Backdrop.svelte`) and its washes are not padded, so the wallpaper and the veils are painted under everything (the sidebar's wash grows by the inset). The shell sits above the keyboard (`--kb`, below) and, on an iPhone home-screen app, is as tall as `--vh`.
- Floating bars at the top and bottom of a card or sidebar (a header, a composer, the phone's bottom bar) use `--safe-top` and `--safe-bottom` themselves, because they float over content that scrolls under them and fades; the shell cannot pad that content from outside. An app places such a bar at `top: var(--safe-top)` and its scroller's fade starts below it (`--fade-t`); nothing else needs to know.

**The browser's own bars.** Safari tints its toolbar and the status bar from `theme-color`. `appearance/wallpaper.ts` sets it to the colour at the top of the page (the card's top strip of the wallpaper copy under its veil) whenever the veil is solved, through `shell/chrome.ts` `setThemeColor`, which writes only when the value changes (some browsers repaint their chrome on every write and it flickers). So in a Safari tab the bars are the wallpaper's colour instead of black; installed as a home-screen app the status bar is translucent and the picture shows through.

**iPhone home-screen app (iOS 26 and 27).** The page is laid out one status bar shorter than the screen (`100dvh`, `innerHeight` and `position: fixed` boxes stop about 62 px above the bottom; WebKit bugs 301994 and 301108), leaving a flat strip. `chrome.ts` (`startChrome`) marks the root `data-home-screen` and sets `--vh` to the tallest of: a zero-width fixed probe's height (the layout viewport) and, when the page runs under the status bar or sideways, the screen's own size. The shell and the backdrop are as tall as `--vh`. Only for an installed iPhone or iPad web app, never a Safari tab, Android or a desktop browser (a test turns it on anywhere with `globalThis.__homeScreen = true`).

**Keyboard.** iOS keeps the layout viewport under the keyboard, so a field at the bottom is hidden. While a text field has focus on a coarse pointer, the part of the screen the keyboard covers (`innerHeight - visualViewport.height - offsetTop`, ignored under 80 px) goes to `--kb` and the shell and the phone's bottom bar lift by it. Android Chrome resizes the page itself, so the difference is zero and nothing happens.

Checked by `e2e/safe-areas.mjs` (in the gate) with fake insets in portrait (47 top, 34 bottom) and landscape (47 each side, 21 bottom), light and dark: the backdrop fills the screen, the shell pads by the side insets, the card stays between the notches, the header sits below the status bar, `theme-color` is the page's colour, a home-screen app is sized by the probe and the keyboard lifts the shell.

## The background (2026-11-18)

There is exactly one continuous background layer: the wallpaper (`.backdrop`, the 12 px copy) under ONE veil (`.wash`, full window) that `appearance/wallpaper.ts` solves once from the pixels under both sidebars together, so the ink reads on the worse of them. Both sidebars are drawn on it and the middle card is the only thing on top of it. Nothing else paints in the gap beside the card or behind its rounded corners: no veil per sidebar (there used to be one for each, ending at the card's edge, which left the corner squares bare and a step beside the card; the owner reported it three times), no card backdrop, no clip rectangle, and no shadow (a wide card casts none; on a phone it slides over the list and does).

`e2e/seam.mjs` (in the gate) measures it on real pixels: text, icons and the sidebars' content hidden, the rings 3, 12 and 21 px outside the card's rounded corners and gap columns (5 x 5 pixel means, so the grain does not count): the inner ring must lie on the straight line the other two give, each against a control (the same measure well inside a sidebar); a step more than 3 levels of 255 above the control is a seam. It runs both schemes at 1x and 2x on the default wallpaper and a second one in the gate; `SEAM_FULL=1` runs all eight wallpapers at 1x and 2x. On the build before the fix (705fb6c) it fails (up to 50 levels above the control, at the bottom-left corner in light and the top-left in dark); on the fixed build the excess is 0.

## Onboarding (2026-11-18)

A fresh installation's first owner, and a person who followed an invitation, go through a stepped flow instead of the old single card (`shell/onboarding/`). The style is infraternal's setup wizard (read from `DECISIONS.md` 95 to 103 and `docs/design/TASTE.md` in its repository): one centred card over the app's backdrop with a numbered stepper above it; a numbered step turns into the shared tick when it is done (a disc that swells in on the spring with the tick drawn on, 200 ms; a key-driven finish shows it at once, a pointer-driven one draws it); a step that finishes confirms and moves on by itself after a short beat; waits show a count-up in seconds after 400 ms (never a spinner, never invented progress); copying a command turns the button's own icon into the tick (200 ms in, 0.9 s hold, 150 ms out). Fonts and specifics are ours.

**Steps.** The owner has five: **You** (the setup code from the setup link, a name, the passkey: this makes the account and signs it in), **Look** (the preset gallery; the choice shows behind the card at once and is the answer), **Machine**, **Provider**, **Done** (into the first thread). An invited person has three: You, Look, Done. **Machine**: if the instance already has a machine (set up when it was deployed) it shows as connected with its name and platform and there is nothing to do; otherwise a button makes a one-use code and prints the command (`calternal machine enroll --server <origin> --code <code>`) with a copy button, the screen looks again every two seconds, a count-up says how long it has waited and the step confirms when the machine is there. **Provider**: each provider on each machine shows signed in, not signed in (with the provider's own sign-in command to copy and run there), or not installed; Check again asks the machine (`agent.check`) and the screen follows the answer; one signed in ends the step. Built on what the server answers today (`/api/v1/machines`, `/machines/codes`, `agent.readiness`, `agent.check`); the relayed sign-in (a link to open here instead of a command to run there) and the machine enrolled at setup are #48's, and replace the two notes marked in `onboarding/api.ts` when it sends shapes.

**Skipping and resuming.** Who you are cannot be skipped (it makes the account). Look, Machine and Provider can: Skip says what it costs ("Without a machine no thread can run. You can add one later in Settings, Machines."). Where the person has got to is kept on this device (`cal.onboarding`), and at start-up a signed-in person with that record is returned to the step they were on (`boot.ts`), so a reload or a closed tab resumes. Back goes to the previous step. The heading of each step takes focus when it appears, a live line says "Step 3 of 5: Your machine", every control is a button or a field (Tab and Enter do all of it), and with reduced motion the step enters with a fade and the tick has no swell.

**Fixtures.** `?server=0&onboarding=1` runs the owner's five steps and `?server=0&onboarding=invited` the three, with a pretended passkey and a simulated machine (it connects on the third look after the command is made; the provider signs in on the second check); its place is kept in the tab's session storage so a pretended run never leaves anything for the real one. `e2e/onboarding.mjs` (in the gate) walks both, with a resume after a reload, the skip notes, the count-up, the copy tick, keyboard-only entry, reduced motion and axe on every step; `e2e/onboarding-shots.mjs` takes a picture of each step.

### Update (2026-11-18): one path, the #48 wire, and "I write code"

Everyone takes one path of five steps (You, Look, Machine, Provider, Done); the invited person's earlier three-step shortcut is gone. The **I write code** switch on the first step adds three steps before Done: **Repositories** (add a repository by URL), **The command line** (install `calternal`) and **Direct mode** (let agents work on your own machine). The Machine step reads `machines.onboarding` (`ready` is shown as connected) and offers `machines.install`'s single-use script (15 minutes) to add another. The Provider step starts `provider.sign_in.start`, shows the link and display code from `requests.list` (`detail.kind = "provider_sign_in"`), sends the typed code with `provider.sign_in.answer`, and follows `provider.sign_in.status`; Check again stays as the fallback. Fixtures: `?server=0&onboarding=1`, `&attached=1` for a machine already attached.

## The app switcher (2026-11-18)

One component, `shell/AppSwitcher.svelte`, in two places: a capsule under the middle card on a wide screen, the bottom bar on a phone (it rides with the card while a sidebar is pulled). Text only, three names wide, the rest reached by scrolling sideways (snapping, with edge fades), no More list; the chosen app is kept in view; arrow keys, Home and End move and open. The person chooses the order (Settings, Appearance, Move up and Move down; stored as `appearance.prefs.appOrder`, applied by `shell/appOrder.ts`). On a phone the spaces and Settings are behind the account picture in the sidebar footer; a thread's side panel opens from a button on the card's right edge. Checked by `e2e/app-switcher.mjs`.

## Skills and Automations (2026-11-18)

Two pages in Settings, in the current space's section, built on fixtures (`apps/settings/SkillsPanel.svelte`, `AutomationsPanel.svelte`, state in `skills/store.svelte.ts`). On the real server they say the server does not offer them yet.

- **Skills.** A list (name, one line, which agents use it, the file path beside it) and a guided New skill: Name (the name is made from a few words and checked against the Agent Skills rules), Description (what it does and when to use it, at most 1024 characters, with a hint when no "when" is given), Instructions (Markdown, with a starter), Who uses it (Claude, Codex; none means every agent), Review (the exact `SKILL.md`). The last button writes `AI/Skills/<name>/SKILL.md` and opens it in the Notes editor. The rules and the file text are in `skills/skill.ts`, citing <https://agentskills.io/specification> (name 1 to 64 characters, lowercase letters, digits and single hyphens, equal to the folder name; the agents are kept under `metadata`).
- **Automations.** A list (schedule in words, next run, last result, Run now, Pause, Open file) and a guided New automation: Name, When (typed in words and read with the palette's recogniser, shown back as "every weekday at 9:00"; "every 2 weeks" is refused with the reason, because a systemd timer cannot say it), Prompt, Review. The file is `AI/Automations/<name>.md`: frontmatter `id`, `name`, `schedule` (a systemd `OnCalendar` expression such as `Mon..Fri *-*-* 09:00:00`), `timezone`, `enabled`, and the prompt as the body. Each run is a thread. The schedule rules are pure and tested (`skills/schedule.ts`).
- **The guided form** is `skills/Wizard.svelte`: numbered steps that become ticks, one step at a time with its heading focused, Enter moves on when the step is valid, reduced motion fades only. Onboarding has its own stepper; the two should share one (a simplification opportunity).
- **Server operations needed** (listed in `NEEDS`, in `skills/store.svelte.ts`): `skills.list`, `skills.create`/`update`/`delete`, `agents.list`, `automations.list`, `automations.create`/`update`/`delete`/`pause`, `automations.run_now` (starts a thread) and a notes create that takes a path, so the file lands at `AI/Skills/<name>/SKILL.md`. Last result and next run come from the server's timer, not the page.
- Checked by `e2e/skills-automations.mjs`.

## Phone checks (2026-11-18)

No phone layout is pushed before it has been looked at on an iPhone Simulator, in Safari, at the preview address: created, used and deleted on std (Simulator work only); the frames are attached to issue #15. The pass of 2026-11-18 went through the shell (list, thread, side panel), Settings, onboarding, the palette, Notes and the sheets.

*(2026-11-18, later)* Skills page, server mode: the list is read with `skills.list {space_id}` (name and one line each; the file is `AI/Skills/<name>/SKILL.md`). Writing a skill from here and the agents that use each still need the server operations in `NEEDS`.

## The app switcher moves into the sidebar footer, and spaces go behind the account picture (plan, 2026-11-18)

Owner decision (replaces the space-strip plan, #39): spaces are behind the account picture on desktop **and** phone (the account menu holds the searchable space list: a dropdown with a search field on desktop, a sheet on a phone by the menus rule); no space dots in the footer. The app switcher becomes an **icon-only capsule at the bottom centre of the left sidebar**, three icons at a time, scrolling sideways through the rest; a click or tap switches app. Footer order: account picture, app capsule, search. The capsule under the middle card and the phone's bottom bar go away. Nothing below is built yet.

- **One placement.** `AppSwitcher` loses `placement="card"|"bar"` and the `.appbar` slots, `--switcher-clear` and the phone bar's `--pull-x` ride-along go with it. The canvas windows run to the bottom edge (the switcher no longer overlaps them). `SpaceDots` is deleted; the account menu is a `Menu` with `search`: the current space checked, then Settings.
- **Names for keyboard and screen readers.** The capsule is `<nav aria-label="Apps">` of real links (`<a href="/threads" aria-label="Threads" aria-current="page">`): Enter opens, "Threads, link, current page" is announced, the route is right-clickable. Every icon has a tooltip with the app's name (and its shortcut) on hover and keyboard focus, and on a long press on touch. One tab stop (the current app); Left and Right move focus and bring that icon into view (roving, as the tab list does now), Home and End jump, Enter activates.
- **The current app is always in view.** Centred on mount (no animation) and whenever the app changes by any route (a click, the palette, a link, Back); with three apps or fewer nothing scrolls and the icons are centred. Native scroll with `scroll-snap-type: x mandatory`, each icon `scroll-snap-align: center`, one slot of padding at each end.
- **The more-apps hint, without half-icons.** The strip shows exactly three whole icons. When more apps exist on a side, the outer icon on that side is drawn smaller (`scale` 0.75) and fainter (opacity 0.55), the page-indicator pattern; with three or fewer all are full size. No clipped icon, no gradient or mask.
- **Motion tokens.** The marker that shows the current app moves between icons as `translate` over `--dur-base` on `--ease-out`; scale and opacity of the outer icons the same; press `--press-scale` at once; centring after a key or an outside change is an animation of `scrollLeft` over `--dur-base` on `--ease-out` (not the browser's smooth scroll, whose time is not a token), cancelled by the next touch, wheel or key (interruptible); reduced motion: instant (the durations are 1 ms).
- **Sizes.** An icon slot is 40 px (44 on touch): three slots (120) with the 44 px account picture and search fit the 240 px sidebar and the 334 px phone sidebar. The person's order (Settings, Appearance, Move up and down) still decides the sequence.
- **Collapsed sidebar (proposal, to be confirmed before building).** A collapsed left sidebar has only the "Show the sidebar" button at the card's top-left corner. Proposal: that button becomes a two-part pill `[sidebar icon | current app icon]`: the sidebar icon opens the sidebar as now; the app icon opens a dropdown (`Menu`, morphing from the pill) listing every app with its name and the current one checked, so nothing needs a second rail and the names are visible. The shortcuts that switch apps stay. On a phone the sidebar is the list the card slides over: with it closed, switching apps is the card's top-left button (open the list) then an icon, or the palette ("Open Notes"). The alternative (a narrow icon rail that stays when collapsed) costs a permanent 56 px column and a third layout state, so it is not proposed.
- **Plugin-contributed apps.** An enabled plugin that declares an app (id, name, an icon, a route prefix; the shape would travel in the plugin manifest next to its commands) gets an icon in the capsule after the built-ins, in the person's order (an id not yet in the order is appended). The icon is host-drawn: a name from the host icon set, or one bounded SVG path string validated by the host (never an image, HTML or script); a plugin with no icon gets its first letter in a circle. The accessible name is required (a manifest without one is refused). Turning the plugin off removes the icon and, if it was the current app, goes to Threads. The capsule never grows: 50 apps still show three, scrolling.
- **Checks.** At 1, 3, 4 and 20 apps, desktop and phone: the number of whole icons visible, the smaller and fainter outer icon only when more exist, the current app centred after load and after a palette switch, Left/Right/Home/End/Enter, the accessible names and `aria-current`, the tooltip, scroll and snap, and the account menu's space list with search at 1, 3 and 50 spaces (fixtures-only `?spaces=N`, `?apps=N`). `app-switcher.mjs`, the shell-audit and phone-nav bar checks and the `.switcher.bar` / `.appbar` selectors in sidebar-pull are rewritten in the same merge, not kept alongside. Frames on issue #15, the phone in the iPhone Simulator.

## Fonts: Cascadia Code, four roles, and what the server must offer (plan, 2026-11-18)

Owner decisions: Google Sans Flex is the default for interface and chat; Cascadia Code replaces Maple Mono as the monospace default (Maple Mono may stay selectable); four roles, each with its own family (**interface, chat, code, terminal**; the wordmark keeps its own setting); sources are a catalogue (Google, Fontsource, Fontshare), an uploaded file and "System"; the server downloads a chosen font once and serves it itself, as wallpapers do. Ported from the owner's bb plugin (`calternal-ui`: `shared/fonts.ts`, `shared/contract.ts`, `server/fonts*.ts`, `ui/settings/fonts.tsx`, `font-upload.tsx`): the behaviour, not its settings design. Settings follows the Xcode-style model of `docs/settings.md`.

**What we ship now (checked).** Monospace is Maple Mono in `appearance/fonts.ts`, `packages/ui/tokens/tokens.json` (`font-mono`) and `fonts.css`, `scripts/fonts.mjs`, `packages/ui/src/lib/fonts.ts`, the generated Swift tokens, the docs site CSS, presets in `ui-backgrounds-and-themes.md`, four tests, and `ui-design-system.md`, `ui-shell.md`, `ui-shell-status.md`. `public/boot.js` also **preloads `maple-mono-latin-400-normal.woff2` (74 KB) on every first load**.

**The swap to Cascadia Code (web, mine).** Cascadia Code is OFL-1.1 (Fontsource `@fontsource-variable/cascadia-code` 5.3.0): the latin subset is **one variable file of 48.6 KB** covering weights 200 to 700 (static latin 400 and 500 would be 29.5 and 29.9 KB; the variable file is smaller than the pair and gives bold). Maple's two files are 74.1 and 76.7 KB. Plan: change `font-mono` once in `tokens.json` and regenerate (never hand-edit the generated CSS, Swift or docs CSS); `scripts/fonts.mjs` copies the Cascadia file; `fonts.css` declares it with `font-display: swap`; Maple's `@font-face` stays declared but **not preloaded**, its two files kept in `public/fonts` so it remains selectable (151 KB on disk, nothing on the wire unless chosen). **First paint does not wait for the monospace font:** the preload in `boot.js` is removed (74 KB fewer on the first request), code text draws in the `ui-monospace, Menlo, Consolas` fallback and swaps when the file arrives (monospace in code blocks and ids only). Net: first load **-74 KB**; when code is first shown **48.6 KB** instead of 74 KB; on disk +48.6 KB. The docs, presets and the four tests are updated in the same merge.

**Four roles in the web UI (mine).** `FontRole` becomes `ui | chat | code | terminal` (+ `brand`); defaults Google Sans Flex, Google Sans Flex, Cascadia Code, Cascadia Code. Tokens: `--font-ui`, `--font-chat`, `--font-mono` (code), `--font-terminal`, each a stack (the family, then the fallback), through **one loader** (`packages/ui/src/lib/fonts.ts`) that registers whichever families are chosen. Settings, Appearance, Fonts: four rows (Interface, Chat, Code, Terminal) each showing its family and drilling into a picker: sources on the left (Google, Fontsource, Fontshare, Upload, System), families on the right with search, each previewed in itself, an install state (Downloading, Ready, Failed with the plain reason) and Use; a Reset per role; Upload (file picker, family name, the limits shown); a stored-fonts list with sizes and Remove in Storage. The chosen families persist with the other appearance prefs and `boot.js` applies the stored stack before paint (only the interface font is preloaded; an unknown family falls back to the system stack until its file arrives, never blocking paint).

**What needs a server endpoint (for a backend lane).** Fonts are cached instance-wide (like wallpapers); uploads are per person. Proposed operations:
- `fonts.catalogue {source?, query?, mono?, limit?, cursor?}` -> `{fonts: [{family, category, source, ref?, mono}], next_cursor?, partial}`: the merged Google, Fontsource and Fontshare lists, fetched and cached by the server (a day's TTL, size-bounded), paged and searchable (the catalogues have thousands of families); one source failing sets `partial`, never an error.
- `fonts.install {family, source}` -> `{status: "installing"|"ready"|"failed", bytes?, reason?}`: downloads once (woff2, weights 400 to 700, italics when present), idempotent, polled with `fonts.status {family}`. Bounds: an allow-list of source hosts (no other redirect), a per-family size cap (proposed 8 MB), a total cache cap (proposed 200 MB, operator-set; least recently used unused families are evicted), a per-person install rate limit, woff2 only with a magic-number check, a timeout; an operator switch to disable outside downloads (air-gapped: the catalogue then lists installed, uploaded and System only).
- `GET /api/v1/fonts/<family>/<weight>[-italic].woff2` (immutable, content-hashed, `font-src 'self'`) and `fonts.faces {families}` -> `{css}`: `@font-face` rules pointing at those URLs (not inlined data URLs: cacheable and small).
- `fonts.upload` (multipart, not a data URL in JSON): per file at most 8 MB, at most 12 files per family, woff2, woff, ttf or otf checked by magic number (optionally converted to woff2 server-side), a family name of letters, digits, spaces and `. _ -`, a per-person quota (proposed 50 MB), private to the person.
- `fonts.list` -> installed and uploaded families with source, bytes and "used by"; `fonts.remove {family}`: a family in use falls back to the role's default.
- Prefs: the four roles (and brand) in the person's appearance prefs, wherever the server keeps them (today the web keeps them in the browser).
- **Question:** instance-wide installs (proposed: one copy serves everyone, the person who first picks it triggers the download) or per person?
