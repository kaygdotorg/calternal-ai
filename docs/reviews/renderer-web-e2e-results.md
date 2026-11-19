# Web renderer browser suites (2026-11-19)

Commit `4d98fc8f` on calternal-dev-02-std (a shared VM: timings are recorded, not judged; judge on the perf VMs).

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

- perf: 1x | 5000 rows | first rows painted 25 ms after view open (open 0 ms, 5 window requests) | key handler p95 7.1 ms | key to painted selection p50 25.3 p95 79.3 max 179.5 ms | heartbeat gaps over 16 ms: 116, max 187 ms | 2 window requests in 5 s
- perf: 4x | 5000 rows | first rows painted 126 ms after view open (open 1 ms, 5 window requests) | key handler p95 13.7 ms | key to painted selection p50 28.5 p95 56.2 max 62.3 ms | heartbeat gaps over 16 ms: 104, max 356 ms | 2 window requests in 5 s
- scale-notes: notes | 1x | 4974 rows | first rows 1093 ms | handler p95 3.0 ms | key to painted p50 32.1 p95 41.8 max 46.3 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 50 | 23 window requests in 5 s | 21 result versions seen
- scale-notes: notes | 4x | 4974 rows | first rows 1018 ms | handler p95 16.9 ms | key to painted p50 37.2 p95 57.4 max 70.1 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 57 | 20 window requests in 5 s | 21 result versions seen
- scale-threads: threads | 1x | 5000 rows | first rows 1534 ms | handler p95 2.1 ms | key to painted p50 29.6 p95 35.9 max 39.5 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 68 | 21 window requests in 5 s | 23 result versions seen
- scale-threads: threads | 4x | 5000 rows | first rows 1389 ms | handler p95 11.2 ms | key to painted p50 16.1 p95 47.0 max 59.9 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 84 | 23 window requests in 5 s | 24 result versions seen
