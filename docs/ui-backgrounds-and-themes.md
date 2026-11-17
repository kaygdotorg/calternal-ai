# Backgrounds, wallpapers, themes and presets

Research by the `lane/ui-backgrounds` lane (issue #20), 2026-11-17. A recommendation for the UI lane; nothing here is
decided until the owner answers the questions under **Open**. Evidence lives in three demos under
`docs/demos/ui-backgrounds/` (each a few KB, no network, open in any browser) and in screenshots on issue #20.

Inputs read: the owner's plugin `calternal-ui` (`frame/wallpaper.ts`, `preblur.ts`, `color.ts`, `server/unsplash.ts`,
`server/wallpapers.ts`), the Craft studies in the existing app's repo (`craft-styles-study-2026-11-09.md`,
`craft-immersive-faded-2026-11-10.md`, `project-craft-styles-gui-pass.md`), `docs/ui-shell.md`, and on `lane/ui-shell`
`docs/ui-design-system.md` and `packages/ui/tokens/tokens.json`. Licences were read from each upstream repository's own
`LICENSE` (cloned to `/tmp`), not from third-party summaries, unless a row says otherwise.

## Summary (what to build)

1. **Generated backgrounds are recipes, not images.** A recipe is 6 to 9 colours, a seed and a few numbers (about 100
   bytes of Markdown). One small deterministic renderer (OKLab blend, seeded domain warp, monochrome grain) turns it into
   a 2560 by 1440 image **once**; the shell then pre-blurs that image once (the plugin's `preblur.ts`, already ported).
   Nothing animates, nothing blurs per frame. Adopt no runtime gradient library.
2. **Grain is a static tile above the blurred layers, not baked into the wallpaper.** Measured: lossy encoders erase
   grain (AVIF at quality 70 and WebP at 85 are *more* banded than the ungrained source), so grain added before encoding
   is lost exactly when files are small.
3. **The default set is eight generated looks, each in a light and a dark rendition, plus a short curated photo and
   painting shelf from CC0 sources.** Generated looks need a 35% wash for legible ink; every photo tried needs 70 to
   85%. Photos only ship if they pass the same guard.
4. **Themes are palettes (8 colours) plus derivation rules**, not 40 hand-written token sets. 24 community variants map
   with one function; the contrast guard moves a colour away from upstream in 10 of them (ink-3) and 8 (accent text).
5. **Presets are the gallery.** A preset = palette + backdrop recipe + accent + reading face + material, shown as a
   miniature of the real shell. First set of 12 below.

## 1. Generated backgrounds

### Verdict, ranked

| Rank | Choice | Why |
| --- | --- | --- |
| 1 | **Own renderer: seeded OKLab mesh + grain, rendered once** (`docs/demos/ui-backgrounds/mesh-grain.html`, about 40 lines) | Deterministic; same recipe gives the same pixels on every client when rendered by one implementation; no dependency; 0.3 to 0.6 s for 2560 by 1440 in V8 (measured, 4 runs); cheaper still in Rust |
| 2 | **Paper Shaders `static-mesh-gradient` / `grain-gradient`** (`@paper-design/shaders` 0.0.82, Apache-2.0, 881 KB unpacked, 0 deps, WebGL) | Best-looking library found, and the author's static variants exist; but it is WebGL (needs a GL context just to make one image), pre-1.0 with documented breaking changes, and cannot run in the Swift client. Use as a *visual reference* when tuning recipes, not as a dependency |
| 3 | `grain-gradient` 2.0.0 (MIT, 35 KB, 0 deps) | Small and static-capable; unproven; same cross-client problem |
| Avoid | Animated WebGL mesh (Stripe-style "whatamesh", Paper's animated `mesh-gradient`, Unicorn Studio) | Continuous GPU and battery cost, fights `prefers-reduced-motion`, and exactly the look called slop |

Paper Shaders licence conflict: the repository `LICENSE` and both `package.json` files say **Apache-2.0**; the project's
own Mintlify docs page says PolyForm Shield. Trust the repository, but if it is ever adopted, record the commit and
re-read the licence at that commit.

### Technique findings (all from the demo, Chromium on `calternal-dev-02-std`)

- **Interpolate in OKLab, never sRGB.** Blending in sRGB passes through a grey, dead mid-zone and desaturates hue
  transitions; OKLab keeps lightness monotonic. The demo's Gaussian-weighted points in OKLab plus a 5% sine domain warp
  gives organic shapes with no visible blobs.
- **8-bit banding is real and measurable.** Mean run of identical pixels along a row (long runs mean contour banding,
  about 1 to 2 mean dithered), 2560 by 1440 source, 400 rows sampled:

  | Source or encoding | Mean run (px) |
  | --- | --- |
  | PNG, grain 2.5/255 | 1.1 |
  | PNG, no grain | 12.9 |
  | JPEG q90 (mozjpeg) | 8.4 |
  | AVIF q85 | 9.2 |
  | WebP q85 | 23.5 |
  | AVIF q70 | 25.6 |
  | JPEG q75 | 27.5 |
  | AVIF q50 | 27.7 |

  Grain baked into a lossy file survives only at JPEG q90 and AVIF q85; below that the file is *worse* than having no
  grain. (File sizes for the grained 2560 by 1440 mesh: AVIF q70 4.8 KB, WebP q85 11.9 KB, mozjpeg q90 50 KB, JPEG q90
  from the browser 142 KB; the tiny files are small because they discarded the grain.)
- **Therefore: grain is a runtime layer.** Generate a 256 by 256 monochrome noise tile on a canvas once (65 k pixels,
  well under a millisecond; ships zero bytes), use it as a plain alpha `background-image` over the sidebars' layer at
  about 2 to 3% opacity. Do not use `mix-blend-mode` or a live SVG `feTurbulence` filter on a viewport-sized layer
  (both are per-paint compositing or rasterising work; not measured on devices here, so the shell lane must measure
  before relaxing this). A pre-blurred copy exported as JPEG (as `preblur.ts` does at quality 0.9) is smooth content and
  will band without this tile.
- **Taste, by comparison** (screenshot on the issue). What reads as expensive: 5 to 9 low-chroma points from one or two
  analogous hues, a total lightness spread of about 15 to 25% across the picture, one warm and one cool anchor, grain
  you cannot see but would miss. What reads as AI-generated: three-stop hue-spanning linear gradients (purple to pink to
  cyan), saturated radial "orbs" on near-black, glow hotspots, mid-frame hue fights, visible banding. Rule: **chroma
  below about 0.06 OKLCH for the mesh, hue travel under about 60 degrees, no pure primaries.**
- **What other apps do.** Craft's backdrops are flat colours, flat-ish 12 presets, photos, or none; its own measurements
  found *no grain* in the app (only in marketing slides) and an opaque page card, so its premium feel comes from curation
  and pairing, not texture. macOS Tahoe's dynamic wallpapers are pre-rendered stills or video, not procedural. No
  reliable public detail was found for Arc, Linear or Raycast backgrounds, so none is claimed.

### Proposed default look

The shell's `tokens.json` already carries a 3 by 3 `backdrop` colour mesh per scheme (`#efe6dc` to `#d9dfe4` light,
`#2d2621` to `#191e23` dark). Those colours are calm. Render them once through the OKLab mesh with grain (third demo
row) and keep them as the **quiet default**. Their lightness spread is very small, which is also why the sidebar wash
can stay at the plugin's 35%. If the owner finds that too flat, the **Dusk / Sage / Slate** recipes in the demo are
the livelier alternative (still 35% wash). This is a taste call: see Open.

Render parameters: sigma 0.42 to 0.55 (of image width), warp 0.03 to 0.06, grain 2.5 only if the file is JPEG q90,
otherwise no baked grain and the runtime tile.

### Performance rules for backgrounds

- Render once per (recipe or image, scheme, viewport bucket). Cache the result (a file under userdata, not recomputed
  at start). The decoded 2560 by 1440 bitmap is 14.7 MB; release it after the pre-blur.
- Stored source: at most 2560 by 1440, JPEG q90 (mozjpeg) for generated recipes and photos alike, at most about 250 KB
  each (generated recipes also store only the recipe, so the image can be rebuilt; lossy-only storage of photos).
  AVIF is fine for photos (q85 is 17 KB for the grained mesh, far smaller for real photos) but never trust it to carry
  grain.
- Pre-blurred copies: as `preblur.ts` does (1/k of the viewport, k up to 8, blur and saturation baked in). Re-render
  after resize, debounced.
- No `backdrop-filter` over a large area; live blur only on small glass (already a shell rule).
- Declare the placeholder: a 24-byte ThumbHash (`thumbhash` 0.1.1, MIT, 13 KB) per wallpaper so the first paint is
  already the right colours before any image decodes. Optional; the existing 48 by 48 sampler already gives the tone.

## 2. Wallpapers

### Licence ranking for a bundled set

| Rank | Source | Licence, read from | Bundling verdict |
| --- | --- | --- | --- |
| 1 | **Our own generated recipes** | ours | No third-party terms; also the only set that can be re-rendered per scheme |
| 2 | **Met Museum Open Access** (public-domain works) | CC0, [Met policy](https://www.metmuseum.org/about-the-met/policies-and-documents/open-access); each object's `isPublicDomain` verified through the Met API | Safe to bundle; attribution not required; credit anyway |
| 2 | **Art Institute of Chicago Open Access** | CC0 for items marked "CC0 Public Domain Designation" ([artic.edu/open-access](https://www.artic.edu/open-access/open-access-images)) | Same; check the per-item label |
| 2 | **Smithsonian Open Access**, **Cleveland Museum**, **Getty** | CC0 (Cleveland and Getty by their announcements; Smithsonian page was not reachable from the lane, so confirm on [si.edu/openaccess](https://www.si.edu/openaccess)) | Same, after a per-item check |
| 3 | **NASA / ESA-Webb images** | Generally no US copyright; some items carry third-party credit; no implied endorsement; never the NASA logo ([NASA guidelines](https://www.nasa.gov/nasa-brand-center/images-and-media)) | Usable per item; keep a credit line |
| 4 | **Unsplash** | Unsplash License: free to use commercially, "cannot be sold without significant modification", and "does not include the right to compile images from Unsplash to replicate a similar or competing service" ([licence](https://unsplash.com/license), [help](https://help.unsplash.com/en/articles/2612331-why-can-t-i-compile-photos-from-unsplash-to-replicate-a-similar-or-competing-service)) | Fine for the *user's own pick through the API* (as the plugin does, with download tracking and UTM links). A fixed bundled set of ten is probably outside the "compile to replicate a service" clause but not clearly so; **do not bundle Unsplash photos without owner sign-off** |
| 5 | **Pexels** | Pexels licence: forbids redistributing on "other stock photo or wallpaper platforms" ([licence](https://www.pexels.com/license/)) | A wallpaper picker is close enough to that line to avoid |
| 6 | **Pixabay** | Bars distributing content "on a standalone basis" (unmodified); modification or combination required ([terms summary](https://pixabay.com/service/terms/)) | Avoid for bundling |

Unsplash search stays as a user-driven source exactly as the plugin does it (`server/unsplash.ts`): server holds the
key, registers each download through `download_location`, stores 2560 px plus a 480 px thumb once, serves it from our
storage, shows the credit with UTM links. The demo-app rate limit is 50 requests an hour, so production needs an
approved key.

### The default set: 8 looks, each light and dark

Generated recipes, each a Markdown file (frontmatter: `id`, `name`, `colours`, `seed`, `sigma`, `warp`), each rendered
twice (the dark rendition is the same recipe with lightness remapped, not a darkened copy):

| Id | Idea | Hue travel |
| --- | --- | --- |
| `haze` (default) | The shell's token mesh | warm grey to cool grey |
| `dusk` | Indigo to dusty rose (demo) | about 50 degrees |
| `sage` | Sage and linen (demo) | about 35 degrees |
| `slate` | Blue-grey (demo) | about 25 degrees |
| `dune` | Sand to terracotta | about 40 degrees |
| `lagoon` | Teal to deep blue | about 45 degrees |
| `moss` | Olive to cream | about 40 degrees |
| `orchid` | Lilac to rose | about 50 degrees |

(`dune`, `lagoon`, `moss`, `orchid` are the existing fixture palettes in `appearance/library.ts`; `slate` and `paper`
also exist there. Re-tune them to the chroma and spread rule above rather than copy them.) Plus two non-image options
that every client already supports: **None** (flat paper) and **Solid** (one colour with the Craft-measured tint rule
below).

Curated shelf, optional, 4 to 6 paintings and prints, all Met Open Access, verified `isPublicDomain: true` through the
Met API on 2026-11-17:

| Work | Object | Notes |
| --- | --- | --- |
| Hokusai, *Under the Wave off Kanagawa* | [39799](https://www.metmuseum.org/art/collection/search/39799) | cream and indigo, calm when blurred |
| Van Gogh, *Wheat Field with Cypresses* | [436535](https://www.metmuseum.org/art/collection/search/436535) | high contrast, needs the guard |
| Van Gogh, *Cypresses* | [437980](https://www.metmuseum.org/art/collection/search/437980) | green-blue |
| Turner, *Whalers* | [437854](https://www.metmuseum.org/art/collection/search/437854) | warm and soft, the best of the five |
| Ikeda Taishin, *Two-Tiered Box with Design of Autumn Fruits* | [45581](https://www.metmuseum.org/art/collection/search/45581) | lacquer; gold on dark |

All five *as tested* (screenshot on the issue: `photo-ink`) needed a 70 to 85% wash in either scheme, which makes them
read as a frame around the card rather than a wallpaper. So the shelf is **opt-in and each entry is cropped and
tone-compressed** (curves toward a narrow luminance band under the sidebar) before it is admitted; an entry is dropped
if its guarded wash exceeds 50%. Choosing 8 to 12: build the candidates, run the guard in both schemes, keep those at or
below 50%. The Met API (`collectionapi.metmuseum.org/public/collection/v1/objects/<id>`) returns the licence flag and a
2000-px image URL; the lane did not fetch originals beyond web-large copies.

### How apps derive legible ink and accent from a wallpaper

`docs/demos/ui-backgrounds/wallpaper-ink.html` is a runnable version; it mirrors `calternal-ui`'s `frame/color.ts` and
the shell's `washFor`:

1. Read the (pre-blurred) wallpaper **once** into a 24 by 14 grid.
2. Take the cells under the sidebars (left 28%). The worst cell is the brightest in the dark scheme or the darkest in the
   light scheme.
3. Raise a black (dark) or white (light) wash from the tint (35%) in 5% steps until secondary ink (`ink-3`) reaches
   **4.5:1** on the worst cell, capped at 90%.
4. Accent: bin pixels by OKLCH hue (15-degree bins), score by count times chroma, take the best bin; chroma clamped to
   0.09 to 0.17; lightness 0.50 (light) or 0.78 (dark); then nudge lightness 0.02 at a time until the accent is 4.5:1 on
   the card.
5. Pill material fades in only where the wallpaper fights its glyphs (the shell's `material()`; already ported).

Measured in the demo (secondary ink 4.5:1 or better after the wash):

| Wallpaper | Light scheme wash | Dark scheme wash |
| --- | --- | --- |
| Sage | 35% | 75% |
| Dusk | 85% | 35% |
| Dune | 65% | 70% |
| Harsh stress test | 80% | 80% |
| Five paintings | 80 to 85% | 70 to 75% |

Finding: **a wallpaper only works in the scheme it matches.** A dark wallpaper in the light scheme (or the reverse)
needs a wash near 85%, which is the wallpaper disappearing. That is why every generated look ships a light and a dark
rendition, and why the plugin's single "Automatic" mapping (Dune in light, Aurora in dark) is the right instinct.
Libraries, if the in-house sampler is ever not enough: `@material/material-color-utilities` 0.4.0 (Apache-2.0, 1.06 MB
unpacked; Celebi quantizer plus a `score` that ranks colours for theming; **also ships in Swift**, so web and native can
agree), `node-vibrant` 4.0.4 (MIT, 20 KB, 6 deps), `fast-average-color` 9.6.0 (MIT, 78 KB), `colorthief` 3.5.0 (MIT).
Recommendation: keep the owner's 48 by 48 sampler for the web (it is 40 lines and proven) and port the same algorithm to
the Rust shared client core, so Swift gets identical colours; reach for the Material quantizer only if a photo gives a
bad hue.

### Craft's backdrop rules worth copying (measured, not marketing)

From the 2026-11-10 study: the *sidebar tint is a blend, not a filter*: `sidebar = k * backdrop + (1 - k) * neutral`,
k 0.225 light (neutral `#e9e9e9`), 0.27 dark (neutral `#454545`). **Faded** blends toward a foot neutral (`#bcbcbc` light,
`#3f3f3f` dark) linearly down the window. Photo backdrops are not touched by Faded. The owner chose not to copy the
backdrop wash; sidebars only. Craft ships four backdrop modes (none, solid, gradient, image), 12 solid swatches, 12
gradient presets (start, end, direction: top to bottom, left to right, radial), six featured photos with Shuffle,
Unsplash and Upload, and a Blur Image switch. Style change is a 280 to 300 ms whole-window cross-fade, ease-in-out.

## 3. Community themes

All palettes below were read from the upstream repository named in the third column. Licences are the file's own text.

| Theme (variants) | Licence | Official source | Notes for mapping |
| --- | --- | --- | --- |
| **Catppuccin** (Latte; Frappé, Macchiato, Mocha) | MIT | [catppuccin/palette](https://github.com/catppuccin/palette) `palette.json` | 26 named colours; accent is user-chosen (default Mauve); ships a ready role set |
| **Rosé Pine** (main, Moon, Dawn) | MIT | [rose-pine/palette](https://github.com/rose-pine/palette) `palette.json` | No green or blue: `good` = foam, `info` = pine |
| **Tokyo Night** (Night, Storm, Day) | **Apache-2.0** (folke/tokyonight.nvim; other ports vary, enkia's VS Code theme is MIT) | [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim) `extras/lua` | Apache needs the NOTICE and a state-changes line; keep the licence text in the theme file |
| **Nord** | MIT | [nordtheme/nord](https://github.com/nordtheme/nord) `nord.less` (16 colours) | Light variant is Snow Storm on Nord0 ink; `good` `#a3be8c` fails as text on light |
| **Gruvbox** (dark, light) | MIT | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) `colors/gruvbox.vim` | Light uses the `faded_*` set |
| **Dracula** (+ Alucard light) | MIT | [dracula/dracula-theme](https://github.com/dracula/dracula-theme) README tables | Alucard is Dracula's official light |
| **Solarized** (dark, light) | MIT | [altercation/solarized](https://github.com/altercation/solarized) | Designed for low contrast: guard raises ink-3; base fg itself is about 4.1:1 on the light background |
| **Everforest** (dark, light, hard/medium/soft) | MIT | [sainnhe/everforest](https://github.com/sainnhe/everforest) `autoload/everforest.vim` | Accent is green |
| **Kanagawa** (Wave, Dragon, Lotus) | MIT | [rebelot/kanagawa.nvim](https://github.com/rebelot/kanagawa.nvim) `colors.lua` | Lotus is the light one (paper `#f2ecbc`) |
| **One Dark / One Light** | MIT (Atom `one-dark-syntax`; the popular VS Code port is ISC) | Atom's One syntax themes | Values here are the widely used Atom ones; not re-read from a clone, verify when generating |
| **GitHub** (dark, light) | MIT | [primer/github-vscode-theme](https://github.com/primer/github-vscode-theme), values via `@primer/primitives` 7.10 | Values read from the package's colour JSON |
| **Ayu** (Light, Mirage, Dark) | MIT (`ayu-theme/ayu-colors`; `ayu-vim` is Apache-2.0, avoid it as the source) | [ayu-theme/ayu-colors](https://github.com/ayu-theme/ayu-colors) 9.1.0 | `common.accent.tint` is the accent |

Others found widely used and not mapped here: Monokai (no clear open licence for the original; skip), Material Theme
(MIT), Night Owl (MIT), Palenight, Oceanic Next, Zenburn, Horizon. None adds a distinct *kind* of palette; add on request.

Colour values for the 24 variants are in `docs/demos/ui-backgrounds/themes-map.html` (it is the data, readable at the top
of its script). Catppuccin Latte/Mocha, Rosé Pine (all three), Tokyo Night Night/Day, Nord, Gruvbox, Dracula/Alucard,
Solarized, Everforest, Kanagawa, GitHub and Ayu were taken from the cloned or fetched sources; One Light/Dark and Tokyo
Night's yellow/warn values are from memory and must be re-read before they ship.

### How a palette maps onto the token set

Tokens stay the single source: `packages/ui/tokens/tokens.json` names them; native clients read the same file. A theme
supplies **eight colours** and the client core **derives the rest**, so Swift and TypeScript agree:

| Theme input | Becomes token(s) | Rule |
| --- | --- | --- |
| `bg` | `paper`, `tint`, base of the `backdrop` mesh | `paper = bg`; sidebar tint = bg shifted 35% toward white (light) or 20% toward black (dark) |
| `fg` | `ink`, `ink-rgb`, `shadow-rgb` | direct |
| (derived) | `ink-2`, `ink-3`, `ink-4` | mix(paper, ink) at **82 / 70 / 54%** in OKLab (what the current light and dark tokens measure), then **ink-3 guarded to 4.5:1** |
| (derived) | `line`, `hover`, `press` | ink at 12% / 5% / 9% alpha (current tokens) |
| `accent` | `accent-base`; `accent` (text); `accent-soft` | `accent-base` direct; text variant nudged in OKLCH lightness to **4.5:1** on paper; soft = 14% (light) or 22% (dark) mix into paper |
| `danger`, `good`, `warn`, `info` | same names | direct, text uses guarded to 4.5:1 |
| (derived) | card surface | paper mixed 2.5% (light) or 3.5% (dark) toward black or white |
| (derived) | `backdrop` 3 by 3 mesh | nine points = paper at lightness plus or minus 3%, chroma pulled toward the accent hue at no more than 0.02 |
| fixed | `glass-*`, shadows, radii, motion, type | **not themable** by palette; themes cannot alter geometry or motion |

Result of running the mapper on all 24 (demo screenshot on the issue): **10 variants need ink-3 moved** (Catppuccin
Latte, Rosé Pine Dawn, Tokyo Night Day, Solarized Light and Dark, Everforest Light and Dark, Kanagawa Lotus, One Dark,
Ayu Light) and **8 need the accent moved for text** (Rosé Pine Dawn, Tokyo Night Day, Nord Snow Storm, Solarized Light
and Dark, Everforest Light, One Light, Ayu Light). The upstream colour is kept for fills and swatches; only text uses the
guarded value. Two palettes cannot reach 4.5:1 by moving ink-3 toward their own `fg`, because the `fg` itself falls short:
**Solarized Light (about 4.0:1) and Tokyo Night Day (about 4.4:1)**. For those two the guard must be allowed to push ink
past the palette's `fg` toward black, or the theme ships with a stated exception; this is an Open item.

Theme file shape (human-readable first, per the owner's rule), one Markdown file per variant:

```
---
id: catppuccin-mocha
name: Catppuccin Mocha
scheme: dark
paper: "#1e1e2e"
ink: "#cdd6f4"
accent: "#cba6f7"
danger: "#f38ba8"
good: "#a6e3a1"
warn: "#f9e2af"
info: "#89b4fa"
licence: MIT
source: https://github.com/catppuccin/catppuccin
---
Optional notes. Palette © Catppuccin, MIT.
```

Do not ship a theme as CSS: the plugin API says themes are design tokens every client reads; CSS stays a web-only extra.
Ship the licence line and source for every bundled theme (MIT needs the notice; Tokyo Night's Apache-2.0 needs the
licence text and NOTICE preserved).

## 4. Presets

### What makes Craft's gallery feel curated (measured)

- **One flat gallery, no categories** (131 styles, three columns, a search field, names of one to three words).
- **Each tile is a portrait miniature of the real page** on its own backdrop, with the style name set *in that style's
  own font*, plus the real separator and cover. Preview is the product, not a swatch.
- **Pairing is the work.** A style is font + text colour + paper colour + cover + backdrop + separator; Craft's own
  analysis: 81 light and 50 dark papers, 51 gradient, 47 solid, 17 photo backdrops, 16 with no card, 17 that flip with
  the app appearance (by *swapping paper and text and keeping the backdrop*), 114 fixed.
- **Applying is instant and reversible**: one click, one 300 ms cross-fade of the whole window, chrome included.
- **Restraint**: no grain, an opaque page, a card radius of about 10 pt, almost no shadow, no per-style accent
  (checkboxes follow the text colour), contrast guard with a one-tap fix.
- **Saved custom styles first**; the gallery pins a default.

### Proposed Calternal preset anatomy

`backdrop recipe + scheme palette (8 colours) + accent + reading face + material`, plus whether it flips between
schemes. Fonts stay Google Sans Flex and Maple Mono by default (owner decision); a preset may pick one reading face from a
short list (Google Sans Flex, a serif, Maple Mono) but never a font file we do not ship. Material is `glass` (the
shell's pane) or `paper` (Craft's opaque card), nothing else.

Gallery tile = a 4:3 miniature of the actual three-region shell (flat sidebars on the backdrop, dimmed card), the
preset name set in its reading face, a half-filled circle if it flips, a ring on the selected tile, saved presets
first, a search field. The miniature renders from the same tokens, so it cannot drift from the product.

### First 12

| # | Name | Backdrop | Palette | Accent | Reading face | Material | Twin |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | **Calternal** (default) | `haze` | built-in tokens | amber `#c26a12` | Google Sans Flex | glass | flips |
| 2 | **Paper** | none, warm flat | built-in, paper `#f7f7f7` / `#1f1f1f` | amber | serif | paper | flips |
| 3 | **Slate** | `slate` | built-in dark-led | blue-grey | Google Sans Flex | glass | flips |
| 4 | **Dune** | `dune` | warm | terracotta | serif | glass | flips |
| 5 | **Sage** | `sage` | green-grey | sage | Google Sans Flex | glass | flips |
| 6 | **Dusk** | `dusk` | indigo-rose | rose | Google Sans Flex | glass | flips |
| 7 | **Catppuccin** | derived mesh | Latte / Mocha | mauve | Google Sans Flex | paper | flips |
| 8 | **Rosé Pine** | derived mesh | Dawn / main | iris | Google Sans Flex | paper | flips |
| 9 | **Tokyo Night** | derived mesh | Day / Night | blue | Maple Mono | paper | flips |
| 10 | **Nord** | derived mesh | Snow Storm / Nord | frost | Google Sans Flex | paper | flips |
| 11 | **Gruvbox** | derived mesh | light / dark | orange | Maple Mono | paper | flips |
| 12 | **Mono** | none | grey scale | ink | Maple Mono | paper | flips |

Community-palette presets use the palette's own paper and a mesh derived from it (rule above), so Catppuccin Mocha has
no stock wallpaper behind it; its look is its colours. Every preset is twin-capable because every row has both
variants; a "fixed" flag exists for a future photo preset. Presets are Markdown files too (frontmatter holds the
recipe reference, theme id, accent, face, material).

## Performance rules (all four areas, in one place)

1. Everything expensive happens when a choice changes, never per frame or per scroll. State the cost in each public
   function's doc comment (the lane's review rule).
2. A wallpaper change costs: one decode, two canvas renders (sidebars at 1/k, card at 1/8), one 24 by 14 read, one 48
   by 48 read, one grain tile if missing. Resize re-renders after 400 ms.
3. No `backdrop-filter` over a large area; no animated gradient; no `mix-blend-mode` or live SVG filter on a large
   layer; grain as an alpha tile.
4. Stored image at most 2560 by 1440, JPEG q90 mozjpeg, at most about 250 KB; photos may be AVIF but never carry the
   grain; generated looks also keep their recipe.
5. Memory: release the full decode after pre-blur; keep only the 1/k copies.
6. Preset switch is one 300 ms cross-fade, no layout change; respect `prefers-reduced-motion` (1 ms) and
   `prefers-reduced-transparency` (near-opaque glass).
7. Measured on real devices before the shell lane relaxes any of this; none of the GPU cost figures above are measured
   here (headless Chromium only; see Open).

## What to avoid, and why

- **Rainbow or purple-pink-cyan gradients, glow orbs, saturated blobs on black.** They are the visible signature of
  generated slop and fight text. Chroma and hue-travel limits above.
- **sRGB gradient interpolation and unguarded 8-bit ramps.** Grey mid-zone; measurable banding.
- **Animated WebGL backgrounds.** Continuous GPU, battery, motion-sickness, reduced-motion conflicts, no native parity.
- **Grain baked into lossy images**: measured to vanish below JPEG q90 and AVIF q85.
- **Big `mix-blend-mode`, `backdrop-filter` or SVG filters** on viewport layers.
- **Hotlinking Unsplash, Pexels or any CDN.** Pages must not load from other sites (shell decision); store once.
- **Unedited Pexels or Pixabay assets in the bundle**, and **an Unsplash bundle** without sign-off (see table).
- **Photos with faces, text or bright small highlights** under the sidebars; anything that needs more than a 50% wash.
- **Raw mixes of a palette's `fg` as secondary ink**: Solarized, Everforest and Tokyo Night Day start well under 4.5:1.
- **Themes as CSS** that touches geometry, motion or the protected layer.
- **Hand-tuned per-wallpaper washes** (the plugin's 2.3 to 2.7:1 incident): derive them.

## Demos

| File | Size | Shows |
| --- | --- | --- |
| `docs/demos/ui-backgrounds/mesh-grain.html` | about 6 KB | slop vs calm; token mesh default; banding revealer; grain levels |
| `docs/demos/ui-backgrounds/wallpaper-ink.html` | about 6 KB | wash, ink and accent from a wallpaper with the contrast guard, both schemes |
| `docs/demos/ui-backgrounds/themes-map.html` | about 7 KB | 24 community variants mapped to tokens with the guard |

Screenshots (Chromium via Playwright on `calternal-dev-02-std`) are attached to issue #20, not in the repository.

## Open

Design choices the documents do not settle; each needs an owner answer:

- **Who renders generated looks?** (a) TypeScript canvas in the web client only: simplest, but Swift needs its own port
  and pixels will differ slightly; (b) **Rust shared client core renders once and stores a JPEG**: same pixels on every
  client, one implementation, costs a small renderer in Rust (recommended); (c) server renders on demand: adds a request
  path and a place for it to be slow.
- **Default look:** quiet token mesh (near flat, 35% wash) or livelier Dusk/Sage recipes (still 35% wash, a visibly
  richer backdrop)? Taste call.
- **Bundle photos at all?** Generated only (zero licence risk), or add the CC0 painting shelf (opt-in, needs
  tone-compression work) or Unsplash (needs sign-off on the "compile" clause)?
- **Theme and wallpaper: independent axes or a single choice?** Recommended independent (theme = palette + scheme,
  wallpaper separate) with a preset saving both; costs a second picker.
- **Who derives ink and accent: Rust core or each client?** Rust core recommended so Swift and TypeScript agree;
  costs porting `frame/color.ts` once.
- **Grain on by default?** Recommended on at 2 to 3% opacity; measure on a real phone first (Chromium headless cannot
  judge GPU cost).
- **Tokyo Night is Apache-2.0**, the rest MIT: accept shipping an Apache-licensed palette with its NOTICE, or omit it?
- **Solarized Light and Tokyo Night Day have a `fg` under 4.5:1**: allow the guard to exceed the palette's own ink, or ship them with a stated exception?
- **GPU and battery cost of the grain tile and pre-blurred layers is not measured**: requires device runs by the shell
  lane (perf VMs only for performance, per AGENTS.md); this lane did not run on perf VMs.

## Simplification opportunities

- `tokens.json` `backdrop` (nine hand-picked colours per scheme) can become *derived from `paper` and `accent`* by the
  same rule used for community themes, removing 18 stored values.
- The fixture `appearance/library.ts` paints eight palettes on a canvas at startup; replacing it with recipe files
  plus one renderer removes the bespoke blob painter and the fake Unsplash generator from client code.
- One guard function (4.5:1 by moving lightness) serves wallpapers, ink-3, accent text and semantic colours; the plugin
  has three separate hand-tuned places (`washFor`, accent, pill contrast).
