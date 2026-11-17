# Design system (`packages/ui`)

Tokens are data; surfaces are components. Visual language is ported from the mockup
(`origin/review/switcher-a-pills`: `packages/ui/src/styles/tokens.css`, `glass.css`, `Pill`, `Toolbar`,
`Icon`); `docs/ui-shell.md` wins where they differ. Deviations are listed in `docs/ui-shell-status.md`.

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

## Adaptive material

`lib/material.ts`. A surface uses `use:adaptive` (the components do it themselves). The background is a
colour mesh, the same data that paints it; the brightness behind a surface is nine interpolations of that
mesh inside the surface's rectangle. `material(scheme, behind, role)` returns the tint alpha and blur:
unchanged over a calm backdrop, denser (up to +0.45 alpha) and slightly blurrier where the backdrop fights
the ink. Values are written once per layout change as `--mat-a` and `--mat-blur` on the surface (batched in
one animation frame, reads before writes). Nothing runs on scroll and no pixel is ever read.

The blur is capped near the mockup's floating-glass 20 px: the headless Chromium used for screenshots stops
blurring beyond about 24 px, so a larger value would not be verifiable here.

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
