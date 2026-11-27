# A JavaScript interpreter inside the shared core: what it costs · 2026-11-27

Lane `lane/engine-wasm-vs-ts` (issue #108). Measurement for the owner question
**O1** (may a plugin ship small pure functions that run on devices with no I/O?),
`docs/proposals/extension-surface-2026-11-27.md` §4.2 and §5.1 on `dev`. Nothing
here is product code. Code, raw results and the full tables are in
`spikes/engine-wasm-vs-ts/` (`rust/interp/`, `harness/`, `results/interp/`).

**All timings are development-host timings** (`calternal-dev-01-compute02`, shared
x86_64 VM). They are not a benchmark pass and not a device measurement. WebKit is
the Linux WPE build (WebKit 26.6), not Safari and not an iPhone. "Native" is
x86_64 Linux standing in for the native apps. Sizes are exact; **KB = 1,000 bytes**.

## The short answer

- **QuickJS costs about 230 KB more download (brotli), not a megabyte** (about
  305 KB if the module is built for speed). Boa costs 645 to 950 KB.
- **A QuickJS plugin function costs about 0.4 µs per item** in Node and WebKit
  (0.17 µs when it takes only numbers): a 10,000-item column in about 4 ms,
  100,000 items in about 37 to 39 ms. That is 2.5 to 4 times the engine's own
  formula language and about 17 to 18 times the same function written in Rust.
- **QuickJS's step budget stops the endless loops tried**, including ones that
  catch errors, loops inside built-ins and a runaway regular expression. The one
  exception is a loop whose every step is slow (below).
- **QuickJS's memory limit does not work in Wasm as shipped.** With a 16 MB limit
  an allocation bomb grew the Wasm memory to **1.2 GB in 3 seconds**. About 65
  lines of our own allocator fix it (held at 18 MB). A step budget alone also does
  not bound time (one script ran 15 s natively inside its budget), so a deadline
  is needed too; that is a few more lines and was tested.
- **Boa should not be used.** It is three times the added size, 1.7 to 4 times
  slower per call,
  three of the hostile scripts run past its limits, it has no memory limit, and
  the only cap available (refusing allocations in the host) **kills the whole
  core** in Wasm and the whole process natively.
- **A correction to the audit:** QuickJS is C. "A memory-safe interpreter … inside
  the Rust core on iOS" is not what it would be: in the browser an escape stays
  inside the Wasm sandbox; natively it would be memory corruption in the app.

## What was built

The interpreter is a feature of the spike's Wasm engine crate; each candidate is
one build. **No host function is registered with either interpreter.**

| | QuickJS | Boa |
|---|---|---|
| Version | QuickJS-ng 0.16.2 through `rquickjs` 0.14.0 | `boa_engine` 0.22.0, default features off |
| Written in | C | Rust |
| `wasm32-unknown-unknown` | Builds unmodified: the crate ships its own C library build (`libc.a`) and a small shim. The dev shell needed a clang that targets Wasm and `llvm-ar` | Builds after choosing a random-number backend (a compiler flag and 5 lines) and supplying a clock (10 lines) |
| What the module imports from the page | **One function: a clock** (`__rquickjs_host_now_us`), used by the C shim, not visible to scripts | Nothing |
| Built-ins given to scripts | Chosen: no `Date`, `Proxy`, `Promise`, `WeakRef`, `performance` | All of the default set (`Date` returns 0 with our clock) |
| Wrapper code (lines, no blanks or comments) | 219, of which the capped allocator is 64 | 106 |

Neither gives a script `fetch`, `XMLHttpRequest`, `WebSocket`, timers, `console`,
`require`, `process` or `WebAssembly`. Both give `eval` and `Function`, which
reach nothing outside the interpreter. **`Math.random` exists in both and must be
removed or seeded**: in QuickJS it differs from run to run (it is seeded from the
clock), so a device and the server would disagree. Boa is repeatable here only
because our random backend returns zeros.

The plugin module is 2.3 KB (`rust/interp/src/module.js`): a custom formula
function (`score`: done, hours × the project's rate, a late penalty, a bonus if
the title contains "a", rounded to cents), a field check and a merge. The formula
function was run over the spike's task items five ways, and **all gave the same
sum to the last bit** at every size: Rust in the engine, the engine's own formula,
each interpreter called once per item, each called once for a whole column, and
(in the browser) the page's own JavaScript.

## Size

| Build | Engine alone: raw / brotli | + QuickJS: added raw / brotli | + Boa: added raw / brotli |
|---|---:|---:|---:|
| Wasm, size (`-Oz`, then `wasm-opt -Oz`) | 203 / 82 KB | +598 / **+233** KB | +2,866 / +645 KB |
| Wasm, speed (`-O3`) | 385 / 103 KB | +1,131 / +305 KB | +6,851 / +952 KB |
| Native x86_64 binary (`-O3`, stripped) | 679 / 272 KB | +1,280 / +472 KB | +8,006 / +1,808 KB |

gzip for the Wasm builds (size / speed): engine 95 / 136 KB, with QuickJS
368 / 533 KB, with Boa 1,054 / 1,900 KB. The size build of QuickJS is 25 to 45%
slower per call than the speed build.

## Cold start

Median of 15 fresh processes (Node, native) or fresh pages (WebKit), ms.
"Instantiate" includes compiling the module; downloading is separate (the sizes
above). WebKit's clock steps by 1 ms.

| | Instantiate | Create a context and load the 2 KB module | First call |
|---|---:|---:|---:|
| Node, engine alone (size / speed) | 4.4 / 2.0 | – | – |
| Node, QuickJS (size / speed) | 3.7 / 4.2 | 12.7 / 16.4 | 0.5 / 0.9 |
| Node, Boa (size / speed) | 9.7 / 16.7 | 25.9 / 37.0 | 1.1 / 1.6 |
| WebKit, engine alone (size / speed) | 8 / 9 | – | – |
| WebKit, QuickJS (size / speed) | 11 / 13 | 7 / 6 | < 1 |
| WebKit, Boa (size / speed) | 17 / 25 | 15 / 13 | 1 |
| Native, QuickJS | – | 0.43 | 0.02 |
| Native, Boa | – | 1.16 | 0.03 |

Once warm, creating another context and loading the module takes 0.39 to 0.43 ms
(QuickJS in Wasm; 0.29 ms native) and 0.9 to 1.3 ms (Boa; 0.71 ms native). The
first creation in Node is slow, most likely because V8 compiles the
interpreter's code when it is first used.

## Warm calls

Median ms for the whole list (95th percentile in brackets), speed builds. The
function takes a text argument (the title).

**Node (V8)**

| | 1,000 | 10,000 | 100,000 |
|---|---:|---:|---:|
| Written in Rust, in the engine | 0.0079 (0.0089) | 0.203 (0.263) | 2.15 (2.85) |
| The engine's own formula | 0.106 (0.124) | 0.942 (1.19) | 10.8 (12.5) |
| **QuickJS, one call per item** | 0.357 (0.392) | **3.68** (4.72) | **36.6** (40.6) |
| QuickJS, one call per column | 0.408 (0.471) | 3.97 (5.02) | 43.5 (54.1) |
| Boa, one call per item | 1.14 (1.26) | 11.4 (13.9) | 115 (129) |
| Boa, one call per column | 2.23 (3.33) | 24.4 (30.4) | 326 (370) |
| The page's own JavaScript, no sandbox | 0.013 (0.016) | 0.191 (0.256) | 3.64 (4.49) |

**WebKit 26.6 (WPE, dev VM)**

| | 1,000 | 10,000 | 100,000 |
|---|---:|---:|---:|
| Written in Rust, in the engine | 0.0080 (0.011) | 0.209 (0.231) | 2.25 (2.75) |
| The engine's own formula | 0.144 (0.168) | 1.40 (1.67) | 14.5 (16.5) |
| **QuickJS, one call per item** | 0.357 (0.411) | **3.67** (4.50) | **39.0** (44.0) |
| QuickJS, one call per column | 0.478 (0.587) | 4.60 (5.20) | 48.0 (58.0) |
| Boa, one call per item | 0.800 (0.960) | 8.00 (9.33) | 78.0 (83.0) |
| Boa, one call per column | 1.73 (2.18) | 15.0 (16.5) | 155 (163) |
| The page's own JavaScript, no sandbox | 0.024 (0.048) | 0.264 (0.292) | 4.17 (4.83) |

**Native x86_64**

| | 1,000 | 10,000 | 100,000 |
|---|---:|---:|---:|
| Written in Rust | 0.0065 (0.0072) | 0.162 (0.182) | 1.72 (2.00) |
| The engine's own formula | 0.073 (0.083) | 0.709 (1.09) | 8.29 (9.46) |
| **QuickJS, one call per item** | 0.269 (0.309) | **2.77** (3.33) | **26.9** (29.1) |
| QuickJS, one call per column | 0.325 (0.366) | 3.06 (3.50) | 33.5 (35.1) |
| Boa, one call per item | 0.462 (0.538) | 4.59 (5.68) | 44.9 (47.8) |
| Boa, one call per column | 0.742 (1.09) | 7.13 (8.16) | 71.4 (77.2) |

Without the text argument, QuickJS per item takes 1.65 ms (Node), 1.73 ms (WebKit)
and 1.33 ms (native) for 10,000 items, and 16.5 / 17.5 / 13.2 ms for 100,000: **about
half the cost of a call is turning the title into a JavaScript string.**

What that means for a person: a plugin's formula column over 1,000 items is
unnoticeable (0.4 ms), over 10,000 it uses a quarter of a screen frame (4 ms), over
100,000 it is two to three frames and belongs in a background worker, as the
heavy built-in query already does at that size. **Passing whole columns in one call
is slower than calling once per item** in every case measured, so the simple design
is also the faster one. Putting QuickJS on the capped allocator changes call times
by less than 10% (10,000 items: 3.58 ms Node, 3.67 ms WebKit, 2.63 ms native).

## Memory

| | QuickJS | Boa |
|---|---:|---:|
| One context with the module, Wasm (bytes the context holds) | about 70 KB (counted by the capped allocator) | about 200 KB |
| Wasm memory growth per context, 50 contexts | about 160 KB | about 190 KB |
| One context, native | 99 KB (53 KB for a further one sharing a runtime) | 251 KB (241 KB each further one) |
| Garbage held after calling once per item over 100,000 items (Node / WebKit / native) | none: flat | **111 / 157 / 330 MB** until a collection runs |

QuickJS frees as it goes; its memory did not grow with the list. Boa collects
lazily: the Wasm memory reached 149 MB (Node) and 198 MB (WebKit) in the Boa
builds against 52 MB in the QuickJS build running the same lists, and Wasm memory
never shrinks. QuickJS's own memory counter under-reports in Wasm (see below), so
the Wasm figures come from the capped allocator and from linear-memory growth.

## Stopping hostile scripts

Eleven scripts, run with a 16 MB memory limit; "not stopped" means still running
after 20 s (the test process was then killed). Times are for Wasm in Node unless
marked. QuickJS's budget was 100 interrupt polls (one per about 10,000 jumps or
calls); Boa's was a million loop iterations per function call, and separately its
"sliced" evaluation given 100 slices of 10,000 cost units.

| Script | QuickJS as shipped: budget + its memory limit | QuickJS: budget + our capped allocator | QuickJS: capped allocator + 2 s deadline | Boa: loop limit + allocation cap | Boa: sliced + allocation cap |
|---|---|---|---|---|---|
| endless loop | stopped, 7 ms | stopped, 10 ms | stopped, 2 s | stopped, 53 ms | stopped, 10 ms |
| endless loop inside try/catch | stopped | stopped | stopped | stopped | stopped |
| endless work spread over many calls | stopped, 30 ms | stopped | stopped | **not stopped** | stopped |
| endless loop inside a built-in callback | stopped | stopped | stopped | stopped | **not stopped** |
| endless recursion | stopped (stack) | stopped | stopped | stopped | stopped |
| long built-in with no script loop | stopped, 36 ms | stopped | stopped | **not stopped** | **not stopped** |
| runaway regular expression | stopped, 36 ms | stopped | stopped | **not stopped** | **not stopped** |
| allocation bomb (arrays) | **not stopped: 1.2 GB after 3 s** (native: stopped, 18 ms) | stopped, out of memory, 43 ms | stopped | **the instance dies** | **dies** |
| allocation bomb (string doubling) | stopped by the string length limit | stopped | stopped | **dies** | **dies** |
| allocation bomb that catches its errors | **not stopped** (native too, 20 s) | stopped after 0.9 s (native: 15 s) | stopped, 2 s | **dies** | **dies** |
| one huge allocation | refused | refused | refused | refused | refused |

- After a budget or deadline stop, the same QuickJS context runs the next call
  correctly. After a memory stop in Wasm the script's own data still fills the
  context: it has to be dropped, and **a fresh context in the same instance works**.
- Boa "dies" means: in Wasm the instance traps and cannot make a new context, so the
  engine and its loaded items are lost with it; natively the process aborts.

**The QuickJS memory defect, exactly.** `js__malloc_usable_size` in the bundled
`quickjs/cutils.h` returns 0 on any target it does not name, and
`wasm32-unknown-unknown` is not named. QuickJS adds that figure to its running
total, so the total barely moves and the limit never fires. Giving the runtime an
allocator that records each block's size (`Runtime::new_with_alloc`) restores
both the count and the limit. This is worth reporting upstream; I did not.

**The budget's gap.** The budget counts jumps and calls, not time, and one step can
be slow (a script that keeps hitting the memory limit and catching it ran 15 s
natively within its budget). The interrupt handler can also check a deadline; in
Wasm it uses the one clock import the module already has.

## What is measured and what is not

Measured: everything in the tables above, on the dev VM.

Not measured, and any statement about them is a guess:

- Safari or a physical iPhone, Android, ARM64, or any timing outside this VM.
- The interpreter as a **second Wasm module loaded only when a plugin needs it**,
  which would avoid the download for everyone else but needs the columns copied
  between modules.
- Precompiling a module to bytecode to shorten loading.
- A module that allocates heavily per item (the merge function was written but not
  timed), or many different modules at once.
- Security: no fuzzing and no review of either interpreter's escape history. The
  hostile list is eleven scripts I wrote, not a security test.
- App Review's reaction (the audit's open point).
- The server side beyond the native x86_64 figures.

## Recommendation

If O1 is answered yes, **use QuickJS, with three things required from the first
line**: our capped allocator, a deadline beside the step budget, and `Math.random`
removed from the context. Call it once per item. Budget about 230 KB of download
and 70 to 160 KB of memory per plugin context. Because a pure module runs on the
server too, run the same QuickJS there (as the audit proposes). On iOS, decide
knowingly that a C interpreter is inside the app process; the audit's fallback
(Apple's own JavaScriptCore) avoids our C code but brings a second engine that must
agree.

The cost that remains to be judged is the 230 KB: it is nearly three times the
engine it sits in (82 KB). If only formula functions and field checks are wanted
at first, a lazily loaded second module is the thing to measure next.

## Simplification opportunities

- One call per item is both the simpler and the faster design: do not build a
  column-passing interface.
- One context per plugin, dropped and recreated after any limit stop: no state to
  repair, 0.4 ms to rebuild.
- Give modules only the built-ins they need (no `Date`, `Promise`, `Math.random`):
  less to agree on between device and server.
- Many plugin formulas could instead be new built-in engine functions: those are 2.5
  to 4 times faster, need no interpreter and no extra download. The interpreter
  earns its place only for logic the formula language cannot express.

## How to repeat it

On the dev VM, in the spike directory: `nix develop path:. -c harness/run-interp.sh`
(about 20 minutes), then `harness/interp-extra-limits.sh`,
`node harness/interp-summary.mjs`.
