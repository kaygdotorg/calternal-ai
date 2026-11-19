# Web renderer browser suites (2026-11-19)

Commit `fa718964` on calternal-dev-02-std (a shared VM: timings are recorded, not judged; judge on the perf VMs).

| suite | pass | fail | exit |
|---|---|---|---|
| keys | 12 | 0 | ok |
| fixtures | 1 | 0 | ok |
| components | 9 | 0 | ok |
| actions | 19 | 0 | ok |
| bulk | 9 | 0 | ok |
| reading | 7 | 0 | ok |
| images | 3 | 0 | ok |
| perf | 8 | 0 | ok |
| scale-notes | 8 | 0 | ok |
| scale-threads | 8 | 0 | ok |

## Performance and scale lines

- perf: 1x | 5000 rows | first rows painted 36 ms after view open (open 0 ms, 5 window requests) | key handler p95 2.4 ms | key to painted selection p50 27.3 p95 37.1 max 41.9 ms | heartbeat gaps over 16 ms: 59, max 47 ms | 2 window requests in 5 s
- perf: 4x | 5000 rows | first rows painted 185 ms after view open (open 0 ms, 5 window requests) | key handler p95 14.6 ms | key to painted selection p50 37.9 p95 56.4 max 66.1 ms | heartbeat gaps over 16 ms: 103, max 363 ms | 2 window requests in 5 s
- scale-notes: notes | 1x | 4988 rows | first rows 726 ms | handler p95 2.9 ms | key to painted p50 27.3 p95 36.4 max 46.7 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 37 | 25 window requests in 5 s | 27 result versions seen
- scale-notes: notes | 4x | 4988 rows | first rows 745 ms | handler p95 12.7 ms | key to painted p50 26.5 p95 47.6 max 62.3 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 113 | 25 window requests in 5 s | 29 result versions seen
- scale-threads: threads | 1x | 5000 rows | first rows 1045 ms | handler p95 2.1 ms | key to painted p50 23.0 p95 36.8 max 44.2 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 100 | 23 window requests in 5 s | 27 result versions seen
- scale-threads: threads | 4x | 5000 rows | first rows 729 ms | handler p95 10.3 ms | key to painted p50 31.0 p95 47.6 max 61.9 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 77 | 23 window requests in 5 s | 25 result versions seen
