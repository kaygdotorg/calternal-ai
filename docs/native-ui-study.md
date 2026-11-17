# Native UI study (macOS 26 and iOS 27)

Lane `native-ui-study`, issue #19, 2026-11-17. Purpose: study the system apps once so nobody
has to again, and turn the numbers into web tokens for `docs/ui-shell.md`. Token-style values
are in `docs/native-ui-study.json`; reference images are attachments on issue #19 (https://git.kayg.org/kayg/calternal-ai/issues/19), linked below.

Read section 1 first: it says what every number is worth. Section 9 lists what could **not**
be measured. Nothing here is a Calternal decision; sections 7 and 8 are proposals for the UI
shell lane.

## 1. Method, devices and trust levels

| Platform | Device and OS | Scale | How |
|---|---|---|---|
| macOS | Tahoe VM `calternal-macos-01-std`, macOS 26.5.2 (25F84), arm64, one 1920x1080 display | **1x**: 1 point = 1 pixel, so text and 1 pt strokes are anti-aliased coarsely | cua-spacesd (own loopback instance, launched with `open` so it holds the Accessibility and Screen Recording grants): `get_window_state` with `full_output` returns every accessibility element's **frame in points** plus a window screenshot; `set_window_frame` and `drag` to resize and probe limits |
| iPhone | iOS 27.0 simulator "iPhone 18 Pro" (402 x 874 pt, 1206 x 2622 px, **3x**) on the Mac Studio, Xcode 27.2 | 3x | `xcrun simctl io screenshot` / `recordVideo` (60 fps), `simctl ui` (appearance, content size), `simctl launch/openurl`. No accessibility tree (see section 9) |

iOS 27.1 is installed but would not create an iPhone 18 Pro, so 27.0 was used.

Trust levels used in the tables:

- **AX**: an accessibility frame in points. Exact for layout, but it is the *model* value
  (it jumps to the end state during an animation).
- **px**: measured from a screenshot with a small script (edges, bounding boxes, arc fits) and
  divided by the scale. About +-1 pt on macOS (1x), +-0.4 pt on iPhone.
- **fit**: a size inferred from ink height (SF Pro cap height 0.705 em, descender about 0.21 em).
  Checked against the known iOS Dynamic Type table at two sizes (it matched to under 0.5 pt).
- **doc**: a documented or well-known Apple value that was *not* re-measured here. Marked so
  you can discount it.

**Legend used in every table:** unmarked numbers are measured (AX or px). "(inferred)" means derived from a measured number. "(doc)" means taken from Apple's documentation and not re-measured; sources are listed in section 12. Pass 2 re-read pass 1 against the images and the JSON; the corrections are listed at the end of section 11.

Windows were set to chosen sizes, so "default" below means the size the app opened at or the
size stated. Nothing personal is on screen: demo notes written for this study on a VM with no
accounts.

## 2. macOS layout (Tahoe, "Liquid Glass" windows)

### 2.1 Window chrome

| Item | Value | Source |
|---|---|---|
| Window corner radius (windows with a toolbar) | about 28 pt (arc fit gives 26 to 30) | px |
| Traffic lights | 16 pt discs, left edges at x = 18, 41, 64 (23 pt pitch), centred at y = 26 | AX |
| Toolbar band height | **52 pt** in Notes, Finder, Safari, Settings | AX |
| Toolbar item hit area | 44 to 45 pt wide, full 52 pt high; glass *capsule* buttons draw inside as **38 x 38** at y = 7 (7 pt vertical inset) | AX |
| Search field in a toolbar | Finder 174 x 38; Notes about 322 x 36; capsule ends | AX / px |
| Address field (Safari) | about 442 x 34 centred, capsule ends | px |
| Segmented view switcher (Finder) | four 36 to 37 pt wide segments, 36 high, group is one capsule | AX |

The toolbar has no hard line and no solid fill: its buttons are glass capsules floating over
the content, which scrolls underneath (see 6.2).

### 2.2 Sidebar (floating panel)

The sidebar is a rounded panel **inset 7 to 8 pt** from the window's left, top and bottom
edges (outline frame starts at x = 7; row tints start at x = 8). Content to its right is flush
with the window edge, unframed.

| App | Panel (outline) width | Row height | Section header | Row tint inset / icon column | Source |
|---|---|---|---|---|---|
| Finder | 173 default; **min 120**; dragging to x=80 clamps at 120, to x=10 collapses (threshold not found); max follows the window (761 of 1100 = 69 %) | 32 | 19 | row x=8, w=171; icon cell starts x=18 (10 in) | AX, probed with drag |
| Notes | 222 default; 202 at the minimum window width (709) | 32 (folder), list row 56 | 19 | row x=8, w=220 | AX |
| Settings | 215 | 32 | 19; profile row 54 | row x=8, w=198; text at x=23 | AX |

The sidebar's resize handle is not at the visible edge: a drag started 8 pt inside the edge (x=188 of 196) did nothing, one at 198 (2 pt outside, in the gap) worked.

### 2.3 Lists, content, panes

| Item | Value | Source |
|---|---|---|
| Notes list column | 280 wide at default window; row 56 high, content inset 16 each side (cell 248 in a 280 row); date section header "Today" about 15 pt semibold | AX |
| Notes editor | text area x = 509 to 1261; text starts 22 to 23 pt from the pane's left; the date line is 15 high, centred | AX |
| Notes at minimum window width 709 | sidebar 202, list about 200, editor 283: the three panes persist; below 709 the window refuses to shrink | AX (`set_window_frame` 600 returned 709) |
| Notes at wide (1800) | editor text area 1274 wide: **no reading-width cap**; lines run the full pane | AX |
| Settings page | cards 460 wide at x = 243 (20 pt right margin), rows **44** high, 1 pt dividers, **10 pt** between groups; header card (icon 52, title, caption) 460 x about 160 | AX |
| Safari start page | content column 728 wide (178 to 906) centred in a 1100 window; Favorites tiles 72 pt, pitch 93.6 | px |
| Inspector / right pane | **not measured**: no app with an inspector exists on the VM (no Xcode, Pages, Keynote). Finder's Preview pane was not opened. See section 9 | n/a |

Split-view behaviour: panes never overlay on macOS; a pane collapses (Finder: drag past the
minimum, or ctrl-cmd-S; Notes: the sidebar button) and the others take its width. Window
minimums keep the remaining panes at their own minimums (Notes 709).

### 2.4 Mac type (SF Pro; inferred from cap height at 1x)

Mac system sizes are smaller than iOS: the standard text is **13 pt**.

| Where | Cap or ink height | Inferred size and weight | Source |
|---|---|---|---|
| Notes list title, Finder row, Settings row label | 9 pt cap | 13 pt (body; titles semibold) | fit |
| Finder / Notes sidebar section header ("Favorites", "On My Mac") | 8 pt cap | 11 pt semibold, secondary colour | fit |
| Settings page title "General" | 16 pt cap | 22 pt bold (Title 2) | fit |
| Safari "Favorites", "Privacy Report" headings | 16 pt cap | 22 pt bold | fit |
| Notes "Today" list heading | 14 pt incl. descender | about 15 pt semibold | fit |
| Notes editor title (Heading style) | 17 pt cap | about 24 pt bold | fit |
| Notes editor body | 9 pt cap | 13 pt | fit |
| Notes row time and preview | x-height only | 12 pt (time in medium, preview secondary) | doc |
| Line heights, tracking | not measurable at 1x | SF Pro text sizes 11 to 19 use a tracking table (about 0 at 13 pt, about +0.06 at 11 pt, about -0.4 to -0.6 at 22 pt), line height about 1.23 to 1.3 x size | doc |

### 2.5 Mac spacing, shape and colour

| Item | Value | Source |
|---|---|---|
| Sidebar panel inset from window | 7 to 8 pt | AX |
| Sidebar row tint (selected) radius | about **8 pt** (arc fit identical on 32 pt rows and on Notes' 56 pt list row) | px |
| Settings group card radius | about 13 to 14 pt | px |
| Capsule controls (search field, address field, toolbar buttons) | radius = half height (28 pt field: 14) | px |
| Row to row inner insets | sidebar icon 10 pt in from the tint, 16 in lists | AX |
| Selected list row (Notes) | #DCDCDC on white; sidebar selected #F2F2F2 on #F7F7F7 (light, active window) | px |
| Separator | #E6E6E6 vertical hairline between list and editor, 1 pt | px |
| Sidebar background | #F7F7F7 flat over a white-ish desktop, content #FFFFFF | px |
| Icon sizes | toolbar glyph about 16 to 18 pt in a 38 pt capsule; sidebar icons about 16 pt in a 20 pt column | AX / px |

## 3. iPhone layout (iOS 27, iPhone 18 Pro 402 x 874 pt)

| Item | Value | Source |
|---|---|---|
| Large title | ink 30 pt (cap + descender) at the default size = **34 pt bold**; ink left edge at 18 pt (text origin 16) | px, fit |
| Bottom search (Messages, Settings): a floating glass **capsule** | **about 347 x 49 pt**, inset **27 pt** from each side and **27 pt** above the bottom edge; magnifier left, mic right; 17 pt placeholder | px |
| Safari bottom bar: three floating glass pieces | back circle 35 to 80 pt (about 45), address capsule 91 to 311 (**220 x 49**), tabs circle 321 to 367 (about 46); gaps about 10 pt; bottom edge 34 pt above the screen bottom (the home-indicator inset) | px |
| Grouped list (Settings) | page #F2F2F7, cards white, **17 pt** side margin (16 plus anti-alias), rows **52 pt** pitch, 1 pt separators, icon tile **28 x 28** at 13 pt inside the card, separator starts 72 pt from the card's left (icon rows) | px |
| Row title text | digits and parentheses span 15.7 pt = 17 pt (Body) | px, fit |
| Chevron | right end, about 9 pt wide, secondary colour | px |
| Reminders first-run sheet | edge-to-edge sheet, top about 62 pt below the screen top, large top corners (not measured; the mockup's earlier 40 pt measurement of Safari's sheet is the better number), primary action a blue **capsule** button 326 x 51 pt with 38 pt side margins ([ios27-reminders.png](https://git.kayg.org/attachments/0e0817aa-be8d-43e6-9a31-6618d0c60fbf)) | px (approx, from the screenshot) |
| Status bar | time at 177 to 311 px = x about 59 to 104 pt; battery right edge about 276 pt (island layout) | px |

Safe areas were **not** read from the system (no accessibility tree). Known for this class of
device (doc): top about 62 pt, bottom 34 pt. The bottom 34 pt value agrees with the Safari bar's
measured bottom gap.

### 3.1 Dynamic Type (measured on Messages; title ink heights)

| Content size | Title ink (pt) | Implied title size | Row title ink (pt) | Implied size |
|---|---|---|---|---|
| extra small | 27.3 | 31 | 12.7 | 14 |
| large (default) | 30.0 | 34 | 15.7 | 17 |
| extra extra extra large | 35.3 | 40 | 20.3 | 22 |
| accessibility XXXL | wraps | - | wraps | - |

These match Apple's table: Large Title 31, 32, 33, **34**, 36, 38, 40 for XS to XXXL and
44, 48, 52, 56, 60 for accessibility sizes; Body 14, 15, 16, **17**, 19, 21, 23, then 28, 33,
40, 47, 53. At the accessibility sizes the layout reflows rather than scaling: names wrap onto
several lines, the date drops *below* the name (the two-column row becomes a stack), and the
chevron stays right-aligned ([ios27-messages-accessibility-extra-extra-extra-large.png](https://git.kayg.org/attachments/ca13cea3-d360-449e-b043-b6b1fb1e68a3)). The
bottom search pill does **not** grow with Dynamic Type (it stays 49 pt, glyph and text
inside stay at the default).

iOS text styles at the default size (doc): Large Title 34/41 regular (bold in navigation),
Title 1 28/34, Title 2 22/28, Title 3 20/25, Headline 17/22 semibold, Body 17/22, Callout 16/21,
Subhead 15/20, Footnote 13/18, Caption 1 12/16, Caption 2 11/13. Tracking falls from about
+0.4 at 11 pt through 0 at 17 to about -0.4 at 28+ (SF's optical tracking).

## 4. Materials

What was observed:

- **macOS 26 windows**: the toolbar is *transparent*. Buttons are glass capsules (translucent
  fill, a faint edge, very soft shadow) floating on the content; the content scrolls under
  them with a soft fade. Safari's toolbar tints with the page colour (the blue-grey in
  [macos26-safari-mac.png](https://git.kayg.org/attachments/6dd228aa-5392-4721-b6b5-13ffc269e100) is the system tinting the glass toolbar, not a flat fill).
- **Sidebars** are floating glass panels inset from the window edge and rounded to be
  concentric with the window (window about 28, panel about 20). Over the very light VM
  wallpaper the sidebar read as a flat #F7F7F7; the translucency is real but low contrast
  here. A dark-wallpaper run would show more (not done).
- **iPhone**: the bottom search and Safari's bar are glass capsules floating *above* the
  content (they overlap the page text, which shows blurred through them, see
  [ios27-safari-page.png](https://git.kayg.org/attachments/30dbe3d1-8a9a-4493-9853-5ae01853eb19)), with a thin lighter edge and a very soft shadow. Navigation and
  tab surfaces in Settings are not separate bars any more; content runs under them.
- **Where it is not used**: list rows, grouped cards, the content itself, and text areas are
  opaque. Glass is for the layer that floats above content (controls, sidebars), never for the
  reading surface.
- **Light vs dark (iPhone)**: dark page a dark grey (about #222 in Safari, not pure black; eyeballed, colours not sampled), the glass fill goes a darker grey with a light edge; text switches to white with secondary lowered ([ios27-safari-dark.png](https://git.kayg.org/attachments/bb90142a-9f70-4599-ac34-ed042069df63), [ios27-settings-dark.png](https://git.kayg.org/attachments/38b09e28-9fa5-45f8-80c2-c660afcf31cd), [ios27-messages-dark.png](https://git.kayg.org/attachments/b5aef430-7a6c-4578-b06b-a69f59cfab1a)).
- **Vibrancy**: secondary labels (previews, dates, captions, section headers) are a lowered
  alpha of the label colour, not a separate grey, so they stay legible over any wallpaper.

Not measured: blur radius, saturation boost, refraction. The real material samples and
*bends* the backdrop (lensing at the edge, a specular rim that follows the light); that is
rendered by the system compositor and has no CSS equivalent (section 8).

Shadows: windows and floating capsules use a large, soft, low-opacity shadow with no hard
outline; separators between list and detail are a 1 pt hairline (#E6E6E6 light).

## 5. Controls

| Control | Observed |
|---|---|
| Toolbar buttons (macOS) | glass capsules or circles 38 pt high; icon-only, 16 to 18 pt glyph; groups share one capsule (Finder views, Notes format tools) with 1 pt dividers |
| Pill / capsule search | 28 pt high in a sidebar (Settings), 36 to 38 in a toolbar; radius = half height; magnifier 15 pt, 8 pt in |
| Segmented control (macOS) | one capsule, selected segment a lighter inner capsule |
| Prominent button (iOS) | blue **capsule**, 51 pt high, 326 wide (38 pt side margins) (Reminders first run) |
| Small plain button (Safari "Customize Start Page") | grey rounded rectangle 162 x 30, text 13 pt |
| Badges | Notes sidebar count: plain secondary text at the trailing edge (no pill); iOS chevron rows show a secondary value then a chevron |
| Selection | macOS sidebar: rounded tint (radius about 8); list: darker neutral tint across the row; iOS: row highlight only while pressed, large selected state on iPad |
| Toggles, menus, popovers, focus rings | **not captured** in this pass (section 9) |

## 6. Motion

Measured: **app launch / switch on iOS** (Simulator, `recordVideo`, 60 fps, frame times from
the file): first changed frame to settled frame = **15 frames = 0.25 s** (4.673 to 4.923 s;
second launch the same, 7.110 to 7.325 s, about 0.22 s). The change is a fast cross-dissolve
with scale, not a long spring.

macOS sidebar collapse (Finder, ctrl-cmd-S): sampled at 10 Hz by screenshot; one intermediate
frame was caught, so the transition finishes **within about 0.2 to 0.3 s**. Accessibility
frames jump to the end state, so they cannot time it.

Not measured (no input path into the iOS Simulator and no video backend on the VM): push,
sheet, popover, selection and pull-to-refresh timings, and reduced-motion behaviour.
Do not quote the following as measured: Apple's published guidance is that system
transitions run about 0.25 to 0.4 s, with spring-driven sheets (response about 0.35,
damping about 0.85) and that Reduce Motion turns slides and zooms into cross-fades.

## 7. What this means for Calternal (tokens and rules for web)

Regions follow `docs/ui-shell.md`. "Mockup" is
`packages/ui/src/styles/tokens.css` on `review/switcher-a-pills`.

### 7.1 Where the mockup already matches

| Mockup token | Value | Native finding |
|---|---|---|
| `--text-body` 17 on phone | 17 | iOS Body 17 (measured) |
| `--text-section` 11, `--text-label` 12, `--text-ui` 14 | | Mac header 11 semibold, Mac callout 12; Mac body is 13 (so 14 is one step above native) |
| `--hit-min` 44 | 44 | iOS minimum; Mac toolbar hit area 44 x 52 |
| `--r-pill` (capsule) | 999 | every search, address and bar control is a capsule |
| `--r-card` 14 | 14 | Settings group card about 13 to 14 |
| `--r-wcard` 20, `--r-win` 32 | | Mac window about 28 and floating sidebar about 20: card 20 with window 28 is the same concentric rule |
| `--sheet-inset` 8, 47/40 corners | | consistent with the sheet measured earlier (#469); not re-measured |
| `--dur-slow` 300 ms ceiling, `--dur-base` 220 | | measured iOS app transition 220 to 250 ms |
| Glass: blur then tint, never near-opaque | | matches: Apple's glass is a thin tint, not a solid |
| Toolbar over content with no stroke | | matches: no hairline outlines |
| Phone sheet bottom gap, bottom pill inset | `--sheet-pad` 14 | native pill inset is 27 (see 7.2) |

### 7.2 Where the mockup differs (value to consider)

| Region | Mockup | Native | Suggested token |
|---|---|---|---|
| Sidebar row height | `--st-row` 30 | Mac sidebar rows 32 (Finder, Notes, Settings) | 32 on desktop |
| Selected row radius | `--r-wrow` = 20 - 8 = 12 | about 8 | 8, or keep concentric (12) as a deliberate choice |
| Window inset | `--win-margin` 5 | sidebar inset 7 to 8 | 8 |
| Gap between panes | `--pane-gap` / `--win-gap` 10 to 12 | Notes and Finder panes abut (0 gap); only the sidebar floats | keep gap only between a floating card and the sidebar; list and editor flush |
| Toolbar | `--pill-h` 28, top toolbar 32, `--bar-h` 44 | toolbar band 52, capsule buttons 38 | desktop toolbar 52 with 38 buttons (or 44 / 32 if density wins) |
| Desktop text | `--text-ui` 14 | Mac 13 | 13 for a native-feel Mac theme; 14 reads larger than the system |
| Title | `--text-title` 30 | iOS large title 34 bold; Mac window title 22 | phone title 34 at default size |
| Bottom pill | `--tier-cap` 52 | search and Safari bar 49 high, 27 to 34 above bottom | `--tier-cap` 49, offset 27 (+ safe-area) |
| Phone circle buttons | `--tier-top` 44 | Safari circles about 45 to 49 | 46 or the same 49 as the capsule |
| Dynamic Type | `--type-px` scaling | iOS reflows at accessibility sizes (stack, wrap); the bottom capsule does not grow | keep scaling to XXXL, switch to stacked rows beyond about 1.35x and cap capsule height |
| Notes reading column | narrow centred column | system Notes runs full width of the pane (text inset 22) | the narrow centred column is a Calternal choice, not native; keep, but make the width a plugin setting |
| Settings / list rows | rows 32 | iOS grouped row 52, Mac settings row 44 | settings rows 44 (desktop) and 52 (phone) |

### 7.3 Rules by region

- **Sidebar**: floating panel, inset 8, radius about 20 (window radius minus inset), rows 32,
  row tint radius 8, section header 11 pt semibold secondary at 19 high, icon column 20 with
  16 pt glyphs, default 215 to 222, min 200, max about 50 % of the window. Dragging below the
  min collapses it (Finder). Bottom footer (account, app switcher, search) stays inside the panel.
- **Middle card**: no inner card inside it for reading apps; list rows 56 for two-line items
  (title 13 semibold, preview 12 secondary), 16 pt content inset, a 1 pt `rgb(0 0 0 / .09)`
  divider, selected row a neutral tint at radius 8.
- **Right card**: a glass or flat pane at the same inset; panel tabs at its bottom follow the
  iOS tab idiom (below). Width: 320 to 380 (doc; not measured).
- **Request bar / composer** (floating over content): capsule, 49 high on phone and 38 to 44 on
  desktop, 17 pt (phone) or 13 pt (desktop) text, leading icon 18, trailing mic or send,
  27 pt inset from the sides and bottom on phone (plus safe area), shadow soft and low.
- **Narrow screens**: bottom tab / capsule bar floats 34 pt above the bottom (home indicator);
  page content scrolls under it with a fade; the sidebar slides over rather than shrinking.
- **Type**: phone Large Title 34 bold, Body 17, Footnote 13; desktop body 13, section 11
  semibold, window title 22 bold. Follow the mockup's `--type-px` for Dynamic Type.
- **Shape**: capsule for anything interactive and short, card 14, panes 20, window 28,
  concentric inner radii (outer minus inset).
- **Hit targets**: 44 x 44 on touch; on desktop the visible 38 pt capsule sits in a 44 x 52 target.
- **Motion**: 220 to 250 ms ease-out for app and pane changes; nothing over 300 ms; Reduce
  Motion turns slides into 120 ms cross-fades (the mockup's rule already does).
- **Native behaviours worth copying** (doc, not captured): cmd-comma settings, cmd-N new,
  cmd-F search, cmd-1..n panes, ctrl-cmd-S sidebar, arrow-key list navigation with type-to-find,
  context menu on right-click, swipe actions on rows (iPhone), pull to refresh, and scroll
  edge fades under glass controls.

## 8. What the web cannot match exactly

| Native | Why it cannot be exact | Closest honest approximation |
|---|---|---|
| Liquid Glass refraction and specular rim | the system compositor bends and lights the backdrop live | `backdrop-filter: blur(20px) saturate(180%)` + 1 px inner highlight (`--glass-float-edge`) + soft outer shadow; no lensing |
| Backdrop sampling that adapts to what is behind | needs per-frame sampling of the page behind | tint plus brightness shift (`--pane-bright`), already in the mockup; limit to few big surfaces |
| SF Pro and its optical tracking | not licensed for the web | the mockup's Google Sans Flex with per-size letter-spacing; on Apple devices `system-ui` gives real SF |
| Continuous ("squircle") corners | CSS rounds with circular arcs | `border-radius` plus `corner-shape: squircle` where supported (Chromium 139+, Safari TP); otherwise a 3 to 5 % larger radius |
| Rubber-banding, momentum, spring physics | browser scroll is not scriptable the same | native scrolling; `overscroll-behavior`; springs via the mockup's easing (capped at 300 ms) |
| Large title collapse, swipe-back, interactive sheet dismissal | system navigation controller | CSS scroll-driven animations for the title; pointer-driven sheet drag with velocity |
| Dynamic Island, safe areas | | `env(safe-area-inset-*)` (already used) |
| Share sheet, system menus, haptics | | Web Share API; own menus with native rows (32 desktop, 46 phone); `navigator.vibrate` where present (not iOS) |
| Window chrome (traffic lights, toolbar) | a browser tab has none | none in a tab; the installed PWA gets `display-override: window-controls-overlay` |

## 9. What was not measured

Plainly:

1. **Mac dark mode.** Writing `AppleInterfaceStyle` and relaunching Notes left it light; the
   window kept the light look (the setting was reverted). Dark iPhone shots exist.
2. **Mail and Messages on the Mac.** Both need an account; Mail showed only the add-account
   sheet and Messages a sign-in. No layout data. iPhone Messages (a list with seeded demo
   threads) was captured; **Mail and Notes do not exist** in the iOS 27 Simulator image.
3. **An inspector-style app.** No Xcode, Pages, Keynote or Photos on the VM; Finder's preview
   pane was not opened. Inspector widths are unmeasured.
4. **Reminders on the Mac** (the window never opened without Apple Events permission, which
   was denied on purpose), the Mac **share sheet**, menus, popovers, context menus, hover,
   pressed and focus states, toggles, badges beyond Notes' count.
5. **Anything needing input in the iOS Simulator.** Simulator.app was not running and opening
   it would have shown other people's devices, so no taps: no push or sheet animation timing,
   no share sheet, sheet detents, swipe actions, pull gestures, keyboard layout, scroll edge
   behaviour, or empty and loading states beyond the first-run ones captured.
6. **Reduced motion**, spring parameters, and any animation inside an app.
7. **Real accessibility metadata on iPhone**: safe areas and bar heights are derived from
   pixels or marked "doc".
8. **Blur radius, saturation and colour of glass** over a varied backdrop; one light wallpaper
   only.
9. **Mac Retina rendering**: the VM is 1x, so stroke weights and icon detail are less
   trustworthy (icon sizes are from frames, not strokes). Stroke weights were not measured.
10. **Keyboard shortcut enumeration and focus order**: listed from knowledge only.

## 10. Reproduction

All measurements came from scripts that were kept out of the repository; the method is
repeatable: `get_window_state` with `full_output` for frames, PNG edge scans for radii and
sizes, `simctl io recordVideo` plus `ffprobe` frame times for motion.

What was created and removed:

- macOS VM (`lume@calternal-macos-01-std`): my own `cua-spacesd` on `127.0.0.1:3299` (stopped),
  four demo notes in Notes and an unused attempt at a Reminders list (left in place, harmless),
  lane directory `~/Developer/lanes/native-ui-study` (removed). The existing daemon on 3211 was not touched.
- Mac Studio (`kayg@std`): one simulator `native-ui-study-iphone` (created, used, **deleted**),
  directory `~/Developer/lanes/native-ui-study` (removed), a `cua-driver` daemon started by a
  probe (stopped; it listed every window once, no content kept). Nothing else was changed.

## 11. Second pass (2026-11-17, later)

### 11.1 What became possible and how

- **Mac video.** The driver's own recorder has no video backend on this VM, so a tiny Terminal-hosted `ffmpeg -f avfoundation -framerate 60` (nix `ffmpeg.bin`) records the screen while the driver sends the input. Terminal was given Screen Recording on this disposable VM (the VM has SIP off; one row updated in the system TCC database with `sudo`, then `tccd` restarted). Frames come out at 60 fps with exact timestamps; transitions are found with `tblend`/`signalstats` frame differences and tracked edges.
- **Mac dark mode** works through System Settings' Appearance pane by accessibility (the `defaults` route did not). The VM was set back to Light afterwards.
- **iPhone input did not become possible.** Xcode 27.2 on the Mac Studio contains no `Simulator.app` (searched `/Applications`, the Xcode bundle and Spotlight), so there is no window to click and `simctl` has no touch command. Only headless `simctl` work was possible. See 11.5.

### 11.2 Dark mode (macOS 26.5.2)

Sampled from [macos26-dark-notes.png](https://git.kayg.org/attachments/011d3758-203a-4222-8b5b-115a65566cd9):

| Surface | Dark value | Light value (pass 1) |
|---|---|---|
| Editor / content | #1E1E1E | #FFFFFF |
| List column (translucent over the wallpaper) | #232A2C | #FFFFFF |
| Sidebar (translucent over the wallpaper) | #282D2E | #F7F7F7 |
| Selected list row | #464646 | #DCDCDC |
| Selected sidebar row | #1E2324 | #F2F2F2 |
| Hairline between list and editor | #000000 | #E6E6E6 |
| Hairline under the toolbar | #393F41 | none |
| Toolbar search field | #273033 | translucent grey |
| Settings page / card (dark) | #222829 / #2A3033 | n/a |

The dark sidebar and list pick up the blue-green of the wallpaper: they are real glass, not fixed greys (a different wallpaper gives different greys). Text is white-ish with secondary at lowered alpha; selection stays neutral grey in lists; the sidebar selected row accent is the app's colour (Notes yellow).

**System Settings > Appearance > Liquid Glass has two options, Clear (default) and Tinted** ([macos26-dark-settings-appearance.png](https://git.kayg.org/attachments/0c72fb9c-f30a-4467-a064-f5fbd8583c8e)). The user can therefore make glass more opaque system-wide; a web glass should offer the same choice (a "Tinted" mode with a higher tint alpha is the honest analogue). The same pane has Icon and widget style, Theme colour, Folder colour and "Tint window background with wallpaper colour".

### 11.3 Menus and popovers (dark, 1x)

| Item | Size and metrics | Source |
|---|---|---|
| Context menu (right-click a Notes row, [image](https://git.kayg.org/attachments/4047614f-b64e-4ed3-9bfd-102f8cda1225)) | 247 x 256 pt, background about #262626, rows about 24 pt apart, 11 pt extra between groups (hairline separators), leading 14 pt icons, 13 pt labels, trailing chevron for submenus; the clicked row keeps the app accent as its tint while the menu is open | px |
| Format popover (Notes "Aa", [image](https://git.kayg.org/attachments/6f50f0ba-e3b4-479c-87ec-e534d453ea8f)) | 177 x 326 pt body plus a 6 pt pointer toward the button, top row of B / I / U / S / highlighter / colour 38 pt high, then text styles each in its own style, a check mark for the current one, two hairline groups | px |
| Popover and menu shape | corner radius about 14 to 16 (estimated from the images, not fitted); thin lighter edge; soft large shadow | inferred |

Escape did not close the popover through the driver's `press_key`, and while it was open the main window's accessibility tree was not reachable; a click elsewhere or relaunch was needed. (The real keyboard path was not tested.)

### 11.4 Motion measured (macOS, 60 fps)

| Transition | Duration | Curve | How |
|---|---|---|---|
| Finder sidebar reveal (cmd-ctrl-S), a 761 pt wide panel | **about 0.19 s** (11 frames from first to last moving frame; the whole burst including content fade-in 0.22 to 0.23 s over three repeats) | speed rises then falls symmetrically (px per frame 10, 24, 48, 64, 76, 85, 85, 88, 82, 69, 43): ease-in-out. Least-squares cubic-bezier fit: **(0.4, 0, 0.6, 0.7 to 1)** over 0.18 s, rms error 0.019 | panel right edge tracked per frame, then fitted |
| Same, the duration does not depend on the distance (the panel was 761 wide here, 173 by default) | | time-based, not speed-based | repeat of 3 |

Not completed on the Mac: sheet, popover, push-like page change in Settings and selection timings. The harness ran, but the click sequence was not stable (a matching element was missing on repeat runs; frame differences from Terminal's own window changes polluted the first seconds), so those numbers are **not** reported. The measuring method is sound and cheap now (section 11.1); the remaining Mac transitions are a short follow-up.

### 11.5 iPhone: what is still missing

Everything that needs touches: push and sheet timing, detents, swipe actions, context menus, share sheet, pull to refresh, keyboard, reduced motion, per-row Dynamic Type for list rows with real lists, safe areas read from the system. Options:

1. **XCUITest runner** driven by `xcodebuild test` against the simulator I create: a UI test bundle can tap, swipe, long-press, read every element's frame (so safe areas, navigation bar and tab bar heights come from the real accessibility tree) while `simctl io recordVideo` records at 60 fps. Cost: hand-writing a small Xcode project (no `xcodegen`/Tuist on the machine, and installing is not allowed there) and a signing-free test run; about half a day. Best fidelity.
2. **Open Xcode.app** itself (Xcode 27 hosts the simulator window) and click with cua-driver. Cost: Xcode may be in use by other lanes; I did not open it.
3. **Private HID client** (what `idb` uses) in a small Swift tool: fragile across OS versions. Not recommended.

### 11.6 Corrections to pass 1

- Finder sidebar collapse threshold: pass 1 said "about 100"; the probe only showed 80 clamps at 120 and 10 collapses. Fixed in 2.2.
- Resize handle: pass 1 said about 18 pt outside the outline; the probe found 188 (inside the visible edge) did nothing and 198 worked. Fixed in 2.2.
- Pass 1 said "no inspector app and no Mac dark mode": dark mode is now covered (11.2); inspector is still missing (Finder Preview pane and Preview.app were not opened in this pass).
- Reminders first-run sheet: top corner radius removed (not measured).
- Everything else in sections 2 to 8 was re-checked against the stored measurements and the JSON; no further numeric mismatch found. The JSON now carries a `trust` field per group.

## 12. Sources for "(doc)" values

Pointers recalled from memory, **not re-fetched or re-verified in this pass**; treat the values marked (doc) as unconfirmed until checked against these pages.

- iOS and macOS text styles, Dynamic Type size tables: https://developer.apple.com/design/human-interface-guidelines/typography
- Layout, safe areas, device sizes: https://developer.apple.com/design/human-interface-guidelines/layout
- Liquid Glass overview and adoption: https://developer.apple.com/documentation/TechnologyOverviews/liquid-glass and https://developer.apple.com/documentation/TechnologyOverviews/adopting-liquid-glass
- Motion and reduced motion: https://developer.apple.com/design/human-interface-guidelines/motion and https://developer.apple.com/design/human-interface-guidelines/accessibility
- SF Pro tracking table: https://developer.apple.com/fonts/

## Simplification opportunities

- `--pane-gap`, `--win-gap`, `--win-margin`, `--float-gap` and `--frame-pad` are five names for
  "space between surfaces"; native uses one inset (8) and abutting panes.
- `--tier-top`, `--tier-cap`, `--tier-cap-tall`, `--bar-h` and `--pill-h` could be two tiers
  (44 touch, 49 capsule) plus the desktop 38.
- The twelve `--spring-*` and `--dur-*` tokens collapse to three durations (120, 220, 300) and two curves.
