# UI performance

Speed is a feature of the shell (`docs/ui-shell.md`). This records the techniques and what was measured.
Every number here is a **development-host number**: headless Chromium 159 with software compositing on
`calternal-dev-02-std` (aarch64, 10 cores), device scale factor 2, 1440 by 900. They are not product
benchmarks and say nothing about a real GPU, a phone or Safari. Real-device measurement is still to do.

Reproduce: `cd apps/web && npm run build && node e2e/perf.mjs` (inside `nix develop`, on a dev VM).

## The test

The fixture thread `t-long` has **10,000 messages** (`apps/web/src/apps/threads/generate.ts`, deterministic):
about 80% of agent messages carry a folded group of tool steps, one in 250 carries a 2,400-line build log,
and one in eight has a generated-UI table card. Messages are produced on demand from a seeded generator,
so the fixture costs no memory until a range is read.

## Techniques

| Problem | Technique | Where |
| --- | --- | --- |
| 10,000 rows of unknown height | Windowed list: only rows near the viewport exist in the DOM. A Fenwick tree holds every row's height (an estimate until measured), so "first row to draw" and "offset of the window" are O(log n) lookups; a height change is O(log n). | `lib/VirtualList.svelte`, `lib/fenwick.ts` |
| Rows above the viewport change height after measuring | One `ResizeObserver` for all drawn rows; a frame's measurements are applied together and the scroll position is corrected once, so the reader never sees a jump. | `VirtualList.svelte` |
| Layout work while scrolling | The scroll handler reads `scrollTop` only and writes state only when the first or last row changes (rAF-coalesced). No per-row layout reads. Rows use `contain: layout style`; the scroller uses `contain: strict`. | `VirtualList.svelte` |
| Reading a long file | Messages are read in pages of 100 around what is on screen, never the whole thread; unread rows show a skeleton. | `conversation.svelte.ts` |
| Streaming text re-rendering the list | The streaming message is one extra row at the end; its text is its own reactive field (`stream.text`), read by that row only. A delta changes one text node in one row. Markdown is re-parsed for that message only. | `conversation.svelte.ts`, `ThreadView.svelte` |
| A 2,400-line tool output | Folded by default. When opened, a second fixed-line-height window draws only the lines in view (about 30 nodes for 2,400 lines); the text is split once, when first opened. | `LogView.svelte` |
| Following the end of a growing list | A list at the bottom stays at the bottom while it grows, and starts there; once the reader scrolls up it stops following. | `VirtualList.svelte` |
| The middle card's blur | Not a live filter. The wallpaper is blurred once, when chosen, into a small canvas (1/k of the viewport, k up to 8) exported as a JPEG blob; the card paints that image plus a tint and a dim. The same pass reads the brightness grid and the colours. Measured below: the live-blur alternative misses 47 % to 60 % of frames on the same list. | `appearance/wallpaper.ts`, `Card.svelte` |
| Adaptive glass cost | Brightness behind a surface comes from the background's own 3 by 3 colour mesh (nine interpolations), never from pixels; one batched read per layout change (resize, scheme change, mount), none on scroll. | `packages/ui/src/lib/material.ts` |
| Backdrop blur is the expensive part of a frame | One blur layer per surface, never nested; surfaces never set filter, opacity below 1 or masks on themselves (that makes them a backdrop root). Touch forms scale the blur down (token `blur-scale` 0.7). | `glass.css`, tokens |

## Measured

Scrolling the 10,000 message thread (120 steps of 900 px up from the end, 40 random jumps anywhere in the
thread, 120 steps of 700 px down), one frame per `requestAnimationFrame`. Three runs of each variant, back to
back, host load average 0.5 to 0.9:

| Variant | Phase | Median | 95th percentile | Worst | Frames over 20 ms (of 120, 40, 120) |
| --- | --- | ---: | ---: | ---: | ---: |
| **Pre-blurred wallpaper (shipped)** | Steady up | 16.7 ms | 16.7 to 16.8 ms | 16.8 ms | 0, 0, 0 |
| | Random jumps | 16.7 ms | 16.7 to 16.8 ms | 16.8 ms | 0, 0, 0 |
| | Steady down | 16.7 ms | 16.7 to 16.8 ms | 16.8 ms | 0, 0, 0 |
| Live `backdrop-filter: blur(75px)` over the card (what this replaces) | Steady up | 16.7 to 16.8 ms | 33.4 ms | 49.9 to 50.1 ms | 57, 59, 57 |
| | Random jumps | 16.7 to 33.3 ms | 33.4 ms | 33.4 ms | 20, 19, 20 |
| | Steady down | 16.7 to 33.3 ms | 33.4 to 49.9 ms | 33.4 to 66.7 ms | 60, 58, 73 |

16.7 ms is the display's 60 Hz frame. With the wallpaper blurred once, no frame in any run was late. With a live blur over the same card, 47 % to
60 % of frames missed a vsync and the worst took 66.7 ms. (An earlier run of the same list, before the wallpaper work, with the card as a
smaller live glass layer on a busier host, had 5 % to 25 % slow frames; the live-blur rows above are the controlled comparison.)

Choosing a wallpaper (click to everything applied: two canvas renders, the brightness read, the wash, the colours): 106 to 140 ms over 15 picks,
including a 60 ms debounce and the test driver's round trips. Window resize repeats the render after a 400 ms debounce.

DOM size (all elements on the page, including the shell), final build:

| Position | Message rows drawn | All elements |
| --- | ---: | ---: |
| At the end (start) | 9 | 623 |
| At the top | 11 | 593 |

Ten thousand messages cost the same DOM as ten.

Streaming (the running fixture thread, 4 s of text arriving every 40 ms): **84 to 86 DOM mutations, all inside
the streaming row** (plus the window's own transform); no other row was touched; no long task (50 ms or
more) was recorded.

## Not measured yet

- Real devices: an iPhone, a Mac with Safari, a mid-range Android. The first number that matters is the
  blur's cost on a phone GPU.
- Memory after a long session, and a thread of 100,000 messages (the height table and the tree are O(n) memory, 16 bytes a row).
- Cold start: the app waits for its font family before first paint; its cost on a slow network is unknown.
- First-open of a long thread against the real server (pages of 100 messages per call is a guess).

## Costs measured on 2026-11-17, and the three fixed

`e2e/perf-suite.mjs` measures the five things that matter (frames from `requestAnimationFrame`, long tasks of 50 ms or more, time to the visible result), at 1x and at 4x CPU
slowdown; `e2e/perf-profile.mjs` is a V8 sampling profile of one scenario by function. Dev VM (shared, load about 1), headless Chromium, 1440 by 900. One frame is 16.7 ms.
These are development-host numbers, not benchmarks.

**Before (at 1x nothing is late; at 4x the costs show):**

| Scenario | 1x | 4x CPU |
| --- | --- | --- |
| First load (cold, no cache) | first contentful paint 108 ms; 2,143 KiB transferred (images 1,643, fonts 376, JS 110, CSS 13); 10 requests | FCP 308 ms |
| Scroll the 10,000-message thread (200 wheel steps) | 0 late frames, no long task | 161 late frames of 584, p95 50 ms, 5 long tasks (worst 422 ms) |
| Switch thread (12 clicks) | click to painted 31 ms median | 54 ms median, 12 late frames, 3 long tasks (worst 439 ms) |
| Open the Account sheet over the wallpaper (6 times) | ready in 56 ms | 7 late frames, 3 long tasks (worst 400 ms) |
| Open the palette and type 36 characters | open 38 ms, keystroke to painted rows 16.6 ms median | 5 late frames, 3 long tasks (worst 399 ms) |

**The three worst costs:**

1. **Start-up rendered every generated wallpaper.** The profile at 4x showed 1,445 ms in one anonymous function: the library drew 9 generated looks in 3 forms each (27 canvases) while the
   default is a photograph. Now a look is only listed; its picture is rendered when it is chosen, its thumbnail when a picker opens (`library.resolve`, `thumbOf`, `appearance.ensureThumbs`).
   Start-up self time at 4x: 1,445 ms to 226 ms. The 400 ms long tasks inside the sheet, palette and thread-switch runs (they were this work finishing late) are gone.
2. **Every message row paid for a style recalculation.** `bounce` read two tokens with `getComputedStyle` when each button was created; a message row creates three buttons, so scrolling
   created hundreds and forced a style recalculation each (187 ms at 4x in 80 steps, in the profile). The tokens are now read when a click happens. And the three buttons and the menu of a
   message are created when the message is first pointed at or focused (`armed`), not for every row (touch has no hover, so there they are made at once).
   Scrolling at 4x: late frames 161 to 94 to 102 of 584, p95 50 to 33 ms, long tasks 5 to 0 to 1.
3. **The wallpaper files were bigger than the pre-blurred copies need.** The photographs and their light renditions were 2,400 px; the app draws them at 1/8 to 1/2 scale with a blur. They are
   now 1,600 px at quality 72 (`scripts/wallpapers.mjs`): first load 2,143 KiB to 1,097 KiB (images 1,643 to 594), no visible change.

**After:**

| Scenario | 1x | 4x CPU |
| --- | --- | --- |
| First load | FCP 108 ms; 1,097 KiB | FCP 356 to 400 ms (308 before: within the run-to-run spread, three runs; the win is bytes and main-thread work, not first paint) |
| Scroll 10,000 messages | 0 late frames | 94 to 102 late frames, p95 33 ms, 0 to 1 long task |
| Switch thread | 33 ms median | 7 to 9 late frames, 0 long tasks |
| Open a sheet over the wallpaper | ready in 57 ms | 1 to 2 late frames, 0 long tasks |
| Palette open and typing | open 31 ms, keystroke 16.6 ms | 1 late frame, keystroke 16.4 ms median, 0 long tasks |

**Blur and veils add no per-frame work.** The wallpaper is blurred once into small canvases and the veil is solved once per wallpaper, scheme, box or window-size change
(`appearance/veil.ts`); while scrolling, the surfaces are plain images and gradients. At 1x the 10,000-message scroll still has 0 late frames. What remains at 4x is the message list itself
(creating and removing rows) and the sampling profiler's own `elementsFromPoint` hit tests.

## The field editor, stage 1 of the editor port (2026-11-17)

`packages/editor` (transactions with origins, history of the person's own changes, the input-method gate, the keystroke probe) now sits behind `InlineEditor`, so the composer, side chat, rename fields and
the palette field all run on it. Measured on the dev VM, Chromium at **4x CPU slowdown**, typing 74 characters in the thread composer at about 16 keys a second with the probe (`?test=1`, `e2e/field-probe.mjs`):

| Measure | Result | Budget carried from the existing app |
| --- | --- | --- |
| Key press to the next frame after the field changed | median 7.3 ms, p95 10.1 ms, worst 31.7 ms | composer key p95 8 ms (gated there; the probe measures to paint, which includes the frame) |
| Composer after the 2026-11-17 pass (4x CPU, dev-02, 3 to 5 runs) | script work per plain key: median 2.7 to 3.0 ms, p95 4.3 to 5.5 ms; key to next frame over all 74 keys: median 3.0 to 3.7 ms, p95 7.5 to 9.5 ms, worst 26 to 35 ms | p95 8 ms: met on script work; key-to-frame p95 straddles it. The worst keys are the first one and the ones that open a picker (@ # /) |

What changed: the field grows with `field-sizing: content` where supported (no `scrollHeight` read per key), the mirror copies font metrics once instead of reading computed style per key and exists only while there are marks, and the recogniser is warmed when the composer mounts. What remains is the frame boundary (a key landing just after a frame waits up to a frame) and the heavier first and picker keys; both are noise-dominated on the shared VM, so the figures above are ranges, not a pass claim for key-to-frame p95.
| Added to the first paint | 7.0 KB minified, **2.0 KB brotli** (the whole package) | under 3 KB |
| Palette keystroke to painted rows (`e2e/perf-suite.mjs`) | one frame, unchanged | one frame |

## The block editor, stage 2 of the editor port (2026-11-17)

`packages/editor/src/block` (CodeMirror 6 over plain Markdown, hidden syntax, parse-ahead, blocks, lists, handles) hosts a note in the Notes middle card. `e2e/editor-perf.mjs` on a fixture note of about 50,000 words
(`longNote()` in `apps/notes/client.ts`: headings, paragraphs, nested lists, tasks, code, quotes), Chromium on the dev VM, **4x CPU slowdown** like the existing app's budgets (1x in brackets):

| Budget (existing app) | Result |
| --- | --- |
| Keystroke to paint, 50,000-word note, p50 150 ms (a ceiling; typing at about 16 keys a second): near the end | median **8.2 ms**, p95 17.9 ms, worst 19.3 ms [9.1 / 19.5 / 22.8] |
| the same, in the middle of the note | median **9.4 ms**, p95 18.4 ms [10.3 / 17.3] |
| Open a note, short, from the list | 36 ms [30] |
| Open the long note from the list (warm) | median 30 ms, worst 49 ms [28 / 48]; the document is parsed to the end of the viewport before the frame, the rest at idle |
| Open the long note cold (a fresh page to the first drawn heading, including the lazy editor chunk) | 410 ms [192] (the existing app's cold-note boot budget is 3.5 s) |
| Scroll the long note, 200 wheel steps up and down, frame p95 50 ms | frame median 16.7 ms, p95 16.8 ms, **0 frames over 50 ms** [same] |
| Bundle | the editor is a lazy chunk, **90.8 KB brotli**, loaded when a note opens; the first-paint chunk is unchanged (105 KB brotli in total) |

What is not measured yet: typing inside a table cell and a 1,000-item nested list (tables are deferred; the long note has nested lists), and the Safari/WebKit target (p50 50 ms in the existing app); both belong with stage 3.

## WebKit: iOS Simulator Safari (2026-11-17)

Run with the in-page benchmark (`?bench=1&firstrun=0&test=1` on `/notes/n-1`, `src/bench.ts`, loaded only with that parameter), because Safari cannot be driven from outside. Typing goes through `execCommand("insertText")` on the focused field, the browser's own text-input path. Device: a fresh iPhone 18 Pro simulator, iOS 27.0, Safari, 402x714 viewport at 3x, on the Mac Studio (created for the run and deleted after). The Simulator runs on the host CPU, so these are faster than a phone: they show that nothing is slow in WebKit's layout or paint, not what a phone will measure. WebKit has no CPU throttle.

| Scenario | Result | Target |
| --- | --- | --- |
| Type in the long note (56 keys), key to next frame | p50 9 ms, p95 10 ms, worst 13 ms | WebKit typing p50 50 ms: met |
| Same, script work only | p50 8 ms, p95 9 ms, worst 10 ms | |
| Type in the composer (56 keys), key to next frame | p50 2 ms, p95 6 ms, worst 30 ms | composer p95 8 ms: met here |
| Open the long note, warm (5 runs) | p50 32 ms, p95 47 ms, worst 49 ms (a first run showed 111 ms once) | open p95 15 ms is the Chromium 4x desktop budget; no WebKit target set |
| Open a short note | 32 to 57 ms | |
| Scroll the long note (200 frames, 800 px steps) | p50 17 ms, p95 18 ms, worst 29 ms, 0 frames over 50 ms | p95 frame 50 ms: met |

Nothing breaches, so nothing was changed for WebKit. Playwright's WebKit does not run on the dev VMs: the build downloads, but the host lacks GTK 4, GStreamer and Pango libraries, and installing them needs root, which this lane does not use. The screenshot of the run is attached to issue #15. For a device number, run the same URL on a real phone.

## Editor lazy pieces and pickers (2026-11-17)

First paint is unchanged: the pieces are separate chunks fetched only when a note needs them (`e2e/note-pieces.mjs` checks that a plain note fetches none). Typing in the long note at 4x, key to next frame, three runs: median 12.0 to 13.9 ms, p95 18.9 to 20.3 ms (stage 2 recorded 8.2 and 9.4 median). Switching every piece off in the same session gives median 10.3 to 10.7 ms, so the pieces cost about 2 ms at 4x and the rest of the difference is the shared VM, which also moved the composer numbers today; the budget is 150 ms.

## Performance and reliability pass (2026-11-17)

Method for everything below: dev-02 (a shared VM; numbers move between sessions, so every comparison is an A/B inside one session with the builds alternating run by run), Chromium at 4x CPU slowdown, fresh page per run. Tools in `apps/web/e2e/`: `ab-typing.mjs` (two builds, typing in a note; key-to-frame from the probe, and main-thread script, layout and task time per key from CDP `Performance.getMetrics`, which is far steadier than frame times), `ab-composer.mjs` (the same for the composer, with a list of the keys over 8 ms), `profile-typing.mjs`, `profile-key.mjs` and `trace-key.mjs` (CPU profile and trace of one chosen key), `palette-focus.mjs`, `first-load.mjs` and `scripts/bundle-weight.mjs` (what a first load downloads, and which source files make up a chunk).

**1. Palette focus (the flaky check).** Cause: Ctrl+K showed the dialog, and the field took focus one animation frame later (`requestAnimationFrame` in `Palette.svelte`). For that frame, keys went to whatever had focus before, and a check made as soon as the dialog appeared failed. Measured over fresh opens at 1x: the dialog was on screen with the field not yet focused in 4 of 30 opens (focus always arrived, median 17.5 ms after Ctrl+K). Fix: focus in the same turn the dialog appears (the frame is kept as a fallback). After: 0 of 70 opens (40 at 1x, 30 at 4x). `e2e/palette-focus.mjs [runs] [--4x]`.

**2. Typing in the long note (stage 2 against now, same session, alternating, 10 rounds, 4x).**

| | Stage 2 | Before the fix | After | Plain short note, after |
| --- | --- | --- | --- | --- |
| key to next frame, median of run medians | 6.4 ms | 7.8 ms | 6.7 ms (stage 2 6.3 in that run) | 4.5 ms (stage 2 4.5) |
| script per key | 4.46 ms | 5.72 ms | 4.72 ms (stage 2 4.35) | 2.72 ms (stage 2 2.58) |
| layout and style per key | 1.5 ms | 1.4 ms | 1.4 ms | 1.2 ms |

The "12 to 14 ms" figure earlier in the day was a different session; in one session the gap was 1.3 ms, not 4 to 6. Now +0.4 ms in the long note and +0.14 ms in a plain note, inside the 1 ms target. Where it went (bisected with a build per commit, 7 rounds each): the composer and probe commit and the pickers commit measured equal to stage 2 (script 4.16 against 4.21, 4.31 against 4.30); the lazy pieces commit added 0.94 ms. Switching the code piece off recovered 0.4 ms and switching block IDs off recovered nothing, which pointed past the pieces' own code to how code was parsed: `markdown({codeLanguages})` makes CodeMirror run a nested-language parse (`parseMixed`) that walks the whole document tree on every edit. Fixes, each kept: code fences are coloured by parsing only the fences on screen, cached by language and text, with no mixed parse (`block/code.ts`); the pickers' trigger check looks at the line first and walks the parse tree only when a trigger character is there; every tree walk skips a fence's own grammar (`IterMode.IgnoreMounts`); the check for a newly needed lazy piece runs only for a change that contains a character a piece is made of, and not at all once every piece is loaded.

**3. Composer key to next frame (stage-2-era build `f7ee3d5` against now, 12 rounds, 4x).**

| Key | Before | After |
| --- | --- | --- |
| first key | 24.5 ms | 8.1 ms |
| the `@` that opens the picker | 11.6 ms | 3.0 ms |
| first letter after `@` | 12.6 ms | 9.1 ms |
| p95 inside a run (median of runs) | 11.3 ms | 8.2 ms |
| p95 over all keys pooled | 12.8 ms | 11.6 ms |

What was on those keys, from the traces: the first key flipped the input-modality attribute on the page (a rule on every element turns transitions off for keyboard use), a 15 ms style recalculation of all 685 elements at 4x; the `@` key built the popover and the search index; the first letter after it created the highlight layer and its font metrics. Fixes: a typed character in a text field defers the modality flip to an idle moment (Tab, arrows, Enter and Escape still flip at once); the pickers' search indexes are built once per pool, warmed when the composer is idle, and the popover opens one frame after the key that asked for it; the highlight layer exists from mount (metrics copied once) and holds text only while there are marks. The p95 is not yet reliably under 8 ms: the pooled figure still includes noisy stretches of the shared VM (consecutive keys of any letter slow), and `P` after `@` is still 9 to 10 ms. The within-run figure is at the budget, not under it with margin.

**4. First load (Threads page, uncompressed and gzip, as the page downloads it).**

| | Stage 2 | Before this pass | After |
| --- | --- | --- | --- |
| scripts | 360.3 KB (121.1 gzip) | 373.6 KB at the last report (about 368 KB measured after later fixes) | 283.7 KB (102.6 gzip), 8 files |
| styles | 77.2 KB (13.9 gzip) | 77.4 KB | 66.3 KB (13.2 gzip) |
| fonts, images | 375.6 KB, 625.3 KB | same | same (the images are the fixture wallpaper) |

Taken out of the first load: the Notes app (list, middle card, panels, its store and fixture notes) and the Settings pages load when first opened (a Notes page then fetches them with the editor chunks, 296 KB of script before paint and the rest after); the generated-UI renderer (60 KB) is fetched the first time a message holds a card or a question form is about to be drawn; the app data starts per app, Threads always and the others on first visit. Settings' registry stays in the first load because the palette finds every setting by its words before Settings has been opened. By the time the app paints, everything it asks for has arrived (the page waits for its font first), so the saving is download and parse time, not a changed paint time; a fast first paint with the rest after needs a decision about showing before the font arrives, which is not made here.

## Shell interactions (2026-11-18)

`e2e/shell-perf.mjs` at 4x CPU slowdown, dev-02, six runs each (the first of each includes anything loaded on first use):

| Interaction | Time to ready (median, worst) | Frames over 20 ms | Worst frame |
| --- | --- | --- | --- |
| Dragging a sidebar handle (153 frames) | n/a | 0 | 16.8 ms (median and p95 16.7) |
| Switching space | 31 ms, 38 ms | 0 in 6 runs | 17 ms |
| Switching app | 31 ms, 36 ms | 0 in 6 runs | 17 ms |
| Opening Settings | 38 ms, 52 ms | 1 in the first run, 0 after | 33 ms (was 200 ms) |

The first open of Settings was one 200 ms frame: the Appearance page asks for the thumbnails of the generated looks, and the "haze" look converted its colour mesh to OKLab again for every one of its 20,000 pixels (`recipe.ts`). The mesh is now converted once per scheme, thumbnails use a coarser grid than a wallpaper, and `ensureThumbs` paints one picture per frame. A prefetch of the apps loaded on demand was tried and dropped: it did not touch this cost and added bytes to the first load.

## The documentation site in the app's look (2026-11-18)

`apps/docs` now reads more of the single token source (`apps/docs/scripts/prepare.mjs` from `packages/ui/tokens/tokens.json`): radii, the pill height, the motion durations and curve, the press scale, and the default wallpaper's two tiny pre-blurred renditions. `calternal.css` uses them on Starlight's own classes only (no component is replaced beyond the existing theme picker): the wallpaper behind a veil on a fixed layer of its own, a translucent header and sidebars, the page as a rounded card, pill search and selects, the app's rounded rows and callouts, an accent focus ring. A themed preset drops the wallpaper. `e2e/docs-weight.mjs` (landing page and a doc page, both schemes, five runs):

| | Before | After |
| --- | --- | --- |
| First contentful paint | 36 ms | 20 to 44 ms |
| Stylesheet (gzip) | 13.2 KB | 13.6 KB |
| Document (gzip) | 9.3 KB | 9.5 KB |
| Image (gzip) | none | 3.8 KB (light) or 4.1 KB (dark), one of the two |
| Fonts, scripts | 192 or 214 KB, 30.3 KB | the same |

A first version put the wallpaper in `background-attachment: fixed` and blurred the header: it measured 44 to 56 ms and repaints on scroll, so it is a fixed pseudo-element now and the header is not blurred.

## What a key costs in the thread composer: three measures (2026-11-18)

`e2e/ab-key-work.mjs` types a 74-character line in the composer (60 ms between keys) at 4x CPU throttling, 20 alternating rounds of two builds (A: before the composer work of 2026-11-17 and 18, B: after: no page-wide restyle on typing, material measured once per surface, blind picker placement, progressive picker rows, warm first key), and reports three things for every key (1,480 keys per build):

| | dev-02 (headless Chromium, software compositing) | | Mac VM (headed Chrome, virtual GPU) | |
| --- | ---: | ---: | ---: | ---: |
| | A | B | A | B |
| **(a) main-thread work per key**, median | 4.9 ms | 5.2 | 3.8 | 3.9 |
| (a) p95 | 8.1 | 8.7 | 7.5 | **7.3** |
| (a) worst | 19.1 | 13.4 | 62.9 | **15.6** |
| (a) keys over 8 ms | 5.3% | 6.9% | 3.2% | 1.9% |
| (a) first key, median | 11.2 | 9.8 | 8.0 | 7.8 |
| **(b) keys not drawn in the very next frame** | 0 of 1,473 | **0 of 1,476** | 540 of 1,476 | 519 of 1,477 |
| **(c) key to next frame**, median | 8.6 | 8.4 | 15.3 | 15.3 |
| (c) p95 | 17.0 | 16.3 | 32.2 | 32.1 |
| (c) keys over 8 ms | 52.8% | 51.7% | 71.4% | 68.7% |

**(a) Main-thread work** is the key event's task plus the rendering task that draws it (style, layout, paint, commit), from a Chrome trace: the time the page's own thread is busy for that key, with no waiting for the display in it. It is the part of a key the page controls, and the one a code change moves. (Tracing itself adds some overhead, so the same numbers without it are a little lower.) **(b) Frames missed** counts keys whose frame was not the very next one: the first animation frame after the key came more than 20 ms after the frame before it, so a frame was dropped around it. On dev-02 none was; on the Mac VM about a third were, but that virtual display delivers frames about every 33 ms (it is not a 60 Hz panel), so a frame interval above 20 ms is the display's pace there, not work: read (b) on the Mac as a property of the VM. **(c) Key to next frame** is the app's own probe, from the key event to the next animation frame, and it includes the wait for the display's refresh (0 to one frame interval, 16.7 ms at 60 Hz, 33 ms on the Mac VM), so its median and p95 sit near a frame whatever the key costs: a median of 4 to 8 ms with half the keys "over 8 ms" is that distribution, not slow keys. The 8 ms budget has not been changed; which of the three it applies to is the owner's to say. As measured: **(a) p95 is 7.3 ms on the Mac VM (under 8) and 8.7 ms on dev-02 (just over, in a noisy, software-composited headless run), and (b) is zero on dev-02**; it cannot be judged on the Mac VM's display. The first key remains the largest single cost (about 10 ms on dev-02, 8 on the Mac): its script is first-time work (the composer now runs a warm-up key at idle, which took 1.2 ms off it), its style cost is the pause of decorative animations (3 ms), and the rest is paint and commit.
