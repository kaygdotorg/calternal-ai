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
  `--row-h`, `--bar-h` (header band) and the phone's bottom capsule `--tier-cap`. Four radii: `--r-pill`, `--r-card` 14,
  `--r-pane` 20, `--r-select` 8.
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
| `Capsule` | Segmented control; `kind="tabs"` or `"radio"`; `morph` is the phone's bottom bar | `label` required; each item has a `label` even when only its icon shows |
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
- **Thread rows are one line:** title, time (hover only), compact state, provider mark. No second line, no automatic workspace labels; people make their own sections.

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

The existing app's press, as plain CSS on each control (`Pill`, `IconButton`, `Button`, `Row`, `Capsule`): `scale` goes to `--press-scale` while pressed and returns over
`--dur-fast` on `--ease-out`, transform only. There is no rebound past 1 (owner, 2026-11-18: the overshoot was too much; the existing app has none). A keyboard activation never
moves it (key modality makes every transition 0 s) and reduced motion keeps only the scale.

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
