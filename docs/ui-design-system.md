# Design system (`packages/ui`)

Tokens are data; surfaces are components. Visual language is ported from the mockup
(`origin/review/switcher-a-pills`: `packages/ui/src/styles/tokens.css`, `glass.css`, `Pill`, `Toolbar`,
`Icon`); `docs/ui-shell.md` wins where they differ. Deviations are listed in `docs/ui-shell-status.md`.

## Native

The Apple clients' design system is `apps/apple/CalternalUI` (SwiftUI, macOS 26 and iOS 27). Its tokens are generated from the same `tokens/tokens.json`;
its README lists what matches this system exactly, what differs because the platform differs, and what is open.

## Tokens

- **One source:** `packages/ui/tokens/tokens.json`. `scripts/build-tokens.mjs` emits
  `src/styles/tokens.generated.css` (git-ignored; every build, dev and test script runs it first). Native
  clients read the JSON directly. A token is added there and nowhere else.
- **Groups:** `base` (type scale, spacing `s-1`..`s-10`, radii, motion, sizes), `light`, `dark`, `touch`
  (phones and tablets), `desktop` (fine pointer with hover, 760 px and wider), `backdrop` (the 3 by 3
  colour mesh), `springs`.
- **Desktop and touch are tuned separately, not scaled:** glyph 18 px against 22 px, row 30 against 48, hit
  target 28 against 44, blur scale 1 against 0.7, their own text sizes.
- **Fonts, only these:** Google Sans Flex for all UI text (the `Dark` family adds the GRAD axis for light-on-
  dark ink with identical advance widths; the Rounded variant is used for the app name and avatar initials),
  Maple Mono for code. No system font is in any stack. Files are the subset latin and latin-ext woff2s, the
  page head preloads only the active scheme's file, and nothing paints until it has loaded (`font-display:
  block`, `lib/fonts.ts`, `apps/web/src/main.ts`).
- **Type scale:** `text-title`, `text-heading`, `text-body`, `text-ui`, `text-label`, `text-code`,
  `text-section`; weights `ui-weight` 440, `body-weight` 460, `strong-weight` 620. Components use these and
  no other size or weight. `apps/web/e2e/audit.mjs` lists every size and weight actually drawn.
- **Spacing:** components use `--s-*` only (plus the tokens derived from them: `--card-gap`, `--pad-region`).
- **Radii:** concentric where it costs nothing (a menu item is the menu radius less its padding); the selected-row
  tint is the native 8 whatever it sits in (decided, see below).
- **One inset, three durations, two curves** (collapsed after `docs/native-ui-study.md`, which found the mockup
  had five names for the space between surfaces and twelve duration and spring tokens): `--inset` (8) is the
  only gap between surfaces and from the viewport's sides; `--dur-fast` 120, `--dur-base` 220, `--dur-slow` 300
  and the curves `--ease-out` and `--spring` (one spring, damping 0.9, response 0.42 s, run over `--dur-slow`).
  Heights: `--pill-h` (the one control tier: 38 on a desktop, 44 on touch, which is also the touch hit target),
  `--row-h`, `--bar-h` (header band) and the phone's bottom capsule `--tier-cap`. Radii: one concentric scale, see
  "Corner radii" below.
- **Reduced motion and transparency:** under `prefers-reduced-motion` every duration is 1 ms and every spring
  is linear; under `prefers-reduced-transparency` glass becomes near-opaque.

## From the owner's plugin (`calternal-ui`)

Studied read-only at `/home/kayg/Developer/bb-plugins/calternal-ui` (`frame/css.ts`, `preblur.ts`, `wallpaper.ts`, `color.ts`, `accent.ts`,
`pill-contrast.ts`, `pills*.ts`, `edge-fade.ts`, `composer.ts`, `shared/defaults.ts`, `shared/fonts.ts`, `server/fonts*.ts`, `server/wallpapers.ts`,
`ui/settings/*`, `README.md`). It is the look the owner lives in daily and the bar to beat. What was taken, and what was not:

| Plugin | Here |
| --- | --- |
| A wallpaper behind flat sidebars; the middle is the only card | Taken (`Backdrop`, `Card variant="pane"`, `RightSidebar`) |
| The wallpaper is blurred **once**, into a canvas 1/k of the viewport (k up to 8) with saturation baked in, exported as a JPEG blob and shown as a plain background; no live filter | Taken: `appearance/wallpaper.ts` (`preblur.ts` ported; two copies, sidebars' and card's) |
| Defaults: sidebar blur 12, tint 35, no dim; card blur 75, tint 85, dim 30; saturation 120 % (sidebar) and 140 % (card) | Taken as the defaults, editable in settings |
| Card glass is two layers of its own (blurred wallpaper copy, then tint and dim), never a backdrop filter on the card, so the composer inside still blurs through | Taken (`Card variant="pane"` `::before` and `::after`) |
| Card dim: a white wash in light (`min(0.9, dim × 1.5 + 0.2)`), black in dark; a 50 % white wash on the sidebars in light | Dim taken as tokens `card-dim` and `card-dim-a`; the sidebar wash is derived (below) instead of fixed |
| The average colour and the most vivid hue are read once from a 48 by 48 copy (hues binned by 15 degrees, scored by count times chroma); the accent follows the hue (lightness fixed per scheme, chroma 0.09 to 0.17) | Taken (`colours()`; settings: Accent "From wallpaper") |
| Pill glass: `blur(16px) saturate(180%) brightness(1.12)` (dark 0.72), clear fill, an inner shade, a 1 px edge and a soft shadow; drawn on a `::before` layer, never a backdrop filter on the button | Taken as tokens `glass-*` (`Glass.svelte`) |
| `pill-contrast.ts`: a pill gets a faint fill (up to 0.35) only where its backdrop fights its glyphs, estimated from the wallpaper's brightness grid and from "hazards" scrolling under it (images, video, solid blocks) | The wallpaper half is taken (`material()` from the brightness grid, up to 0.35). The hazards half is not: pills here float over chat text, not over arbitrary media, so far |
| Contrast: ink stays legible whatever the wallpaper (the plugin raised light washes by hand after measuring 2.3 to 2.7:1) | Derived instead of hand-tuned: `washFor` solves for the wash that brings secondary ink to 4.5:1 over the darkest (light scheme) or brightest (dark) cell under the sidebars. Checked by axe on all 8 built-ins in both schemes (`e2e/wallpapers.mjs`) |
| Edge fade: rows fade progressively as they pass under floating bars (CSS scroll-driven animations, per row) | Not yet: a static mask fades the message list's top and bottom |
| Fonts per role (interface, chat, code, terminal) from Google, Fontsource, Fontshare, own files, System; stored on the server so pages never load from those sites; SF/system stack for Apple | Taken for three roles (`appearance/fonts.ts`, `SettingsSheet.svelte`); the catalogues are fixture lists until the server exists |
| Settings sections: Wallpaper (upload, link, library, Unsplash), Glass, Colour and size, Fonts, Wordmark, Storage | Wallpaper (upload, library, Unsplash), Glass, Colour, Fonts. Not yet: paste a link, Wordmark, Storage list with sizes |
| The composer is one tidy row: model, context, permission, environment | Taken as three chips (provider and model, permission, where it runs) with labels that shorten on the composer's own width |
| The thread title is a pill; wordmark row with the sidebar button at the other end; swipe gestures on phones; root size 17.6 px; card corner 62 px with squircle corners; 12 px gap | Not taken: sizes follow the native study (13 px text, pane radius 20, inset 8); title pill, wordmark and gestures are still to build |

## Native values (decided 2026-11-17)

Where `docs/native-ui-study.md` section 7.2 measured something different from the mockup, the system follows native. Each
is one token in `tokens.json`, so a change is a one-line edit followed by `npm run tokens`.

| Token | Value | Replaces (mockup) | Used by |
| --- | --- | --- | --- |
| `row-h` desktop | 32 | 30 | every row: sidebar, menus, steps, lists |
| `r-select` | 8 | 12 | the selected-row tint and small rounded rectangles |
| `text-ui` desktop | 13 | 14 | all UI text on a desktop |
| `bar-h` | 52 desktop, 60 touch | 54 | header bands of the sidebar, middle and right cards |
| `pill-h` desktop | 38 | 30 | toolbar buttons, pills, buttons |
| `tier-cap` / `cap-offset` | 49 / 27 | 52 / 12 | the phone's bottom capsule and its gap above the bottom |
| `text-title` | 34 touch, 22 desktop | 30 | Notes title |
| `row-h-settings` | 44 desktop, 52 touch | row height | settings rows (`Row tall`) |
| `inset` | 8 | 5 | gap between cards and to the viewport's sides |

Kept from the mockup deliberately: the narrow centred Notes column. Reading text stays at `text-body` (15 on a desktop).

## Wallpaper

`appearance/wallpaper.ts`. Choosing a wallpaper (a built-in, an upload, an Unsplash photo saved by the server) costs one pass: two canvas renders
(the sidebars' copy at the chosen blur, the card's much blurrier copy), one 24 by 14 brightness read and one 48 by 48 colour read. The results are
CSS variables (`--wall-side`, `--wall-card`, `--wash`) and a brightness grid handed to `setBackdropField`; nothing runs per frame and no
`backdrop-filter` covers a large area. A resize re-renders after 400 ms. The wash over the sidebars is `washFor(scheme, range, base)`: the wash
that brings `ink-3` to 4.5:1 over the worst cell under the sidebars, never below the chosen tint (35 % by default) and never above 0.9.

The built-in set (`appearance/library.ts`) is eight palettes painted on a canvas (Dune, Lagoon, Aurora, Ember, Orchid, Moss, Slate, Paper), not
photographs: no image files ship, and every one has a light and a dark region. "Automatic" (the default) uses Dune in the light scheme and Aurora in
the dark. Uploads are images the person chooses (IndexedDB here, the server later). Unsplash search goes through `WallpaperLibrary.search`; the real
one needs the server's access key and returns the photo's credit, which the settings sheet shows as the licence asks.

## Fonts

Defaults, exactly as the existing app: Google Sans Flex for all UI text, in its light family (weight and optical size) or, in the dark scheme, its
Dark family (the same plus GRAD 30: more ink, identical advance widths); Maple Mono (400, 500) for code. Weights 440 (UI), 460 (reading text) and 620
(emphasis); optical size follows the font size (`font-optical-sizing: auto`). The Rounded family is used only where the existing app uses it, with
`font-variation-settings: "ROND" 100`: the app name in the sidebar header. No system font is in any default stack, nothing paints until the
active family is loaded, and `registerFonts` (`packages/ui`) adds the faces under the app's mount path.

Why the text was reported as not reading as Google Sans Flex: `e2e/font-compare.mjs` renders the same sentences at 13, 17 and 19 px and weights 440,
460 and 620 twice, once with the existing app's own CSS (its `@font-face` rules and body settings, taken from `origin/review/switcher-a-pills`) and once
through this app. The glyphs and advance widths are identical; the faces are the same files with the same axes. Three real differences were found
and fixed: the Rounded family was drawn without `"ROND" 100`, so the app name was not rounded at all; Rounded was also applied to the avatar
initial, where the existing app does not use it; and the text sat at 14 to 15 px with the heavier 620 weight in more places than the existing app
(the owner's 13 px and the existing weights are now the only ones). A browser that fails to load the font (a wrong base path, a blocked file) would
show a system face; `e2e/preview-check.mjs` checks on the deployed copy that Google Sans Flex and Maple Mono are loaded.

Font choice is a setting per role (interface, chat, code): sources Google Fonts, Fontsource, Fontshare, own files and System. The server will
download a chosen family once and serve it from its own storage; `FontLibrary.install` is that call. Own files are registered in the browser
(`FontFace`) with weights and italics read from their file names. In this fixture only the shipped families and own files can be drawn; the others are
listed and say they need the server.

## Adaptive material

`lib/material.ts`. A surface uses `use:adaptive` (the components do it themselves). The background is a
colour mesh, the same data that paints it; the brightness behind a surface is nine interpolations of that
mesh inside the surface's rectangle (or, with a wallpaper, of the brightness grid read from its blurred copy). `material(scheme, behind, role)` returns the tint alpha and blur:
unchanged over a calm backdrop, denser (up to +0.35 alpha, the plugin's limit) and slightly blurrier where the backdrop fights
the ink. Values are written once per layout change as `--mat-a` and `--mat-blur` on the surface (batched in
one animation frame, reads before writes). Nothing runs on scroll and no pixel is ever read.

Pills and sheets blur 16 px (the owner's plugin and iOS). The middle card's blur is baked into its own copy of the wallpaper, so it can be
75 px with no per-frame cost. (A live blur above about 24 px is not drawn by the headless Chromium used for screenshots, which is one more reason
a large live blur is avoided.)

## Surfaces

| Component | What it is | Accessibility contract (enforced by its props type) |
| --- | --- | --- |
| `Pill` | A round glass (or tint) button, icon only or icon + label | `label` required |
| `IconButton` | Round button with one icon | `label` required (name and tooltip) |
| `Button` | Text pill: primary, secondary, quiet, destructive | `text` required |
| `Toolbar` | A pill holding buttons, arrow-key roving focus | `label` required |
| `Card` | Landmark region: `glass` (blurs and dims the background), `solid`, `attention` | `label` required |
| `Sheet` | Modal dialog (bottom or side), focus trap, Escape, focus return | `label` required |
| `Menu` | Popover list with radio items and arrows | `label` required |
| `Row` | List row, link or button | `title` required |
| `Badge`, `Icon`, `Spinner`, `Glass` | Status, line icons (decorative, `aria-hidden`), progress, the glass layer | Badge carries its status in text; icons never carry meaning alone |

Every interactive component is keyboard reachable and shows the one focus ring (`:focus-visible`, 2 px
accent). Hit targets are at least 44 px on touch (`--pill-h`); `audit.mjs` checks this on a phone.
Tests: `npm test --workspace @calternal/ui` (names, roles, keyboard, focus trap, axe) and
`npm test --workspace @calternal/web`.

## Interaction rules

- **Focus rings only for the keyboard.** `data-input="key" | "pointer"` on `<html>` (`lib/input.ts`); the ring is inset so it never draws outside its box.
- **No animation from the keyboard.** In `key` mode every transition and animation duration is 0. Pointer and touch animate.
- **Nothing out of bounds.** Menus and popovers place themselves with `lib/place.ts` (flip, shift, clamp to the window); rings are inset.
- **Surfaces are never seen before their material is ready** (`lib/ready.ts`): a popover or sheet appears only after the blur and tint are in, and leaves with a fade.
- **One state signal.** A thread's state is an icon in a row (spinner, bell, warning) or a word in the title pill, never both (`StateMark form`). Idle and done show nothing.
- **Thread rows are one line:** the provider mark at the left, the title, the compact state at the right. Pointing at a row swaps the state for exactly three actions (copy link, archive, three dots = the right-click menu, one definition). No second line, no automatic workspace labels; people make their own sections.
- **Swipe rows** (`apps/web/src/lib/swipe`, ported from the existing app's `rows/`, #400, same thresholds and springs): finger on a phone, two-finger sideways scroll on a desktop. Right shows Copy link; left shows Archive (a long swipe runs it) and More. Never from the screen's left or right 24 px (the shell's), a mostly vertical or slow drag is not a swipe, one row is open at a time, a drag in progress blocks it. One change from the existing app: only a scroll of the row's own container shuts an open row. Proof: `e2e/swipe-rows.mjs`.

## Primitives

| Primitive | Role |
| --- | --- |
| `Panel` | the material for cards, panes, pills (radius token), floating or docked |
| `Pill`, `IconButton` | round controls; both carry the tooltip and the press |
| `Popover`, `Menu` (search, anchor), `Sheet` | floating surfaces on `Panel` |
| `InlineEditor` | the one text-input surface (composer, side chat, rename); the Notes block editor's input replaces this file alone |
| `Toasts` | ported from the existing app; mount once; T1 rule: toasts only for something to act on (Undo, Retry) or a result awaited |

## Tooltips: which engine, and why

Compared sveltebits' `warm-tooltip` (uses the `motion` library; delay 400 ms, warm window 300 ms, travel 320 ms, pop scale .94 with blur, long-press 500 ms)
with the existing app's `tooltip-engine.ts` (dependency-free; delay 400, warm window **600**, travel 320 on the spring, shared surface plus a stage where
words slide with blur on swap, keeps itself centred while the trigger moves, **keyboard focus-visible only**, hides on scroll and Escape, shortcut chips).
**Kept the existing app's**: same behaviour as the reference, no dependency, a longer warm window that suits a toolbar, and it already follows our
keyboard rule. Ported to `lib/tooltip.ts` (`use:tip={{ label, kbd }}`); the label also names the control.

## Pill press

The existing app's press, as plain CSS on each control (`Pill`, `IconButton`, `Button`, `Row`, `Selector`): `scale` goes to `--press-scale` while pressed and returns over
`--dur-fast` on `--ease-out`, transform only. There is no rebound past 1 (owner, 2026-11-18: the overshoot was too much; the existing app has none). A keyboard activation never
moves it (key modality makes every transition 0 s) and reduced motion keeps only the scale.

## Popover morph

`Popover` and `Menu` take `morph` (the composer's three chips use it): with a pointer or touch driving and motion not reduced, the popover covers its button (`place(..., { cover: true })`:
left and bottom edges aligned, at least as wide) and grows out of the button's own shape, and shrinks back into it, as Apple's ⋯ menus do. Ported from the calternal-ui plugin
(`frame/clip-morph.ts`, fitted from iOS 27): `lib/clip-morph.ts`. Open: a damped spring (response 330 ms, damping 0.74, settled 480 ms), a capsule until 75 % grown then the menu's radius, a 2 to 4 %
overshoot as a scale about the button's corner, rows blurred 10 px to 0 and fading in between 75 % and 95 %. Close: rows out in 70 ms, the glass back into the button over 250 ms on
cubic-bezier(0.2, 0, 0.5, 1), the button returns at 160 ms with a small swell. These are the fitted values, not motion tokens. The clip is on the glass layer and on the words, never on an ancestor of the
glass (that would make a backdrop root and the blur would pop in at the end). Driven by a requestAnimationFrame loop, first frame painted synchronously. Interruptible: a reversal continues from the
progress reached. Keyboard use and reduced motion place the popover beside the button and fade it (nothing moves). Proof: `e2e/morph.mjs`. Headless Chromium does not draw backdrop-filter, so the blur
and the look of the morph are judged on a real GPU (the Mac).

## Focus ring

Keyboard only (`data-input="key"`), 2 px, inside the control, following the control's own radius (the rule sets no `border-radius`). A host that holds a text field draws the ring itself (`data-ring-host`,
`--ring-offset`). A modifier on its own and a Meta or Control chord with a character (a screenshot shortcut) are not the keyboard. Proof: `e2e/focus-ring.mjs`.

## Motion, per transition (measured on the dev VM, p50 / p90 / p98 ms to reach the end value)

| Transition | Duration and curve | p50 / p90 / p98 |
| --- | --- | --- |
| Press a control to 0.96 | `--dur-fast` 150, `--ease-out` | 17 / 50 / 83 |
| Phone: card slides aside, panel slides over | `--dur-sheet` 340, `--ease-sheet` (native fit) | 83 / 217 / 300 |
| Desktop: sidebar hide/show, sheet open | `--dur-sheet` 190, Mac curve | 67 / 150 / 167 |
| Menu / popover | ready, then fade `--dur-base` 220; leave `--dur-leave` 200 | opens within a frame |
| Toast | spring capsule (ported), arrive 260 | unchanged from the existing app |
| Tooltip | pop 160, travel 320 on `--spring` | unchanged from the existing app |

All are interruptible (a reversal half way is continuous, checked in `e2e/motion.mjs`), and none run from the keyboard.

## Craft references applied

Emil Kowalski's and Jakub Krehel's animation skills: ease-out for entrances, nothing slower than 300 ms, scale from 0.95+ never from 0, interruptible
transitions, no animation for keyboard actions. Hallmark's and the existing app's Craft studies: one inset ring, concentric radii, optical alignment of icons and carets.

## Backgrounds, themes, accent

Mechanism from `docs/ui-backgrounds-and-themes.md`: the wallpaper is blurred once (canvas, then a JPEG blob), brightness grid drives the wash and the
material, a 48×48 hue sample gives the accent (guarded to 4.5:1, overridable), twelve presets map palette to tokens, and recipes (OKLab gradients) render
light and dark. Choices (default wallpaper `haze`, default preset `calternal`, grain) are tokens in `tokens.json`; the mechanism is code.

## Material: the veil (replaces the brightness heuristic)

`appearance/veil.ts` implements section 5 of `docs/ui-craft-and-settings-study.md`. For each surface (left sidebar, right sidebar, middle card) it reads the pixels of the
pre-blurred copy under it and solves how much of the scheme's paper colour must cover them: primary ink 7:1, ink-2 5.5:1 and ink-3 4.5:1 on the worst 1 % (with 0.35
headroom for grain and JPEG), compositing in gamma-encoded sRGB as CSS does. Layers: **Tint** (paper with a whisper of the picture's hue, OKLCH chroma 0.25 of the mean, at
most 0.05) then **Dim** (plain paper); **Blur** is the pre-blurred copy itself, not a layer. A box off makes the others carry the floor; all three off is opaque paper;
reduced transparency behaves the same. Sidebars start at tint 0.35 and dim 0.25 and rise only as needed; the card's veil is the floor plus 0.02 split between the layers
that are on. Secondary and tertiary ink move toward the primary per surface until they reach their targets (`--l-ink2`, `--r-ink3`, `--c-ink2`, ...). A sidebar may flip to the
other scheme's ink when that needs clearly (0.12) less veil (`data-ink`, the generated `[data-ink]` token blocks); the card never flips.
`washFor` (still used by pills) now mixes in encoded sRGB; before, it mixed in linear light and delivered 3.0 to 3.8:1 where it promised 4.5:1.
Checked on rendered pixels: `e2e/veil-pixels.mjs` (all eight photographs, both schemes).

The photographs suit the dark scheme, so the light scheme uses a lighter rendition of each, made at build time (`scripts/wallpapers.mjs`).

## Card against sidebars: the plugin's values and ours (2026-11-18)

The owner's report: the middle card and the sidebars look too alike, which hurts contrast, and Clear to tinted turned the sidebar light in dark mode. The reference is the `calternal-ui` bb plugin (`~/Developer/bb-plugins/calternal-ui`, `shared/defaults.ts`, `frame/css.ts`, `frame/preblur.ts`).

**The plugin's defaults** (settings Chat and Sidebar surfaces):

| Surface | Blur | Tint (of the theme's colour) | Dim | Notes |
| --- | --- | --- | --- | --- |
| Sidebar | 12 px, saturate 120 % | 35 % | off (20 % if switched on) | light scheme adds a 50 % white wash |
| Middle card | 75 px, saturate 140 % | 85 % | 30 %, black in dark, white in light (boosted to about 65 % in light so a dark photo never greys the card) | |

So the card is a nearly solid sheet (about 0.9 of the picture covered, and 75 px blur on what remains) and the sidebars are the picture seen through light glass (0.35). The plugin renders the blur once into a small canvas (`frame/preblur.ts`, a 1/8-size copy for 75 px) and drops the live `backdrop-filter`; ours has done the same since the first veil work (`appearance/wallpaper.ts`: two pre-blurred copies, 12 and 75 px, no live blur on the card or the sidebars), so the cost was never the blur: the card was solved to the readability floor plus a margin, which left it as thin as 0.36 and as bright as the wallpaper.

**What changed.** The card now starts at the plugin's tint 0.85 and dim 0.3 (0.45 in the dark scheme, black; white in light) and the readability floor can only raise it; a sidebar starts at tint 0.35 with no dim and the floor raises its dim only as far as it needs (`appearance/veil.ts`). Nothing flips to a light veil in the dark scheme (a sidebar can still flip to dark text over a very dark picture in the light scheme). Clear to tinted keeps at least 0.3 of the scheme's own paper over every surface.

**Contrast between the card and each sidebar** (WCAG ratio of the mean colour of each surface with text and icons hidden; default wallpaper; 1500 x 970; `e2e/material-contrast.mjs`, dev-02; the plugin's recipe is painted onto our shell with `PLUGIN=1` so both are measured the same way):

| | dark: card vs left / right | light: card vs left / right |
| --- | --- | --- |
| before (solved floor, flip allowed) | 1.16 / 1.19 (the build on the shared port; see below) | 1.20 / 1.09 |
| plugin recipe | 1.18 / 1.04 | 1.21 / 1.19 |
| now | 1.15 / 1.06 | 1.57 / 1.43 |

Measured colours now: dark card (31, 29, 28) against left sidebar (45, 40, 36); light card (233, 231, 230) against (192, 186, 179). The dark sidebar stays dim because the 7:1 floor on the default photograph's highlights asks for a dim of 0.5; the plugin has no such floor, which is why its dark sidebars are brighter (55, 45, 36) while ours keep their text readable. The two are within 0.03 of each other in dark and ours is 0.3 stronger in light.

(The "before" row was measured against a preview already running on the shared test port; treat it as indicative. The two after rows were measured against our own build on a free port, `e2e/with-preview.mjs`.)

**Why Clear made the sidebar light in dark mode.** Two causes: the solver could flip a sidebar to the other scheme's ink and paper whenever that needed clearly less veil (a bright photograph in the dark scheme did), and the slider scaled whatever veil came out, so a flipped light veil at low strength read as a light tint; the scaled veil also had no floor. Fixed by never flipping in the dark scheme and by `scaled()` (`veil.ts`): strength is multiplied but the colours stay the scheme's paper and the total never falls under 0.3.

**Clear to tinted is a stepped control** (`StepSlider` in `packages/ui`): five named steps (Clear, Light, Balanced, Dense, Tinted) that multiply the veil by 0.3, 0.65, 1, 1.4 and 1.8; it snaps, a press or drag moves to the nearest step, Left/Down and Right/Up move one step, Home and End the ends, the ends are labelled and `aria-valuetext` names the step. Balanced (the middle) is the solved veil. A stored value from the old continuous slider rounds to the nearest step. Measured per step in the dark scheme (left sidebar mean colour): Clear (56, 46, 37), Light (51, 43, 37), Balanced (45, 40, 36), Dense (40, 38, 36), Tinted (37, 36, 36): darker with every step and never light.

## Switch (2026-11-18)

`Switch` in `packages/ui` is the squish switch of sveltebits.xyz/r/squish-switch ported without its `motion` dependency, and it is the only on/off control: Settings' switches and the Materials boxes (Blur, Dim, Tint, formerly checkboxes), the Plugins pages, the appearance sheet, the generated-UI toggle (`GenNode`) all use it; real choice lists (a checklist of options) stay checkboxes.

Ported behaviour and feel (the source's numbers): the knob rides a spring (stiffness 170, damping 21.5, mass 0.9) toward its end; it stretches along its direction of travel with a softened copy of its speed (a second spring, stiffness 320, damping 40, mass 0.6; scale x = 1 + min(0.4, speed / 600) x 0.36, scale y its reciprocal, so it squashes across); under a mouse it swells to 1.035 on `--spring`; a drag follows the pointer, flips as it crosses the middle and releases on the spring with the speed it had; a tap has a 4 px slop (8 px on touch); Escape during a drag restores the state at press; the track's colour fades on `--dur-base`. Our changes: sizes come from the spacing tokens (so the interface scale applies); the speed is taken as if the travel were the original's 38 px because our knob travels 12 px, so the squish is as visible on a small switch (up to 1.08 x 0.93 measured); the label is a required prop that names the switch (`aria-labelledby`), `hint` is a second line; the row's label is a `<label>` for it, so a tap anywhere on the row toggles it; Space and Enter work because the knob is a `<button role="switch">`; the focus ring is the shell's; reduced motion: no spring, no stretch, no swell, the knob jumps.

Implementation: at rest the knob's position is plain CSS (`aria-checked` picks the end), so a first paint needs no script; while it moves one rAF loop drives a `transform` (`Spring` in `packages/ui/src/lib/spring-step.ts`, about 30 lines, semi-implicit Euler in 1/240 s sub-steps) and stops at rest. No dependency was added: the first-load chunks went from 65,425 to 65,378 bytes of gzipped JavaScript and from 11,372 to 11,319 of CSS (the switch is in the lazy Settings chunk; the whole build is 446 bytes larger gzipped), measured with `gzip -9` on dev-02. Checked by `packages/ui/test/switch.test.ts` and `e2e/switch.mjs` (stretch, rest, Space, drag, Escape, reduced motion), which is in the gate.

## One material for floating surfaces (2026-11-18)

The owner: context menus and everything floating should follow one shared transparency and blur system that adapts for readability and contrast. The material was already one primitive (`Panel` with `Glass`, `packages/ui`, styles in `glass.css`: a nearly clear tint, 16 px blur, saturation and a brightness shift) but two things made it look like several: some floating surfaces were solid, and its tint was guessed from the wallpaper's brightness grid, not from what is really behind the surface (a menu over the nearly opaque card was tinted as if over the photograph).

**Every floating surface and the material it used before:**

| Surface | Before | Now |
| --- | --- | --- |
| Context menu, menu, picker (provider, permission, runs on), popover | `Panel` dense (shared glass) | same, tint now from the real backdrop |
| Sheet: palette, appearance sheet, dialogs | `Panel` dense floating | same |
| Toast | `Glass` capsule | same |
| Composer, request bar, notice, attention card, title pill, Notes contents card | `Panel` | same |
| Pill, capsule, toolbar | `Glass` | same |
| Tooltip | solid `--ink` fill with paper text (its own rules in `tooltip.css`) | the shared glass (`glass dense`), ink text, tinted like the others |
| Server-link banner ("Can't reach the server") | solid `--paper` pill | `Panel` pill, the shared glass |
| Middle card, sidebars | not floating: the background layer (docs/ui-shell.md "The background") and the card's own surface | unchanged |

**One rule for the tint.** `packages/ui/src/lib/material.ts` keeps the registry (`adaptive`, `scheduleMaterial`) and now takes one source from the app, `setMaterialSource`: `appearance/wallpaper.ts` registers it whenever the veils are solved. For a surface's rectangle it reads what is really behind it (the wallpaper copy under the one sidebar veil, or the card's copy under the card's veil, according to where the surface sits), and returns the smallest tint of the scheme's paper that brings the ink to the same readability floor the sidebars are solved to (7:1 on the worst 1 % of the pixels, plus the same headroom). `--mat-a` is never below the glass's own base (0.04) nor above 0.9; `--mat-blur` stays as before (16 px, a fifth more where the backdrop fights the ink). Without a source (fixtures with no wallpaper) the brightness-grid guess applies, as before. Reads are batched in one frame with the others (the source reads pictures the app already holds, never the page). The Threads lane builds on `Panel`; nothing forks it.

On the default wallpaper every surface above reports `--mat-a` 0.04 in both schemes: the sidebars' veil already makes the ground readable, so no extra tint is needed and the glass stays clear; over a bright photograph in the dark scheme, or a dark one in the light scheme, the tint rises as the floor requires. `e2e/floating-material.mjs` (in the gate) opens a context menu, a picker, the palette and a tooltip in both schemes and checks each is the shared glass (a `.glass` child with a backdrop blur and no background of the surface's own) with the tint written.

## Selection switching and toasts, ported from the existing app (2026-11-18)

**Selector** (`packages/ui` `Selector`, from the existing app's `primitives/selector`): one component for every single choice. A capsule sits behind plain options; when the choice changes its leading edge stretches toward the new option on a quick spring and the trailing edge catches up on a softer one, so it stretches and then gathers itself; labels never move; click again mid-flight and it turns from where it is. It is a tab list (arrow keys, Home and End move and choose, only the chosen option is in the Tab order, disabled ones are skipped) and a keyboard move goes straight there (keyboard actions never animate). Used for every segmented choice (the app capsule in the sidebar footer, the right sidebar tabs, the machine pickers, the colour scheme, chat layout, the readability floor, and the others) and for a plugin's `Choice` (GenUI), so a plugin's choice looks and moves like the app's own. `edges.ts` holds the edge rules as plain numbers and is tested without a browser (`test/selector-*.test.ts`).

**Selectors fill their row (one rule, 2026-11-18).** A Selector that is the control of its own row fills the row's width: `fill` is its default and the class `row-selector` caps it at `--selector-row-max` (28 rem, about 450 px: three or four options stay comfortable in a wide settings pane, and on a phone the cap is above the row width so it simply fills). Settings rows that hold a segmented choice (the colour scheme, the accent source, chat layout, permission mode, window look) stack the control under the label so it can fill. Only a Selector that sits inline beside other content in the same row is sized to its content (`fill={false}`): the machine and member lists (name on the left, the mode or role on the right), the app capsule in the sidebar footer (beside the avatar and search), and a plugin's GenUI `Choice` (the host cannot know its neighbours). Everything else fills: the Inbox / All Threads tabs, the right sidebar tabs, the where-it-runs mode, the font source, the invite and member role pickers, the setup connection choice.

**Toasts** (`packages/ui` `toast/`): already the existing app's Sonner-style implementation (`notify`, `stack`, `bursts`, the capsule that opens from a dot and keeps being the same toast as its words change, `Roll` for counts, undoable and failed variants); only its in-the-bar variant (`inbar`) is not ported, because this shell has no bar for it.

**What a plugin needs.** A plugin's UI is GenUI (portable) first: `Choice` already renders as the Selector. A toast needs a `Toast` action in the GenUI catalog (`packages/genui/src/catalog-data.json` next to `Send`, `Set`, `OpenUrl`, `Copy`) taking a message and an optional action, and the host mapping it to `toast()` / `undoable()`; and a plugin written in TypeScript against the plugin SDK would need `packages/plugin-sdk` to expose the shared UI surface (`@calternal/ui`) across the plugin boundary, which is a decision about what a plugin may import. Neither is built.


## Corner radii (2026-11-18)

One scale, in `packages/ui/tokens/tokens.json` (`r-*`, multiplied by `--ui-scale`), concentric: **a child's radius is its parent's minus the gap between their edges**. The card's value follows the reference (the current bb with the calternal-ui plugin, `bb-plugins/calternal-ui/frame/css.ts`: surfaces 14, cards 22 to 24, menus 14 = item 8 + padding 6, phones follow the screen's corner).

| Token | px | What uses it | Why |
| --- | --- | --- | --- |
| `--r-pane` | 28 (was 20) | the middle card, a sheet (palette, dialogs, the right and bottom sheets), floating panels | the outermost shape on the screen |
| `--r-card` | 20 (was 14) | what sits 8 inside a pane: the composer, request bar and notices in the card, windows, settings groups, code blocks, toasts, attention cards, side chat | 28 minus the 8 px inset (`--inset`) |
| `--r-menu` | 14 (new) | menus, popovers (`Panel radius="menu"`) | a card inset 6, or an item (8) plus 6 of padding |
| `--r-select` | 8 | rows, inputs, buttons, tags, keycaps in a field, tooltips (was 9) | the innermost shape |
| `--r-pill` | 999 | pills, capsules, the switcher, the selector, the esc and clear buttons | half the height |
| `50%` | circle | avatars, dots, round icon buttons | |

Every radius that was in use before, and what it became: `--r-pane` 20 became 28; `--r-card` 14 became 20 (menus and popovers that used it moved to the new `--r-menu` 14); the composer, request bar and notices were `pane` (20) and are now `card` (20) so they stay concentric in the 28 card; `--r-select` 8 unchanged; tooltip 9px became `--r-select`; toast capsule 20px became `--r-card`; toast row, selector 999px became `--r-pill`; swipe action 8px became `--r-select`. Left as they are, with the reason: `50%` circles (15 uses); inline marks, the caret and keycaps (2 to 5 px, relative to the text, not to a surface); the preset gallery's 2 px skeleton bars; the tooltip's `0.45em` keycap. On a phone the card at rest is square and, with a sidebar open, takes `--r-pane`; the sheets' device-corner rule is in the inset sheet (next section).


## Menus and sheets (2026-11-18)

A menu is a **dropdown**, on a phone too: anchored to the button (or the point, for a context menu), morphing out of it (`Menu` on `Popover`, `morph`), rows of the native menu height (42). An overflow (three-dot) menu, a message's actions, a thread's actions, the attach menu, a short list of choices (permission mode, a setting's options) are dropdowns. A **sheet** (the inset bottom sheet, `Sheet`) is for something that needs the height or the keyboard: a picker with a search field (provider, model and reasoning), a list longer than eight rows (`SHEET_ROWS` in `Menu.svelte`), a form, the palette, the first-run look. `Menu` applies this itself: only `search` menus and long lists become sheets on a touch screen, so no caller chooses per device. Checked on a phone by `e2e/phone-menus.mjs`.


## Readability floor and Clear to tinted (2026-11-18, closes #24)

One behaviour, no comparison toggles: the veil is solved to a **7:1** readability floor (secondary ink 5.5:1, tertiary 4.5:1) with text that flips between light and dark where it must, and the light scheme's surfaces are the **lighter copy of the photo** under the solved veil. The "Compare" section, its readability-floor and light-surface toggles, the 4.5:1 floor and the Craft 0.92-white path (and their code and tests) are gone. **Clear to tinted** (five steps: Clear, Light, Balanced, Dense, Tinted; Balanced is the solved veil) is a normal setting in Materials, starting from the system's reduced-transparency and contrast settings.

**What a step costs.** A step of the slider changes one number (`setClarity`) and solves the veil again from the two blurred copies of the picture that already exist (`updateVeil`: sampling 48 by 48 pixels per surface and writing a few custom properties). It never decodes or re-blurs the picture: `setWallpaper` is a no-op for the same picture with the same blur radii (it was not: every preference change re-blurred it, which made each step a 400 to 780 ms main-thread task at 4x CPU). Writing the preferences to storage waits 300 ms after the last change (and happens at once when the page is hidden), so a drag writes once and the visual never waits for storage. Gate: `e2e/clarity-perf.mjs` drives all five steps by keyboard, up and down, and by a pointer drag along the rail at 4x CPU throttling, in both colour schemes, and fails on a main-thread task over 50 ms, on rebuilt wallpaper blob URLs, and on a step that is not on screen within a few frames.
