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
