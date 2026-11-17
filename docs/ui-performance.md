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
