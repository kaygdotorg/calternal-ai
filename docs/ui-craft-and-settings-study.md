# Craft's appearance system and Apple's settings windows

Lane `ui-craft-settings`, issue #22, 2026-11-17. Claude research lane, documents only. Terms: [glossary](glossary.md).
Nothing here is decided: sections 5 and 12 are proposals and open questions for the owner. Evidence images are
attachments on issue #22 (https://git.kayg.org/kayg/calternal-ai/issues/22), linked where used. No image or
third-party file was committed; the eight Unsplash photos were fetched into `/tmp` for analysis only.

## 1. How to read this

**The short version.**

1. Craft keeps text readable by construction, not by measurement. The page is an opaque card, the web sidebar is
   92% opaque, and the only adaptive step in the code is one yes-or-no test on the backdrop's colour that flips
   the header's text colour. There is no function from background contrast to blur, dim or tint. The measured
   Craft numbers are in section 3.
2. A Calternal function that does adapt (section 6) is small: it solves one number per surface, the opacity of a
   veil, so that the primary ink reaches 7:1 on the worst 1% of the surface, then moves ink-2 and ink-3 toward the
   primary ink for that surface until they reach 5.5:1 and 4.5:1. Blur, Dim and Tint choose the look; the readability
   floor is always applied.
3. The eight chosen wallpapers are mostly dark photographs. In the light scheme they need a sidebar veil of 0.51 to 0.71 (0.66 median) before dark ink reaches 7:1 on the worst 1% of pixels, and 0.51 to 0.79 in the dark scheme. If the target is the faintest ink at 4.5:1 instead, the light-scheme veil is 0.74 to 0.88 and the wallpaper is nearly gone (section 6.3). **Which ink sets the
   floor is the most consequential open question.**
4. The shell lane's `washFor` promises 4.5:1 for ink-3 but delivers 3.0 to 3.8:1 in the light scheme, because its
   closed form mixes in linear light while CSS composites in gamma-encoded sRGB (section 8).
5. Apple's good settings windows (System Settings, Xcode) share one pattern: a sidebar of sections, inset rounded groups, a label on the left and one control on the right, help text under the label, no Save button; only System Settings has a
   search field. The bad ones (Notes, Reminders) are one long page of
   mixed controls with sentence-long labels (section 10). Section 11 turns this into Calternal's Settings.

**Trust levels.** Every number says how it was obtained.

| Tag | Meaning |
| --- | --- |
| DOM | Computed style or layout read from a running page (Playwright, Chromium 1255, 1440 by 900, `prefers-color-scheme` set to each scheme) |
| CODE | Read from Craft's public shared-page JavaScript bundle (`share/static/js/main.248864bd.js`, served to every visitor, fetched 2026-11-17) |
| AX | Accessibility frame in points from a macOS app (the VM is 1x: 1 pt = 1 px, so about ±1 pt) |
| px | Measured from a screenshot |
| calc | Computed by the reference function in section 6 on real pixels (Chromium-compatible gamma-space compositing) |
| earlier | Measured by an earlier study in the existing app's repo, not re-measured here |

## 2. What could and could not be opened

| Thing | Result |
| --- | --- |
| Craft for Mac 3.7.1 (direct download `res.craft.do/native/sparkle/Craft.dmg`, notarized) | Installed on `calternal-macos-01-std`; it opens only to "Enter your email to get started" ([screenshot](https://git.kayg.org/attachments/38e4b9e5-a759-4c4a-89b7-1b8a9b246904)). It needs an account. **Not signed in**: no account was created, and no owner identity was used. Craft was quit; the app and the dmg are still on the VM and are removed in section 13. |
| Craft web app (`docs.craft.do`) | Not opened (documents need a sign-in; only the native app's own sign-in screen was tried). |
| Craft **public shared documents and the getting-started guide** | Opened in Chromium on `calternal-dev-02-std`, in light and in dark. These render real Craft pages with real backdrops, and their JavaScript is the real viewer. This is the source of every DOM and CODE number. It is the **viewer**, not the editor: the Style panel, the gallery and the contrast guard live in the editor and were not available. |
| Style gallery, Style panel, Settings > Appearance & Sound | Not re-opened. Their facts come from the earlier native-app passes (2026-11-09 and 2026-11-11, Mac Studio) and are tagged `earlier`. |
| Eight Unsplash photos | The Unsplash site returns 401 to scripted requests from the dev VM and from `curl`; the page's `images.unsplash.com` URL was read through Firecrawl and fetched at width 1600, quality 60, for analysis only. |
| System Settings, Safari, Finder, Terminal, Notes, Reminders | Opened on the macOS VM (26.5.2) in light and dark; AX frames and screenshots. |
| **Mail Settings** | **Not opened.** Mail starts with an "Add Account" window that blocks its menu, and the harness could not dismiss it without an account. No account was added. Mail is therefore absent from the measured sections; its layout is not described. |
| Xcode Settings | Xcode is not on the VM, so it was opened on the Mac Studio (`kayg@std`, Xcode 27.2) with `cua-driver`: launch, open Settings, click each of the 12 sidebar rows to read it, close the window, quit Xcode. Nothing else was touched; std's own appearance (dark) was not changed. |

## 3. Part A, Craft: what the report missed

### 3.1 What the earlier studies established (`earlier`)

From the existing app's repo (`craft-styles-study-2026-11-09.md`, `craft-immersive-faded-2026-11-10.md`,
`project-craft-styles-gui-pass.md`): 131 styles in one flat searchable gallery of 3 columns; paper is opaque, no
grain, card radius 9 to 10 pt, 760 pt page (wide: about 1180), text inset 34 pt; 17 of 131 styles are *dynamic*
and flip light and dark by swapping paper and text while the backdrop stays; the app takes light or dark from the
document while "Apply Document Style to the App" is on; one style change is a 280 to 300 ms cross-fade of the whole
window; the **native** sidebar in Immersive is `(1 - a) P + a N` with the backdrop's colour weighted 0.225 (light)
or 0.27 (dark), and Faded blends it down the window toward a neutral.

### 3.2 What they missed

| # | Gap | What this study found |
| --- | --- | --- |
| 1 | They looked at the native sidebar but not at the *rest* of how readability is kept. | Readability comes from **opacity**, not adaptation. The shared page's sidebar is `rgba(255,255,255,0.92)` over a 20 px blur in light and `rgba(41,41,41,0.92)` in dark (DOM and CODE, identical across five different backdrops). Of the backdrop, 8% shows. The page card is opaque `#FFFFFF` or `#222222` (DOM). The modal scrim is `rgba(36,39,41,0.20)` light and `0.25` dark (DOM). |
| 2 | "What does it change automatically?" was answered as "a fixed tint weight". | There **is** one adaptive step, in the header: Craft takes one colour for the backdrop (a solid colour; the gradient's start colour; an image's BlurHash average, so a single precomputed colour), asks `0.299 R + 0.587 G + 0.114 B > 145` on the gamma-encoded bytes (CODE, module 30884 `GM`), and picks the header's text colour from the scheme pair `#1F2225` / `#F4F4F4` accordingly. The header bar is that backdrop colour at 0.80 over `blur(10px)` (DOM), and a 6% (dark backdrop) or 3% (light backdrop) wash of the text colour is added (CODE: `text + "0F"` or `"08"`). It is a threshold, not a curve: no amount scales with the contrast. |
| 3 | Immersive and Faded were read from pixels only. | In the viewer's code the background has `size: "full" | "faded"`. Faded is one overlay: `position: fixed; height: 80vh; bottom: 0; background: linear-gradient(to top, <page colour>, <page colour at alpha 0>)`, where the page colour is `#FFFFFF` (light) or `#222222` (dark) (CODE). The picture itself is not changed. The native app's Faded (`earlier`) blends the *sidebar* linearly over the whole window height instead, so the two are different mechanisms with the same name. A blurred image is `filter: blur(10px)` on a layer scaled 1.03 (CODE), and a solid backdrop with no picture is `#F7F7F7` (light) or `#1A1A1A` (dark) (DOM). |
| 4 | The preset's contents were listed from the UI. | The style record the viewer reads is `{styleId, textColor, background, paperBackgroundColor, paragraphSpacingType, themeColor, themeId, lineThemeStyle, cardThemeStyle, pageWidthType}`, with `background = {type: none \| solid \| gradient \| image, size: full \| faded, solidColor, gradientColors{start, end, direction: topToBottom \| leftToRight \| radial}, image{url, isBlurred, blurHash}}` and a per-style `dynamic \| fix` flag (CODE). A preset is **data**, 10 fields, and every field has an override. |
| 5 | Dark mode of the *app chrome* over a styled page. | In the viewer the app's light or dark follows the system; the page's own colours do not change with it unless the style is dynamic. Same document, dark scheme: the sidebar goes from white to `#292929` at 0.92, the page from `#FFFFFF` to `#222222`, and the header colour and backdrop **do not change at all** (DOM, `headerBackground` is `rgb(130,181,252)` at 0.8 in both schemes on the same document). |
| 6 | Text over the *sidebar* in the native app. | Not guarded by the app; guarded by the 0.775 or 0.73 neutral share (immersive) and by Craft's choice of ink. With the same ink as Calternal's ink-3, Craft's native sidebar gives 1.9 to 4.9:1 on the eight wallpapers (section 8), so it relies on ink choice and on typical backdrops being light flat colours. |
| 7 | The contrast guard. | Only the editor has it (`earlier`: a warning triangle that snaps text to the nearest readable grey, about 4.1 to 4.5:1). The viewer's code has no contrast function (a search found none); `GM` is the only luminance test and it is a threshold. |

### 3.3 Measured values (light and dark)

DOM on the "Template Book" shared document (blue and purple photographic backdrop), 1440 by 900. Screenshots:
[light](https://git.kayg.org/attachments/11ab8b1e-3063-4ffe-9601-e3b1c70a4197),
[dark](https://git.kayg.org/attachments/b4c668c0-1625-4fb7-870b-5437750f0edf) (same document, `prefers-color-scheme` only),
and the getting-started guide's "Styling Documents" page
([light](https://git.kayg.org/attachments/3a3399c3-723f-458b-9a10-c659b3f14045),
[dark](https://git.kayg.org/attachments/989820e6-9aab-4ba1-a13d-b6ae0f9710a8)).

| Surface | Light | Dark | Source |
| --- | --- | --- | --- |
| Left sidebar (280 wide) | `rgba(255,255,255,0.92)`, `blur(20px)` | `rgba(41,41,41,0.92)`, `blur(20px)` | DOM, CODE |
| Same, opaque fallback when `backdrop-filter` is missing | `rgba(255,255,255,1)` | `rgba(41,41,41,1)` | CODE |
| Page card | `#FFFFFF` (dynamic styles: `#F7F3F1` etc. as authored) | `#222222` | DOM |
| Page layer behind the backdrop when there is none | `#F7F7F7` | `#1A1A1A` | DOM |
| Header bar, 46 px | backdrop colour at 0.80, `blur(10px)` | same colour, same alpha | DOM |
| Header text | `#1F2225` on light backdrops, `#F4F4F4` on dark ones (threshold 145) | same rule | CODE |
| Header wash on top | 3% of the text colour | 6% | CODE |
| Modal scrim | `rgba(36,39,41,0.20)` | `rgba(36,39,41,0.25)` | DOM |
| Soft text colour | `#4C555D` | `#F0F0F0` | CODE |
| Accent pair | `#0087FF` | `#2EC8FF` | CODE |
| Style change | cross-fade of everything, about 280 to 300 ms | same | earlier |

Craft's native sidebar (`earlier`, light / dark): backdrop weight 0.225 / 0.27 over neutral `#E9E9E9` / `#454545`.
Its chroma is a fixed share of the backdrop's: measured here from the earlier table, the sidebar's OKLCH chroma is
0.23 to 0.26 of the backdrop's in light and 0.32 to 0.35 in dark (four flat colours), so the "tint" is
proportional, not adaptive. That is where the tint gain in section 6 comes from.

### 3.4 Presets and gallery: why it feels curated

- **Organisation.** One flat grid under one header, searchable, no categories (`earlier`, 131 styles). The curation is
  in the *names* (1 to 3 words), the *pairing* (each style's backdrop, paper, text, font and separator were
  chosen together) and the *preview* (every tile is the style applied to a miniature page, not a swatch).
- **Bundle.** The 10 fields above. A style also decides card or no card (16 of 131 are page-only), whether it
  carries a cover (31 of 131), and the separator (Doodle 57, Washi 39, Line 35) (`earlier`).
- **Per-part overrides.** After picking a style each part stays editable (backdrop, document colour, text colour,
  cover, separator, font, wide page); the gallery tile is then "Custom Style" (the guide's screenshot shows it).
  Dynamic styles flip paper and text and keep the backdrop.
- **What makes it feel curated.** A small number of decisions made once by people, then applied everywhere: opaque
  paper, 12-slot swatch rows that are the Tailwind 50 and 900 families, four system font designs and no
  uploads, no grain, no shadows to speak of. There is no user-facing "contrast", "blur radius" or "opacity".

### 3.5 User-facing controls, counted

Counts are from the earlier verified pass, the viewer code and the Settings page; the native Style panel was not
re-opened, so treat them as ±2.

| Kind | Count | Which |
| --- | --- | --- |
| Toggles | 4 | Blur Image, Wide Page, Apply Document Style to the App, Reduce Animations |
| Pickers, menus and segmented controls | about 18 | Style gallery; Backdrop mode (none, solid, gradient, image); solid swatch row (12); gradient preset row (12); Start colour; End colour; Direction menu; Effect (Immersive or Faded); Image chooser (6, Shuffle, Unsplash, Upload); Document Color (12); Text Color (12); Cover image and its crop-ratio menu; Separator style and washi pattern; Font (4); App appearance (System, Light, Dark); Interface accent (default and 10) |
| Sliders | **0** | None in the Style panel or Settings. The only sliders are the hue, saturation and brightness sliders inside the custom colour dialog |
| Free text | 0 | Search in the gallery |

## 4. What Calternal decided

From `docs/settings.md` on `origin/dev`: Materials are three checkboxes, **Blur**, **Dim** and **Tint**, with no
sliders, and the values are chosen automatically from the contrast of the selected background so everything stays
readable in light and dark. Wallpaper is per person; a preset sets everything at once; one **UI size** slider
exists; the accent is inferred from the wallpaper. Craft's own count (4 toggles, about 18 pickers, 0 sliders) is
the same shape: choose visually, never type a number.

Apple's current settings window is the same idea at system level (section 10.3): **Appearance** (Auto, Light,
Dark), **Liquid Glass: Clear or Tinted**, and a switch "Tint window background with wallpaper color", all
checkbox-like choices without a value. That is a good precedent for the owner's three boxes.

## 5. The function (proposal)

### 5.1 Meaning of the three boxes (to be confirmed, section 12)

Two layers sit over the blurred wallpaper on each surface, then the surface's ink.

| Box | On | Off |
| --- | --- | --- |
| **Blur** | The surface shows a pre-blurred copy: 12 px under sidebars, 75 px under the card, saturation 1.2 and 1.4 baked in (the owner's plugin and the shell tokens) | The sharp wallpaper |
| **Tint** | A veil in the scheme's paper colour carrying the wallpaper's hue (OKLCH chroma set by the rule below), at 0.35 under sidebars and 0.85 under the card (the plugin's tint strengths) | No hue layer |
| **Dim** | A further neutral veil in the scheme's paper colour at 0.25 under sidebars and 0.60 under the card | No dim layer |

Whatever the boxes say, a **readability floor** is then applied: if the composite does not meet the contrast target,
the dim layer's opacity (or the tint layer's, when Dim is off) is raised until it does, never above 0.9. So
unchecking a box never makes text unreadable; it removes only what readability does not need.

### 5.2 Inputs

| Input | Definition |
| --- | --- |
| `scheme` | light or dark, from the user's setting or the system |
| `role` | `side` (left or right sidebar) or `card` (middle card); pills sit on the card and inherit its composite |
| `flags` | `blur`, `dim`, `tint` booleans |
| `px` | the pixels under the surface of the copy that will be shown (blurred and saturated if Blur is on, sharp if off), cover-fitted to a 1440 by 900 reference viewport and sampled at half scale |
| `mean` | the mean colour of the unblurred wallpaper in that region, converted to OKLCH |
| tokens | primary ink, ink-2, ink-3, paper per scheme: light `#262626 #4A4A4A #5F5F5F` on `#F7F7F7`; dark `#EBEBEB #C4C4C4 #A6A6A6` on `#1F1F1F` (`lane/ui-shell` `tokens.json`; not yet on `dev`) |

"Background luminance statistics" are the 1st and 99th percentiles of the relative luminance of `px` (what the
tables print), but the solver uses the pixels themselves, so the result is exact for the composite, not an
estimate from two numbers.

### 5.3 Outputs

| Output | How |
| --- | --- |
| `blurPx` | `blur` ? 12 (sidebars) or 75 (card) : 0. Constant per role; it is a look, not a readability tool (a 12 px blur moves the sidebar floor by at most 0.06, section 7.2). Phones scale by 0.7 (existing token). |
| `tint` | `{alpha: flags.tint ? 0.35 or 0.85 : 0, colour: paper with OKLCH chroma c at the mean hue}` where `c = min(g * mean chroma, 0.05)`, gain `g = 0.25` light and `0.33` dark (Craft's own measured ratio), reduced until it fits sRGB at the paper's lightness |
| `dim` | `{alpha: flags.dim ? 0.25 or 0.60 : 0, colour: paper}`, then raised by the floor |
| `veil total` | `1 - (1 - a_tint)(1 - a_dim)`; the raised layer is capped at 0.9, so a total can reach 0.935 |
| `ink`, `ink2`, `ink3` | the tokens; then for this surface only, `ink2` and `ink3` are moved toward `ink` by the smallest `t` that brings their ratio to 5.5 and 4.5 on the worst 1% of pixels |
| icon colour | `ink3`'s per-surface value (so icons are at least 4.5:1, above the 3:1 WCAG asks for icons) |
| ink flip | none: ink always follows the scheme (see 12, question 1 for the alternative) |

**Targets.** Primary ink 7:1, ink-2 5.5:1, ink-3 4.5:1, each on the **worst 1%** of the surface (percentile 0.01 of the
per-pixel ratio). The achieved minimum single pixel is reported separately.

**The function** (reference implementation, TypeScript; the product version is Rust in the shared client core, same
steps). Compositing is in gamma-encoded sRGB, as CSS does it.

```ts
type RGB = [number, number, number];            // 0..255, gamma-encoded
const lin = (v: number) => (v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
const lum = ([r, g, b]: RGB) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
const ratio = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
const over = (w: RGB, v: RGB, a: number): RGB => w.map((x, i) => (1 - a) * x + a * v[i]) as RGB;
const p01 = (xs: number[]) => [...xs].sort((a, b) => a - b)[Math.floor(0.01 * (xs.length - 1))];

const TINT_A = { side: 0.35, card: 0.85 }, DIM_A = { side: 0.25, card: 0.60 };
const GAIN = { light: 0.25, dark: 0.33 }, C_CAP = 0.05, A_MAX = 0.9;
const TARGET = { ink: 7, ink2: 5.5, ink3: 4.5 };

export function material(scheme: Scheme, role: Role, flags: Flags, px: RGB[], meanOklch: {c: number; h: number}): Material {
  const T = TOKENS[scheme];
  const tintRgb = paperWithChroma(T.paper, Math.min(GAIN[scheme] * meanOklch.c, C_CAP), meanOklch.h); // fits sRGB
  let aT = flags.tint ? TINT_A[role] : 0, aD = flags.dim ? DIM_A[role] : 0;
  const composite = (w: RGB, aT: number, aD: number) => over(over(w, tintRgb, aT), T.paper, aD);
  const worst = (aT: number, aD: number) => p01(px.map(w => ratio(lum(composite(w, aT, aD)), lum(T.ink))));
  if (worst(aT, aD) < TARGET.ink) {                    // readability floor
    const raiseDim = flags.dim || !flags.tint;
    let lo = raiseDim ? aD : aT, hi = A_MAX;
    const ok = (a: number) => worst(raiseDim ? aT : a, raiseDim ? a : aD) >= TARGET.ink;
    if (!ok(hi)) lo = hi; else for (let i = 0; i < 22; i++) { const m = (lo + hi) / 2; ok(m) ? (hi = m) : (lo = m); }
    raiseDim ? (aD = hi) : (aT = hi);
  }
  const ground = px.map(w => composite(w, aT, aD));
  const toward = (tok: RGB, target: number) => bisect01(t => p01(ground.map(g => ratio(lum(g), lum(mix(tok, T.ink, t))))) >= target);
  return { blurPx: flags.blur ? BLUR[role] : 0, tint: { a: aT, rgb: tintRgb }, dim: { a: aD, rgb: T.paper },
           ink: T.ink, ink2: mix(T.ink2, T.ink, toward(T.ink2, TARGET.ink2)), ink3: mix(T.ink3, T.ink, toward(T.ink3, TARGET.ink3)) };
}
```

**Cost** (measured, JS on `calternal-dev-02-std`, sort-based percentile): 33 ms per call for 3,390 pixels (a sidebar at
1/4 scale) and 137 ms for 11,978 (the card at 1/4). It runs once per (wallpaper, scheme, flags, window size) change
in a worker, never per frame; the Rust version with a 256-bin histogram of the composite is linear and well under
a millisecond per region. Reads: one decode of the pre-blurred copies the shell already makes. Flushes: none.

## 6. Results on the eight default wallpapers

Method: each photo (width 1600) cover-fitted to 1440 by 900 (computed at 720 by 450), regions are the left sidebar
(x 0 to 240), the card (x 248 to 1092) and the right sidebar (x 1100 to 1440), blur and saturation baked as in 5.2,
all three boxes on. "p01" is the 1st-percentile ratio over about 13,500 (sidebar) or 49,000 (card) pixels; "worst
pixel" is the minimum. Ratios are WCAG 2 contrast of the composited ground against the ink, rendered per pixel, so
they are independent of the solver's percentile.

### 6.1 All three boxes on (the default)

Photographers and IDs from `docs/settings.md`; the names in the first column are short labels for the analysed
images.

| Photo (ID) | Scheme | Wallpaper luminance under the left sidebar, p01 / p99 | Left sidebar: tint + dim = veil | Ink p01 / worst pixel | Ink-2 / ink-3 for this surface (p01 ratio) | Right sidebar veil | Card veil | Card ink p01 |
|---|---|---|---|---|---|---|---|---|
| Spratt peak (`fbV03MdVtJA`) | light | 0.023 / 0.384 | 0.35 + 0.47 = **0.66** | 7 / 7 | #373737 (5.5) / #444444 (4.5) | 0.66 | 0.94 | 12.67 |
| Spratt peak (`fbV03MdVtJA`) | dark | 0.023 / 0.384 | 0.35 + 0.47 = **0.66** | 7 / 6.23 | #d2d2d2 (5.5) / #bebebe (4.5) | 0.63 | 0.94 | 12.5 |
| Barrow streaks (`BnOdWOMW7Is`) | light | 0.218 / 0.964 | 0.35 + 0.25 = **0.51** | 8.08 / 6.48 | #404040 (5.5) / #4d4d4d (4.5) | 0.67 | 0.94 | 12.57 |
| Barrow streaks (`BnOdWOMW7Is`) | dark | 0.218 / 0.964 | 0.35 + 0.67 = **0.79** | 7 / 7 | #d2d2d2 (5.5) / #bebebe (4.5) | 0.51 | 0.94 | 11.86 |
| Hoefler painting (`Nkwrpa_v0WA`) | light | 0.077 / 0.62 | 0.35 + 0.38 = **0.59** | 7 / 6.71 | #373737 (5.5) / #444444 (4.5) | 0.59 | 0.94 | 12.93 |
| Hoefler painting (`Nkwrpa_v0WA`) | dark | 0.077 / 0.62 | 0.35 + 0.59 = **0.73** | 7 / 6.43 | #d2d2d2 (5.5) / #bebebe (4.5) | 0.68 | 0.94 | 12.57 |
| Pan'kiv ivy door (`B8zfbse7mOc`) | light | 0.012 / 0.127 | 0.35 + 0.50 = **0.68** | 7 / 6.94 | #373737 (5.5) / #444444 (4.5) | 0.68 | 0.94 | 12.62 |
| Pan'kiv ivy door (`B8zfbse7mOc`) | dark | 0.012 / 0.127 | 0.35 + 0.25 = **0.51** | 8.68 / 7.4 | #c4c4c4 (5.93) / #ababab (4.5) | 0.51 | 0.94 | 13.3 |
| Yu sun branches (`_zDcx4dD7L0`) | light | 0.001 / 0.006 | 0.35 + 0.56 = **0.71** | 7 / 6.98 | #373737 (5.5) / #444444 (4.5) | 0.71 | 0.94 | 12.43 |
| Yu sun branches (`_zDcx4dD7L0`) | dark | 0.001 / 0.006 | 0.35 + 0.25 = **0.51** | 14.78 / 14.1 | #c4c4c4 (10.1) / #a6a6a6 (7.24) | 0.51 | 0.94 | 12.21 |
| Giqueaux leaves (`EX_STserBZM`) | light | 0.021 / 0.421 | 0.35 + 0.48 = **0.66** | 7 / 7 | #373737 (5.5) / #444444 (4.5) | 0.66 | 0.94 | 12.65 |
| Giqueaux leaves (`EX_STserBZM`) | dark | 0.021 / 0.421 | 0.35 + 0.49 = **0.67** | 7 / 5.96 | #d2d2d2 (5.5) / #bebebe (4.5) | 0.51 | 0.94 | 12.93 |
| Giqueaux motion (`ngWVYP9NIYM`) | light | 0.008 / 0.464 | 0.35 + 0.52 = **0.69** | 7 / 6.85 | #373737 (5.5) / #444444 (4.5) | 0.70 | 0.94 | 12.49 |
| Giqueaux motion (`ngWVYP9NIYM`) | dark | 0.008 / 0.464 | 0.35 + 0.52 = **0.69** | 7 / 6.95 | #d2d2d2 (5.5) / #bebebe (4.5) | 0.62 | 0.94 | 12.98 |
| Melnychuk clouds (`I_xBR4NmDhU`) | light | 0.06 / 0.193 | 0.35 + 0.39 = **0.60** | 7 / 6.88 | #373737 (5.5) / #444444 (4.5) | 0.51 | 0.94 | 12.98 |
| Melnychuk clouds (`I_xBR4NmDhU`) | dark | 0.06 / 0.193 | 0.35 + 0.25 = **0.51** | 7.32 / 7.11 | #cdcdcd (5.5) / #bababa (4.5) | 0.72 | 0.94 | 12.42 |

Reading it:

- **Minimum veil** is 0.51 because the floor is `0.35 + 0.25` (Tint and Dim defaults) and the image needed less.
  The veil reaches 0.79 (Barrow in dark, whose brightest streaks reach luminance 0.96) and 0.71 in light (Yu).
- **Worst pixel** is within 0.52 of the 7:1 target in 13 of 16 rows and within 1.04 in the rest (5.96 for Giqueaux leaves and 6.23 for Spratt, both dark, where a few bright pixels survive the blur); p01 is exact because the solver targets it. Guaranteeing the single worst pixel needs a larger veil; the percentile is a parameter (12, question 3).
- **Card** is 0.94 everywhere: it is the default (0.85 tint then 0.60 dim), not the floor; section 6.2 shows the
  card's real floor is 0.56 to 0.79.
- Ink-3 for a surface is darker than the token in light (`#444444` against `#5F5F5F`) and lighter in dark (`#BEBEBE`
  against `#A6A6A6`) exactly where the wallpaper is hard; where it is easy (Yu, dark) the token is unchanged.
- The chroma of the wallpaper's mean is 0.004 (grey Barrow) to 0.11 (Hoefler, red-brown) before the gain; after the gain and cap the veil's chroma is 0.001 to 0.037, so the hue is a whisper: Hoefler is the most coloured at 0.028 (light) and 0.037 (dark).

### 6.2 What the boxes change (the floor)

All boxes off: only the readability floor, no tint, no dim, sharp wallpaper. Left sidebar, right sidebar and card veil
in each scheme, then the left sidebar with only Blur off.

| Photo | Scheme | Left sidebar floor (all boxes off) | Right sidebar | Card | Blur off, others on (left sidebar) | all on (left sidebar) |
|---|---|---|---|---|---|---|
| Spratt peak | light | 0.66 | 0.67 | 0.67 | 0.66 | 0.66 |
| Spratt peak | dark | 0.72 | 0.72 | 0.73 | 0.72 | 0.66 |
| Barrow streaks | light | 0.44 | 0.68 | 0.68 | 0.51 | 0.51 |
| Barrow streaks | dark | 0.79 | 0.00 | 0.78 | 0.79 | 0.79 |
| Hoefler painting | light | 0.59 | 0.59 | 0.64 | 0.59 | 0.59 |
| Hoefler painting | dark | 0.76 | 0.71 | 0.73 | 0.76 | 0.73 |
| Pan'kiv ivy door | light | 0.68 | 0.68 | 0.68 | 0.68 | 0.68 |
| Pan'kiv ivy door | dark | 0.49 | 0.45 | 0.57 | 0.51 | 0.51 |
| Yu sun branches | light | 0.71 | 0.71 | 0.71 | 0.71 | 0.71 |
| Yu sun branches | dark | 0.00 | 0.00 | 0.79 | 0.51 | 0.51 |
| Giqueaux leaves | light | 0.67 | 0.67 | 0.67 | 0.67 | 0.66 |
| Giqueaux leaves | dark | 0.71 | 0.56 | 0.76 | 0.71 | 0.67 |
| Giqueaux motion | light | 0.69 | 0.70 | 0.70 | 0.69 | 0.69 |
| Giqueaux motion | dark | 0.69 | 0.63 | 0.66 | 0.69 | 0.69 |
| Melnychuk clouds | light | 0.60 | 0.25 | 0.56 | 0.60 | 0.60 |
| Melnychuk clouds | dark | 0.49 | 0.72 | 0.70 | 0.51 | 0.51 |

Findings:

1. **For these wallpapers the checkboxes mostly change little on the sidebars.** The readability floor is 0.44 to 0.79 (and 0.00 for Yu in dark and for Barrow's right strip in dark), which is at or above what Tint and Dim add on the sidebars (0.51). Unchecking Dim or Tint changes the left sidebar by under 0.02 in 24 of 32 checks and by 0.03 to 0.26 in the other 8 (Barrow light; Pan'kiv, Yu and Melnychuk dark). They matter most on the card: unchecking Dim lowers its veil by 0.09, Tint by 0.17 to 0.34, both by 0.15 to 0.38, from a 0.94 default toward the 0.56 to 0.79 floor.
2. **Blur is a look, not a readability tool.** At 12 px it moves the sidebar's floor by 0.00 to 0.06.
3. Only the card's floor (0.56 to 0.79) is lower than what the owner's plugin defaults apply (0.94): a real choice
   exists there between the plugin's default and the floor (12, question 4).

### 6.3 Sensitivity to the target ink (the question that decides the look)

Left sidebar veil needed by the readability floor alone (no tint, no dim) on the 12 px blurred copy, for four targets, p01, shown as light / dark (the same pixels). The "primary ink at 7" column is the proposal's target and is 6.2's floor recomputed on the blurred copy, so it differs from 6.2's sharp-copy column by up to 0.06:

| Photo | Primary ink at 4.5 | Primary ink at 7 | Ink-2 at 4.5 | Ink-3 at 4.5 |
| --- | --- | --- | --- | --- |
| Spratt peak | 0.48 / 0.44 | 0.66 / 0.65 | 0.70 / 0.63 | 0.85 / 0.78 |
| Barrow streaks | 0.10 / 0.66 | 0.40 / 0.79 | 0.47 / 0.77 | 0.74 / 0.87 |
| Hoefler painting | 0.38 / 0.57 | 0.59 / 0.73 | 0.64 / 0.71 | 0.83 / 0.83 |
| Pan'kiv ivy door | 0.51 / 0.00 | 0.68 / 0.32 | 0.71 / 0.26 | 0.86 / 0.56 |
| Yu sun branches | 0.56 / 0.00 | 0.71 / 0.00 | 0.74 / 0.00 | 0.88 / 0.00 |
| Giqueaux leaves | 0.48 / 0.47 | 0.66 / 0.67 | 0.70 / 0.64 | 0.85 / 0.79 |
| Giqueaux motion | 0.52 / 0.50 | 0.69 / 0.69 | 0.72 / 0.66 | 0.86 / 0.80 |
| Melnychuk clouds | 0.40 / 0.17 | 0.60 / 0.48 | 0.65 / 0.44 | 0.83 / 0.67 |

Sizing the veil by **ink-3 at 4.5** (the shell lane's rule) makes the sidebar 0.67 to 0.88 opaque in nearly every
row: the wallpaper survives as a faint wash. Sizing by **primary ink at 7** and then deriving ink-2 and ink-3 per
surface keeps 21 to 49% of the wallpaper visible while every text colour still reaches its WCAG-AA target on 99% of
the surface. Section 5 uses the second; the first is one parameter away.

### 6.4 Which scheme suits which wallpaper

The floor tells you which scheme a wallpaper is for. Left sidebar floor, light against dark: Yu sun branches 0.71 / 0
and Pan'kiv ivy door 0.68 / 0.49 want dark; Barrow streaks 0.44 / 0.79 and Melnychuk clouds 0.60 / 0.49 are balanced
or lean light; Spratt peak (0.66 / 0.72), Giqueaux leaves and motion are heavy in both. The default set is
therefore mostly *dark-mode photographs*. Two ways to avoid a nearly opaque light-scheme sidebar, both outside the
checkbox model (12, question 1): a light and a dark rendition of each wallpaper (the backgrounds lane's
recommendation), or surfaces that flip their ink as Craft's header does and Apple's Liquid Glass does.
Measured effect of flipping on the left sidebar (veil needed by the *other* scheme's ink at ink-3, 4.5, p01): light scheme with flip: 0.78, 0.87, 0.83, 0.56, 0.00, 0.79, 0.80, 0.67 against 0.85, 0.74, 0.83, 0.86, 0.88, 0.85, 0.86,
0.83 without (listed in photo order); flipping helps six of eight (saving 0.06 to 0.88), most for Yu (0.88 to 0) and Pan'kiv (0.86 to 0.56), and costs Barrow and Hoefler 0.12 and 0.01 more.

## 7. The same pixels under other recipes

Same regions and blur as 6.1 where the recipe has one (Craft Mac: blurred hard, sigma 40 px; Craft web: 20 px; plugin:
12 px with saturation 1.2). The ink for every recipe is Calternal's tokens, so the table compares only the veil.
p01 ratio of primary ink (ink-3 in brackets). Models are mine from the CSS and code quoted in each row.

| Recipe | What it applies | Light: primary ink p01, min to max over 8 (ink-3) | Dark: primary ink p01, min to max over 8 (ink-3) | Wallpaper visible |
| --- | --- | --- | --- | --- |
| **Proposed** (section 5) | veil by solver | 7.0 to 8.1 (4.5 by construction through the derived colour) | 7.0 to 14.8 | 21 to 49% |
| Craft Mac, Immersive (`earlier`) | 0.225 backdrop + 0.775 `#E9E9E9` light; 0.27 + 0.73 `#454545` dark | 7.4 to 10.1 (3.1 to 4.3) | **3.8** to 10.1 (1.9 to 4.9); 1 of 8 below 4.5 | 22.5% light, 27% dark |
| Craft web sidebar | 0.92 `#FFFFFF` / `#292929`, blur 20 | 12.7 to 13.9 (5.4 to 5.9) | 9.6 to 12.5 (4.7 to 6.1) | 8% |
| bb plugin sidebar | 0.5 side colour then, in light, 0.5 white; no dim | 8.1 to 11.2 (3.4 to 4.7) | **2.8** to 14.8 (1.4 to 7.3); 2 of 8 below 4.5 | 25% light, 50% dark |
| Shell `washFor` | white or black veil, alpha from its closed form | 7.0 to 9.0 (**3.0 to 3.8**) | 11.1 to 16.4 (5.4 to 8.1) | 10 to 65% |

(Plugin model assumes the sidebar colour is the shell's paper token, because bb's own `--sidebar` is the host theme's.
The card in the plugin, with its 0.75 tint and 0.65 or 0.30 dim, gives primary ink 9.7 to 13.8 everywhere.)

Reading it. Craft's recipes are safe because they are *opaque* (Craft web 0.92) or because their light ink is
dark on a pale neutral; the Mac recipe is an Apple-style tint and fails for dark photographs. The owner's plugin
needed hand tuning ("measured 2.3 to 2.7:1", per its comments) and in this model still fails the dark scheme on two of the eight (Barrow streaks 2.8:1, Hoefler painting 3.8:1); Craft Mac fails one (Barrow, 3.8:1). The proposed solver is the only one that meets its targets on all eight wallpapers in both
schemes, at a visibility (21 to 49%) between Craft Mac and Craft web.

## 8. The owner's bb plugin and the shell's `washFor`

Read: `frame/pill-contrast.ts`, `color.ts`, `wallpaper.ts`, `preblur.ts`, and on `lane/ui-shell`
`packages/ui/src/lib/material.ts`.

| Aspect | bb plugin | Shell lane | Proposal |
| --- | --- | --- | --- |
| What is read from the wallpaper | one 48 by 48 decode: mean colour and the most vivid hue (OKLCH, scored by count times chroma); a 1/16-viewport luminance grid for pills (pre-blurred copy, 3 px extra blur) | a 24 by 14 luminance grid read once from the blurred copy; a 48 by 48 colour read | the displayed (pre-blurred) copy at half scale under each surface, read once |
| Statistic | darkest and brightest colours in a pill's rectangle (min and max) | darkest or brightest grid cell | 1st percentile of per-pixel ratio |
| Adaptive output | `--cf-pill-wash` (0 to 1 in 0.1 steps, cap 0.35); the sidebar wash was hand-tuned (0.5 white in light) | a wash alpha that makes **ink-3** reach 4.5:1, clamped 0.35 to 0.9 | veil opacity for primary ink 7:1; ink-2, ink-3 derived per surface |
| Composition model | **gamma sRGB, in the browser's order** (backdrop, blur, saturate, brightness, then fill) | **linear light closed form** (`washFor`) | gamma sRGB, solved numerically |
| Ink | fixed tokens | fixed tokens | fixed tokens, with per-surface ink-2 and ink-3 |
| Cost | rescans throttled to 500 ms, no per-scroll work | one pass at selection | one pass at selection |

**Finding (reproducible from the pixels).** `washFor` solves for the wash alpha as if opacity mixed *relative luminance* linearly (`(need - min) / (1 - min)`). A browser mixes the encoded sRGB values, so a white wash over a dark pixel lifts its luminance less than the formula assumes. In the light scheme the closed form therefore
under-delivers: it asks for the ink-3 ratio 4.5 and measured p01 is **3.0 to 3.8** on all eight wallpapers (table in
section 7, last row); in the dark scheme it over-delivers (5.4 to 8.1). The plugin's pill code, by contrast, composes
in gamma space and its comments already say why. I did not run the shell lane's own e2e check (`e2e/wallpapers.mjs`), so whether it catches this is untested; the model above is `washFor`'s alpha applied as a single wash over the sidebar region. Replace the closed form with a bisection on the composite (as in 5.3).

What the plugin does better than both: pills are measured *in their place* (the card under them, the veil from the
title fade) and write a value only on change, and the sampling is event driven, never per scroll. The proposal keeps
that structure: pills inherit the composite of the surface they sit on.

## 9. Immersive and faded, as a function of the proposal

Not needed as separate switches. Craft's Immersive is "the sidebar carries the picture"; Faded is "the picture
dissolves toward the page colour down the window" (80% of the height, section 3.2, row 3). In Calternal the middle
card is the opaque-ish surface and the sidebars sit on the wallpaper, so the wallpaper fades into the card by
construction, and the three boxes cover the sidebar's tint. If a "Faded" look is wanted, it is a fourth box or a
preset field (a vertical gradient mask on the wallpaper, 80% height); it needs no new contrast logic because the
solver already measures the pixels under each surface. Recorded as a choice, not a recommendation (section 12,
question 6).

## 10. Part B, Apple settings windows

Method: the macOS VM (26.5.2, 1920 by 1080, Light, 1x), each app opened, its Settings window read by the harness
(`get_window_state`: AX frames plus a window screenshot), every toolbar tab clicked, and again in Dark. Xcode on the
Mac Studio (2x, dark). Sizes are AX frames in points relative to the window, ±1. Text point sizes are not in AX;
they are read off the screenshots at 1x and marked "about". Screenshots: see the issue.

### 10.1 Window and pane layout

| App | Layout | Window | Sections or tabs | Notes |
| --- | --- | --- | --- | --- |
| **System Settings** | Sidebar and content, one window ([appearance](https://git.kayg.org/attachments/0c8e0dcc-85c6-483c-9467-0307ee829cb2), [dark](https://git.kayg.org/attachments/86b99a2d-058e-4695-894a-abe9106d8d65)) | 723 by 700 pt default, resizable; toolbar 52 pt with back and forward | 31 sidebar rows (the AX tree lists 31, to y 1211) in about 7 groups separated by 13 pt gaps; two groups start with a 19 pt title row; search field above | Account row first (54 pt); the pane's title is in the toolbar |
| **Xcode** | Sidebar and content ([General](https://git.kayg.org/attachments/17679508-f7ce-4a77-8fac-af4676e15d81)) | 710 by 452 pt, the same for every pane | 12 sidebar rows (General, Appearance, Apple Accounts, Intelligence, Behaviors, Navigation, Notifications, Editing, Shortcuts, Source Control, Components, Locations), 180 pt wide, rows 32 pt, no group titles | The sidebar is flat; sections are inside the panes |
| **Safari** | Toolbar tabs ([General](https://git.kayg.org/attachments/9926a67d-cc2b-42a8-9e73-c68384b078a3), [dark](https://git.kayg.org/attachments/f0fa111d-28b2-4796-b844-c19d98cc9345)) | 836 pt wide fixed, height follows the pane: 234 (Security), 237 (AutoFill), 329 (Profiles), 354 (Privacy), 390 (Search), 465 (Tabs), 509 (General), 538 (Advanced), 600 (Websites) | 13 tabs in one row (General, Tabs, AutoFill, Passwords, Search, Security, Privacy, Websites, Profiles, Extensions, Advanced, Developer, Feature Flags) | Tabs are icon over label, 55 to 83 pt wide by 56 high, toolbar at y 32 to 88; Passwords and Extensions open other apps' panes |
| **Finder** | Toolbar tabs ([General](https://git.kayg.org/attachments/65b661ba-6241-4726-8263-548066bef86c), [dark](https://git.kayg.org/attachments/c9ee721a-0909-4e5f-ad97-2d4806b99ac7)) | 377 pt wide; 350 (General), 392 (Advanced), 591 (Tags), 828 (Sidebar) | 4 tabs | The Sidebar pane alone has 21 checkboxes in one column |
| **Terminal** | Toolbar tabs; Profiles is a master-detail inside one tab ([General](https://git.kayg.org/attachments/d47fbf99-f981-4f12-b2b2-93fa359d5822), [Profiles](https://git.kayg.org/attachments/bdecd9b2-ba61-4422-8482-df4e6bbcccc4), [dark](https://git.kayg.org/attachments/6327e1d1-0832-4f8c-85e6-d3bd4e665b54)) | 667 by 628 pt for every pane | General, Profiles, Window Groups, Encodings; Profiles has a **list of themes with a live thumbnail each and a "Default" mark**, a `+ − ⋯ Default` footer, and 6 sub-tabs (Text, Window, Tab, Shell, Keyboard, Advanced) over bold-headed sections | Encodings is 105 checkboxes. The theme list is the closest Apple has to Craft's gallery: name, thumbnail, default mark, editor on the right |
| **Notes** (bad) | One window, no navigation ([light](https://git.kayg.org/attachments/4a54c373-e495-47ad-8fff-fc7b45bc85f4), [dark](https://git.kayg.org/attachments/1ad95442-e58a-486f-bcbd-d08fa25644ce)) | 548 by 663 pt (687 in dark) | none | Three pop-ups, five checkboxes, a slider and a button in one column |
| **Reminders** (bad) | One window, no navigation ([light](https://git.kayg.org/attachments/50bd17df-1e04-44f5-b9a7-4f5a124b3470), [dark](https://git.kayg.org/attachments/d8713fd6-38cd-4a5f-9bb4-95a49ea97d91)) | 414 by 601 pt | none | Six checkboxes with sentence labels, a pop-up, a stepper, a field |
| **Mail** | not verified | | | See section 2 |

### 10.2 Grouping, headers and row anatomy

**System Settings and Xcode (sidebar style).**

- Content is a column of **inset rounded groups**. In System Settings the content area starts
  about x 231 (the sidebar's right edge 223 plus 8) and a group is 460 pt wide (AX: x 243 to 703): about 12 pt in from
  the left and about 20 pt from the right edge (the scroll bar). Simple rows are
  about 39 pt (two rows of a group measured 39 and 39); navigation rows (About, Software Update) are 44 pt.
- Group **headers** are bold 13 pt text above the group with about 24 pt before and 8 pt after ("Theme", "Windows" in
  Appearance; "Starting Points", "Navigation and Files" in Xcode General).
- **Row anatomy**: label on the left (about 13 pt regular, primary ink), the control on the right edge, aligned to the
  group's inner right edge; optional **help text** under the label in secondary ink, about 11 pt, up to 3 lines, at
  most about 70% of the row width (Xcode "Restore open projects and workspaces"). Rows are separated by 1 pt hairlines
  inset from the left to the label's edge.
- A row can be a **drill-down**: title, a short right-aligned summary ("3 behaviors"), a chevron (Xcode Behaviors,
  [screenshot](https://git.kayg.org/attachments/4d8070d7-96dd-417e-952c-3f764546d6d2)). This is how long lists stay out of the
  sidebar.
- Controls by type (measured, System Settings Appearance): **segmented picture tiles** for Appearance (Auto, Light,
  Dark: three 74 by 66 pt tiles with a caption) and Liquid Glass (Clear, Tinted); **round swatches** for Theme colour
  (9) and Icon style (4); **pop-up menu** with a trailing chevron icon for text highlight and folder colour (about
  24 pt tall); **switch** for "Tint window background with wallpaper color" and "Show on all Spaces"; **pop-up** for
  Sidebar icon size. Xcode adds a **segmented control as a sub-tab** (Theme, Fonts, Editor) and a **slider with a
  swatch** for Primary tint.

**Toolbar-tab windows (Safari, Finder, Terminal, Notes, Reminders).**

- Tabs are icon over label, 55 by 56 pt, in the title bar. Pane content is a two-column form: a **right-aligned label
  column**, then a control column starting at x 357 (Safari; 302 pt wide, pop-ups 26 pt tall, rows on a 33 pt pitch,
  checkboxes 18 pt) or 156 (Notes: controls 373 wide) or 152 (Reminders). Labels end at x 350, a 7 pt gutter.
- **Help text** is under the control, in the control column, in secondary ink (Safari "Open safe files",
  Reminders, Notes).
- A circular **?** help button at the bottom right, 26 pt (Safari, Notes).
- No groups, no headers; vertical gaps of 33 pt between related blocks stand in for sections (Safari General has
  three blocks separated by 33 pt).

### 10.3 Search, deep links, and separating destructive or privileged settings

| Question | System Settings | Xcode | Others |
| --- | --- | --- | --- |
| Search | Field in the sidebar, 195 by 28 pt at (18, 61); typing replaces the sidebar with a list of **panes** ("dark mode" gives one row, Appearance, with its icon, [screenshot](https://git.kayg.org/attachments/62fbf05b-ac60-4767-8247-7bba61e0efc5)). In the one query tried ("dark mode") it listed the pane, not a row | none | none (Safari, Finder, Terminal, Notes, Reminders have no search) |
| Deep link | `open "x-apple.systempreferences:com.apple.Siri-Settings.extension"` opened the Siri pane directly (verified) | none found | none |
| Navigation history | back and forward buttons in the toolbar | back and forward | none |
| Destructive | A plain grey button with an **ellipsis** at the end of its row ("Delete Siri & Dictation History…", "Transfer or Reset") expected to open a confirm (not pressed here); no red anywhere in the page | **Reset…** and **Reset All** in their own rows at the bottom of General (AX: buttons at y 1309 and 1381 in the page) | Terminal: a `−` button in the profiles footer; Notes: "Set Password…" |
| Privileged | No lock icon was seen on any pane; the credential prompt on a privileged action was **not triggered** in this study, so how it appears is unverified here | same | |
| Default or unset | `Automatic`, `Use System Setting` as an explicit menu value (Xcode) | | |

### 10.4 Sizes and spacing, in one table (points, AX or px)

| Item | System Settings | Xcode | Safari | Finder |
| --- | --- | --- | --- | --- |
| Window | 723 by 700 | 710 by 452 | 836 by 234 to 600 | 377 by 350 to 828 |
| Toolbar or title bar | 52 | 52 | 32 + 56 (tabs) | 32 + 56 |
| Sidebar width | 215 (x 8 to 223) | 180 | none | none |
| Sidebar row | 32 pt, 13 pt gap between groups | 32 pt | | |
| Content inset | 12 | about 20 (px) | label column to x 350 | label column to x 48 |
| Row pitch | 39 (simple), 44 (navigation) | about 36.5 (px at 2x) | 33 | 22 to 23 (checkbox rows) |
| Control height | pop-up 24, tile 66 | pop-up 24 | pop-up 26, field 26 | pop-up 24 |
| Light vs dark | same layout and control sizes in both (AX frames compared); window heights differ only where the app restored another pane (Finder General 392 in the dark run against 350, Notes 687 against 663) | dark only (std) | same | same |

### 10.5 Why the good ones are good, why the bad ones are bad

**Good: System Settings and Xcode.**

1. **A section is a place.** Twelve or thirty rows in a sidebar are all visible, each opens a page of one subject,
   and the window never changes size. Safari's tabs also do this but cap at about 13 and resize the window per
   pane, which makes the content jump.
2. **One row, one decision, one control.** A label, one control, optional help; no sentences as labels. Nothing
   asks you to read a paragraph to know what a switch does.
3. **Grouping is shown, not described.** Inset rounded groups and a bold header carry the structure; the eye can skip
   a group.
4. **Control matches the data.** Pictures for looks, swatches for colours, pop-ups for lists, switches for on or
   off, a slider only for a continuous amount. No stepper, no text field for a choice.
5. **Defaults are first-class.** "Use System Setting" and "Automatic" are menu values, so reset is a choice.
6. **Safe by shape.** Destructive actions are separated, last, ellipsised and confirmed; nothing is red until the
   confirm.
7. **Findable.** A search field in the sidebar (System Settings) and URL-like deep links.

**Bad: Notes and Reminders.**

1. One flat page with no groups; unrelated things sit together (Notes: sort order, default account, text size,
   locked notes).
2. **Sentence labels on checkboxes** ("Show all-day reminders as overdue", "Set Time Zone Manually") each with a
   two- to four-line explanation, so the page is read as prose; Reminders' page is 601 pt tall with a stepper and
   a field for one value.
3. **Mixed alignment**: Notes has labels right-aligned at x 150, indented checkboxes at 156 with help text at 180, a
   slider at 156, and a disabled grey "Use Touch ID" below; Reminders aligns its controls at 152, but its
   first-column labels are sometimes at the left edge and sometimes missing ("Set Time Zone Manually" has no label).
4. **Mixed controls with no rule**: a pop-up, a stepper, a slider, checkboxes and a button together; the user
   cannot tell what applies at once.
5. No place for an advanced or destructive item, so they are interleaved (Notes: "Set Password…" at the bottom, between
   a slider and a disabled checkbox).

## 11. Calternal's Settings (proposal)

Settings is an ordinary app (`docs/settings.md`): same three regions as every other app, with a sidebar list of
sections and the page in the middle card. No search box (the palette finds every setting, decided). Every setting is an
operation, so every row has an address.

### 11.1 Sections: User and Admin

The sidebar shows groups with short headers (Apple uses gaps; headers say "User" and "Admin" because that is the
decided split and discoverability matters more than minimal chrome).

| Group | Section | Contents | Address |
| --- | --- | --- | --- |
| **User** (follows you) | Appearance | Preset gallery; wallpaper; Blur, Dim, Tint; fonts; UI size; accent; wordmark; motion | `/settings/appearance` |
| | Agents | New-thread mode (default Approve for me), retry and snooze, streaming, sandbox idle time, decision service and its provider | `/settings/agents` |
| | Keyboard | Shortcuts | `/settings/keyboard` |
| | Machines | Your own machines (add by SSH) | `/settings/machines` |
| | Plugins | Your plugins, per-person options | `/settings/plugins` |
| | Skills | Your skills | `/settings/skills` |
| | Automations | Your schedules and triggers | `/settings/automations` |
| | Account | Name, picture, passkeys, connected provider accounts (what you sign in to, never stored by the server), sign out | `/settings/account` |
| **Admin: this space** (Manager or Owner of the current space) | Members and roles | Members, roles as capability sets | `/spaces/<id>/members` |
| | Invites | Create, list, revoke | `/spaces/<id>/settings` (invites) |
| | Secrets | The space's secrets, by handle | `/spaces/<id>/settings` (secrets) |
| | Instructions and defaults | Space instructions for agents; space defaults for appearance that members may override | `/spaces/<id>/settings` |
| | Space plugins | Which plugins are enabled in this space | `/spaces/<id>/plugins/<plugin-id>` |
| **Admin: server** (the operator) | Groups and plans | Groups named after plans, what each allows | open (see 12) |
| | Machine pool | The cloud machines | open |
| | Storage | The three roots, health | open |

The group is only shown when the person has at least one capability in it; rows inside it show read-only state
when they hold only the read capability (a Reader sees who the members are but gets no Invite button).

### 11.2 Row anatomy and control rules

**Row.** `[label][help text under it]` on the left, **one control** on the right edge, 1 px hairline between rows,
rows in an inset rounded group (`r-card`, 14), a bold 13 pt header over each group. One decision per row. A
label is a noun phrase, never a sentence; the sentence goes in the help text, 12 px, secondary ink, at most two
lines.

**Sizes** (desktop, from the measured Apple values and the shell tokens; touch uses the existing `row-h-settings` 52 and
`pill-h` 44): row 40 (Apple 39 to 44), sidebar row 32, group gap 24, header to group 8, group inset 12, column
max width 560 and centred like Notes' narrow column (Apple's content area is about 492), control column right-aligned,
minimum control width 160. Light and dark only change colour and the wallpaper veil, never layout.

**Control rules.**

| Data | Control | Example |
| --- | --- | --- |
| One independent on or off | **Switch**, applies at once | Streaming, Reduce motion off switch |
| A *set* of independent options shown together and meant to be read as a set | **Checkboxes** in one group | Blur, Dim, Tint (decided) |
| One of 2 to 4 mutually exclusive, visual | **Picture tiles or a segmented control** | Scheme (Auto, Light, Dark), new-thread mode |
| One of 5 or more, or a list that grows | **Pop-up menu** with an inline search when it has providers or machines | Default machine, model |
| A colour or a look | **Swatches or a gallery** | Accent, wallpaper, presets |
| A continuous amount | **Slider**, one only | UI size |
| A number or a name | **Text field**, validated on blur | Display name, idle minutes |
| A list of things | **Drill-down row** with a summary and a chevron | Members: "12 members ›" |
| An action | **Button** with a verb, ellipsis if it opens a confirm | Create invite… |
| A destructive action | Last group of the page, plain button, ellipsis; only the confirm's primary button is red | Delete space… |

No Save button. A change applies when made, is undoable for appearance and shortcuts, and follows the decided rule
for agents: reversible changes just happen, anything about access asks (`docs/settings.md`).

**Privileged.** The page is readable by anyone with read access; the *action* asks, in the shell's own sheet
(passkey), when it changes access (roles, invites, secrets, plans). Hide a whole Admin group only when the person
holds no capability in it.

### 11.3 In the app's three regions

- **Left sidebar** (not a card, on the tinted background): header wordmark; the attention card if anything needs
  attention; **body = the sections**, the three groups above with their headers; footer: account picture,
  switcher, search icon. One selected row, the section; keyboard up and down moves it.
- **Middle card**: the page. Title scrolls away (the top of the card fades, no solid band). Groups of rows in a
  centred column of at most 560. The Appearance page opens with a live **preview tile** (a miniature of the shell on
  the chosen wallpaper in both schemes, like Apple's Auto, Light, Dark tiles) so the three boxes show what they
  do, then the preset gallery with "Show all (N)" rows like System Settings' Wallpaper pane (`sysset_Wallpaper`
  [screenshot](https://git.kayg.org/attachments/b12ab535-a5aa-48b2-8934-214527556b96)), then wallpaper, materials, fonts, motion.
- **Right sidebar**: AI chat by default (decided). Settings' plugin adds one panel tab, **Details**: for Appearance
  the contrast report for the current surface (the numbers in 6.1 for the selected wallpaper), for Admin pages the
  recent changes from the recovery log, so an agent's change is visible and revertable.
- **Phones**: the section list is the first screen; a row pushes its page; the back chevron returns; same
  addresses; touch sizes. A deep link opens the page with the row highlighted for a moment.

### 11.4 Deep links and the palette

A setting is `/settings/<setting-id>` per `docs/addresses.md`. A section opens as a section; a row is addressed
inside it (`/settings/appearance/blur`), highlights, and scrolls into view. The palette indexes every operation's
title, help text and section; selecting a result opens that address. Space-scoped settings keep
`/spaces/<id>/settings`. Apple's deep link `x-apple.systempreferences:<pane>` is the same idea with no row level;
this adds the row level.

## 12. Open (decisions for the owner; options and costs)

1. **Which ink sets the readability floor, and may a surface flip its ink?** (a) Veil sized by **primary ink at
   7:1**, ink-2 and ink-3 derived per surface (proposed): wallpaper visible 21 to 49% on sidebars, hierarchy of the
   ink shades is kept except where the wallpaper is hard. (b) Veil sized by **ink-3 at 4.5:1** as the shell lane
   does: simple, but 0.74 to 0.88 opaque, wallpaper nearly gone. (c) Allow flipping a surface to the other scheme's
   ink when that needs a smaller veil, as Craft's header and Apple's Liquid Glass do: flips six of eight wallpapers in the light scheme and saves 0.06 to 0.88 of veil (ink-3 at 4.5), but one app can then show a dark sidebar in
   light mode, which touches the design system and costs a visible rule ("surfaces may invert"). Cost of (c): one more
   output (`inverted`) per surface and a transition rule.
2. **What do the boxes mean?** Section 5.1 reads Tint as hue plus the theme-coloured layer and Dim as a further
   neutral layer, using the plugin's strengths. Alternatives: Tint = hue only (no opacity of its own), Dim = the
   whole veil. Cost: the numbers in 6.1 move, nothing else.
3. **Percentile**: 1% guarantees 99% of the area; the single worst pixel is within 0.8 to 1.0 of target here. A guarantee of the
   absolute worst pixel costs 0.02 to 0.1 more veil (not measured here; a one-line change).
4. **The card's default** is 0.94 (0.85 tint then 0.60 dim); its readability floor is 0.56 to 0.79. Show more of the
   wallpaper through the card (floor plus a margin) or keep Craft-like near-opacity? The card is where text
   lives.
5. **Wallpapers and schemes**: the default set suits dark mode; either pair each with a light rendition (the
   backgrounds lane's recommendation) or accept heavy light-scheme veils (0.51 to 0.79).
6. **Faded** (an 80% fade of the wallpaper into the page colour) as a fourth box, a preset field, or never.
7. **Setting IDs.** `docs/addresses.md` says IDs are UUIDs; settings are declared in manifests as operations, so a
   readable operation name (`appearance.materials.blur`) is the natural ID. Allow names for settings, or generate
   UUIDs per declared operation and map them in the manifest?
8. **Server-operator addresses**: groups and plans, machine pool and storage have no address in `docs/addresses.md`.
   `/server/<section>` or `/settings/server/<section>`.
9. **Admin group visibility**: hide it when the person has no capability, or show it read-only. Proposed: hide a
   group only when there is nothing readable in it.
10. **Section headers** ("User", "Admin") against Apple's gap-only grouping: the proposal uses headers because
    the owner named the split; it costs 2 rows of vertical space.
11. **Mail Settings** could not be studied; if it matters, someone should add a throwaway mail account on the VM and
    repeat Part B for Mail.

## 13. Reproduction, what was created and removed

- Wallpaper analysis: eight images fetched into `/tmp/ucs-walls` on `hd01` and `calternal-dev-02-std`, deleted at
  the end of the lane; reference function and driver scripts in `~/Developer/lanes/ui-craft-settings/eval` on
  `calternal-dev-02-std` (not in the repository; the function is in 5.3).
- Craft viewer: Playwright (existing `node_modules` of the `ui-backgrounds` lane, symlinked), Chromium 1255. DOM
  dump script and the bundle in `~/Developer/lanes/ui-craft-settings` there.
- macOS VM: `~/Developer/lanes/ui-craft-settings` (scripts, 35 screenshots), `~/Applications/Craft.app`,
  `dl/Craft.dmg`, an own `cua-spacesd` on `127.0.0.1:3298`; the VM was set back to **Light** and these are removed
  at the end. Apps opened: Finder, Safari, Terminal, Notes, Reminders, Mail, System Settings (no accounts,
  nothing saved).
- Mac Studio: Xcode launched, Settings read (12 panes), window closed, Xcode quit; `cua-driver` daemon started with
  `open -a CuaDriver --args serve`; scripts in `/tmp/ucs-xc` on std (removed).

## 14. What could not be verified

- The native Craft app, the editor's Style panel, the gallery and the contrast guard (no account); only the
  shared-page viewer and the earlier studies.
- Mail Settings.
- Text point sizes and corner radii of Apple's rows (no AX access to text metrics; estimates from screenshots).
- Frame cost of the solver on a device; the figures in 5.3 are for JavaScript on a development VM. No frame-rate
  claim is made.
- Other bb or host-theme sidebar colours (the plugin's `--sidebar` is the host's); the plugin comparison uses the
  shell's paper tokens.
- The shell lane's `washFor` was tested as a formula on pixels, not through its running page; the finding in
  section 8 should be confirmed by running `e2e/wallpapers.mjs` with a percentile check.
- Eight photos at width 1600 and quality 60 from Unsplash's CDN; production assets will be the release files.

## 15. Simplification opportunities

- **Drop the three-layer model for one veil**: tint, dim and floor all collapse into a single `(colour, alpha)` per
  surface, and Tint becomes "the veil's colour carries the hue". Same output, one number to store.
- **Replace `washFor` and the pill code's `material()` with the one bisection** in 5.3, and delete the hand-tuned
  50% white wash the plugin needed; the shell's three separate hand-tuned places (washFor, accent, pill
  contrast, per the backgrounds lane) become two.
- **No blur or saturation parameters at all**: constants per role (12, 75, 1.2, 1.4); the tokens already hold them.
- **Do not build a settings search.** The palette indexes operations; the sidebar already lists every section.
- **Generate the sidebar from manifests**: a section is a group of operations with a title, an icon and a
  capability; no hand-written section list or admin check, which also answers the "only if allowed" visibility.
- **Gallery tiles are the live preview component** used at small size, not separate preview images.
- **Dark and light renditions of each wallpaper** remove the need for ink flipping and for any veil above the
  default.
- **One layout for every page**: rows, groups, a header, a footer for the destructive group; plugins fill rows, they
  do not build pages.
