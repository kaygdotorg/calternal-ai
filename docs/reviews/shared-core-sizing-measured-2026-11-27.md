# Shared core: Wasm size, one dependency at a time, measured · 2026-11-27

Lane `lane/engine-wasm-vs-ts` (issue #108). This is **item 1** of "Measurements
for a later lane" in `docs/reviews/shared-core-sizing-2026-11-27.md` on `dev`,
and replaces that paper study's size estimates for the dependencies listed here.
Built on `calternal-dev-01-compute02` only. Sizes are exact; nothing was timed.
**KB = 1,000 bytes.** Code and raw results: `spikes/engine-wasm-vs-ts/sizing/`,
`harness/build-sizing.sh`, `results/sizing/`.

## The short answer

Added download (brotli) on top of the engine, which is **82 KB** at `-Oz` and
102 KB at `-O3`:

| Dependency | `-Oz` | `-O3` | The paper study's figure | Reading |
|---|---:|---:|---|---|
| own frontmatter parser (small subset, reader only) | **+2.6** | +2.9 | part of "+40 to 90" for parser and writer | A floor: no byte-keeping writer |
| `yaml-rust2`, reading | **+23** | +33 | guess "30 to 60" beside an own parser | Lower than guessed |
| `yaml-rust2`, reading and writing | +25 | +36 | | |
| `pulldown-cmark` | **+51** | +63 | "+35 to 60" | As estimated at `-Oz` |
| `regex-lite` | **+19** | +20 | | |
| `regex`, without Unicode tables | +134 | +173 | | |
| `regex`, with Unicode | **+180** | +220 | "could add 100 to 250" | As estimated |
| `jiff`, no time zone database | **+61** | +69 | | |
| `jiff`, with the bundled database | **+125** | +135 | database alone guessed "50 to 200" | The database is **+64** over `jiff` without it |
| `rrule` (brings `chrono`, every time zone, and `regex`) | **+244** | +299 | "unmeasured" | Cannot be a plain module (below) |
| `similar` | **+39** | +68 | | |
| `yrs` (a small slice of its API) | **+53** | +87 | "+190 to 260 net" for Ywasm; linked in "somewhat smaller" | A floor (below) |
| `sqlite3.wasm` with FTS5, published build | **353** (wasm) + 60 (its JavaScript) | | "about +336", third party | About 413 in all |

The interpreter row of that study (candidate 10) is measured in
`plugin-interpreter-2026-11-27.md`: **QuickJS +233 KB** at `-Oz` (+305 at `-O3`),
inside the same module, within the study's guess of 200 to 400.

## How it was measured

One `cdylib` crate with one feature per dependency. Each feature exports one
function that uses the part of the dependency the core would use, on input handed
in at run time, so the compiler cannot compute it ahead or throw it away. Every
variant was built twice: **on top of the spike engine and its exports** (the
figure that matters, because standard library code is shared) and alone over an
empty shell (4.5 KB brotli). Cargo `opt-level` 3 or z, link-time optimisation,
one code generation unit, `panic = "abort"`; then `wasm-opt -O3` or `-Oz`
(binaryen 132), then `wasm-strip` removing every custom section; then gzip -9 and
brotli -11. rustc 1.99.1.

| Dependency (version) | What the export links |
|---|---|
| own parser (`sizing/src/frontmatter.rs`, 105 lines) | `key: value`, nested maps by indentation, `- item` and `[a, b]` lists, quoted and plain scalars |
| `yaml-rust2` 0.13.2, default features off | load to a tree and walk it; second row also writes it back |
| `pulldown-cmark` 0.13.6, default features off | all extensions, events with byte offsets; no HTML output |
| `regex` 1.13.2 / `regex-lite` 0.1.9 | compile a pattern from input, captures, replace |
| `jiff` 1.0.3 (`std`; `tzdb-bundle-always` for the database) | parse a zoned time and a span, add, format, change zone by name, civil dates |
| `rrule` 0.14.0 (`chrono` 0.4.45, `chrono-tz` 0.10.4) | parse a rule set, expand 100 dates |
| `similar` 3.2.1 | line diff, all changes, unified diff |
| `yrs` 0.28.2 | new document, apply an update, text insert and delete, a map entry, encode state and difference |

**A caution on every row:** only code reached from these calls is in the figure.
A core that uses more of a library pays more. `yrs` is the clearest case: an
editor binding would also need the XML types, observers, undo and awareness, so
+53 KB is a floor and says little against the Ywasm figure, which ships the whole
interface. The rows are **not additive**: dependencies share code (`rrule` and
`regex` share the regex engine).

## Full tables

### `-Oz` (size build)

| added | on the engine: raw | gzip | brotli | alone over an empty shell: brotli | JavaScript glue (brotli) |
|---|---:|---:|---:|---:|---:|
| **the engine alone** (total) | 203 | 95 | 82 | – | – |
| own frontmatter parser | +6.5 | +3.2 | +2.6 | +23 | – |
| yaml-rust2 (read) | +73 | +29 | +23 | +48 | – |
| yaml-rust2 (read and write) | +78 | +31 | +25 | +49 | – |
| pulldown-cmark | +148 | +66 | +51 | +59 | – |
| regex, with Unicode | +753 | +269 | +180 | +192 | – |
| regex, without Unicode tables | +445 | +180 | +134 | +146 | – |
| regex-lite | +55 | +25 | +19 | +27 | – |
| jiff, no time zone database | +214 | +78 | +61 | +69 | – |
| jiff, with the bundled database | +460 | +164 | +125 | +134 | – |
| rrule | +1,464 | +374 | +244 | +256 | 1.1 |
| similar | +191 | +60 | +39 | +47 | – |
| yrs | +170 | +65 | +53 | +74 | 1.0 |

### `-O3` (speed build)

| added | on the engine: raw | gzip | brotli | alone over an empty shell: brotli | JavaScript glue (brotli) |
|---|---:|---:|---:|---:|---:|
| **the engine alone** (total) | 330 | 131 | 102 | – | – |
| own frontmatter parser | +9.2 | +3.6 | +2.9 | +23 | – |
| yaml-rust2 (read) | +131 | +53 | +33 | +58 | – |
| yaml-rust2 (read and write) | +138 | +56 | +36 | +60 | – |
| pulldown-cmark | +203 | +90 | +63 | +70 | – |
| regex, with Unicode | +979 | +342 | +220 | +231 | – |
| regex, without Unicode tables | +666 | +248 | +173 | +184 | – |
| regex-lite | +62 | +35 | +20 | +28 | – |
| jiff, no time zone database | +324 | +100 | +69 | +77 | – |
| jiff, with the bundled database | +575 | +188 | +135 | +143 | – |
| rrule | +1,756 | +470 | +299 | +311 | 1.1 |
| similar | +331 | +104 | +68 | +75 | – |
| yrs | +371 | +130 | +87 | +116 | 1.0 |

The engine's `-O3` figure is 330 KB raw here against 385 KB in the first
measurement, because this run also passes it through `wasm-opt -O3`; the brotli
figure is about the same (102 against 103).

The own parser costs far more alone (+23) than on the engine (+2.6) because
number parsing and other standard library code is then not already there.

### Where the bytes are, by crate

From `twiggy` on the `-Oz` Cargo output **before** `wasm-opt`, names excluded, so
these are shares, a little larger than the shipped bytes (raw KB). Constant data
is not attributed to a crate by the tool.

The engine alone: engine 50, `serde_json` 48, constant data 39, `core` 31,
exports and unnamed 24, `alloc` 17, allocator 8, `hashbrown` 4, rest about 8.

| Dependency | Largest additions over the engine, raw KB |
|---|---|
| own parser | the parser 4, exports and unnamed 3, `core` 2 |
| yaml-rust2 (read) | `yaml_rust2` 49, data 7, `alloc` 5, `core` 5, `hashbrown` 5 |
| pulldown-cmark | data 67 (mostly its tables), `pulldown_cmark` 64, `unicase` 8 |
| regex, Unicode | data 345, `regex_automata` 217, `regex_syntax` 90, `aho_corasick` 59, `core` 51 |
| regex, no Unicode | `regex_automata` 216, `regex_syntax` 87, `aho_corasick` 59, `core` 51, data 41 |
| regex-lite | `regex_lite` 29, `core` 11, data 10 |
| jiff, no database | `jiff` 128, data 54, `jiff_core` 31, `core` 27 |
| jiff, with database | data 299, `jiff` 136, `jiff_core` 41, `core` 28 |
| rrule | data 947 (the time zones), `regex_automata` 200, `core` 91, `regex_syntax` 80, `aho_corasick` 54, `rrule` 39 |
| similar | `similar` 189, `core` 10, `hashbrown` 9 |
| yrs | `yrs` 82, `core` 34, `hashbrown` 19, `alloc` 15 |

## Findings

- **`rrule` does not fit the core as published.** It brings every time zone as
  data, a regular expression engine, and `chrono` with its browser bindings: the
  module then imports JavaScript functions (the clock and time zone offset) and
  needs the `wasm-bindgen` tool and its generated script (about 1 KB). A feature of
  our own crate cannot turn those bindings off, because `rrule` asks for them. This
  supports the study's own civil-time subset; I did not write or measure one.
- **`yrs` also needs the bindings as built here**, for a random client number
  (`getRandomValues`, reached through its `fastrand` dependency). A core that wants no JavaScript imports would have to supply
  the randomness itself.
- **The format proposal contains one line YAML reads differently from how it
  looks.** On 14 frontmatter blocks from
  `docs/proposals/kind-and-collection-format-2026-11-24.md`, the own parser and
  `yaml-rust2` agree on 13. In a space profile,
  `[calternal-money.transactions]: change manager` is, in YAML, a **list used as a
  key**; the own parser reads it as text. Either the line needs quotes or the
  format is not YAML and should say so.
- **`serde_json` is a fifth of the engine** (48 of about 229 KB raw), almost as
  much as the engine's own code.
- **SQLite:** the official 3.54.1 package from sqlite.org (its SHA3-256 matches the
  download page): `sqlite3.wasm` 879 KB raw, 407 gzip, 353 brotli, built with
  `ENABLE_FTS5` (also R-Tree, sessions, maths). Its loader `sqlite3.mjs` is 439 KB
  raw and 78 brotli as shipped, 222 and 60 after minifying. It was inspected, not
  run.

## Not measured

From item 1 of the study's list: **each missing formula feature**, **the line
scanner against `pulldown-cmark`** (and the 70 line vectors), **the hash**, and an
**own repeat subset** to set against `rrule` (only "`rrule` against none" was
asked for here). Items 2 to 8 of that list (loading, boundary per gesture, a real
iPhone, Apple builds, disagreements, search, records) were not part of this job.
The own parser was compared with `yaml-rust2` only on those 14 blocks, which is
not a test of a parser. No build was run to see whether a module still works in a
browser: these are link-and-measure builds.

## Simplification opportunities

- Use the own strict parser and drop the general YAML reader: 23 KB against 2.6,
  and one reading of each file.
- If patterns are needed at all, `regex-lite` (19 KB) instead of `regex` (180),
  accepting that it has no Unicode classes and, by its own documentation, is slower
  (not measured here).
- Civil-time repeats without a time zone database: do not link `rrule`, and ship
  `jiff`'s database only if something truly needs named zones on the device
  (64 KB).
- Replace `serde_json` at the boundary with a small hand-written or binary format
  once the boundary's shape is fixed; it is the largest single thing in the engine
  that is not the engine.

## How to repeat it

On the dev VM, in the spike directory, with the format proposal copied to
`sizing/data/kind-format.md`: `nix develop path:. -c harness/build-sizing.sh`
(about 6 minutes), then `node harness/sizing-summary.mjs out/sizing`.
