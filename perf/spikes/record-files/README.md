# Record-files spike (#107)

Throwaway benchmark for the primitives-only host's record files: append, edit,
redact, read-last-N and unread-after-P on 1–256 MiB chat files, in layouts A
(tail rewrite), A2 (whole-file `fsio::write_atomic`), B (in-file slack) and C
(monthly segments). Not a workspace member; nothing here ships.

Report: [`docs/measurements/record-files-2026-11-21.md`](../../../docs/measurements/record-files-2026-11-21.md).

- `src/` — the benchmark (`bench`, `crashtest`, `scrubtest`; see `src/main.rs`).
- `count.py` — tallies strace syscalls between the benchmark's markers.
- `tables.py`, `compact.py` — Markdown tables from `results/`.
- `results/` — perf-01 output of 2026-11-20: `timings.txt`, `counts.txt`,
  `raw.tsv.gz` (every sample), `run-perf01.sh` (the exact run).

Build on a dev VM: `nix develop ../../.. -c cargo build --release`. Measure only
on a perf VM under `perf-lock.sh`.
