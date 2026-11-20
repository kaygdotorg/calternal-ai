# Plugin split memory bench (experimental, not product code)

Measures what an extra plugin costs in a space and how far three layouts
reduce it: one process per plugin (today), start on first call with idle stop,
and one shared runtime with a worker thread per plugin. Results:
[docs/measurements/plugin-split-memory-2026-11-20.md](../../docs/measurements/plugin-split-memory-2026-11-20.md).
Forgejo #103.

- `guest/host.mjs`: supervisor stand-in inside the space microVM; fake in-guest broker.
- `guest/shared-runtime.mjs`, `guest/thread-boot.mjs`: the shared runtime (approach 3).
- `guest/runner.mjs`: serves a module that exports `handlers` (Threads bundles, fixtures).
- `guest/trivial.mjs`, `guest/faults.mjs`: trivial and misbehaving plugins.
- `pack.sh`: builds the guest image on dev-01 from built plugin checkouts.
- `bench.py`: perf VM driver (Firecracker); run under `/usr/local/bin/perf-lock.sh`.
- `summarize.py`: turns `results/*.jsonl` into the report tables.
- `smoke.sh`, `smoke-view.py`: correctness smoke on dev-01; its timings are not used.

Nothing here is wired into the product.
