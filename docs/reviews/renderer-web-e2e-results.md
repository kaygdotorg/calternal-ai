# Web renderer browser suites (2026-11-19)

Commit `91136c53` on calternal-dev-02-std (a shared VM: timings are recorded, not judged; judge on the perf VMs).

| suite | pass | fail | exit |
|---|---|---|---|
| keys | 12 | 0 | ok |
| fixtures | 1 | 0 | ok |
| components | 9 | 0 | ok |
| actions | 18 | 0 | ok |
| bulk | 7 | 0 | ok |
| reading | 7 | 0 | ok |
| images | 3 | 0 | ok |
| perf | 8 | 0 | ok |
| scale-notes | 8 | 0 | ok |
| scale-threads | 8 | 0 | ok |

## Performance and scale lines

- perf: 1x | 5000 rows | first rows painted 22 ms after view open (open 0 ms, 5 window requests) | key handler p95 3.8 ms | key to painted selection p50 29.0 p95 49.3 max 55.7 ms | heartbeat gaps over 16 ms: 70, max 111 ms | 2 window requests in 5 s
- perf: 4x | 5000 rows | first rows painted 250 ms after view open (open 0 ms, 5 window requests) | key handler p95 11.7 ms | key to painted selection p50 19.3 p95 50.0 max 56.4 ms | heartbeat gaps over 16 ms: 133, max 195 ms | 2 window requests in 5 s
- scale-notes: notes | 1x | 4994 rows | first rows 847 ms | handler p95 2.7 ms | key to painted p50 25.8 p95 43.0 max 46.3 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 107 | 25 window requests in 5 s | 27 result versions seen
- scale-notes: notes | 4x | 4994 rows | first rows 758 ms | handler p95 12.7 ms | key to painted p50 25.9 p95 52.7 max 57.1 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 115 | 24 window requests in 5 s | 28 result versions seen
- scale-threads: threads | 1x | 5000 rows | first rows 1183 ms | handler p95 4.2 ms | key to painted p50 24.2 p95 37.3 max 41.7 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 87 | 23 window requests in 5 s | 27 result versions seen
- scale-threads: threads | 4x | 5000 rows | first rows 809 ms | handler p95 11.4 ms | key to painted p50 30.2 p95 53.5 max 60.7 ms | keys with a skeleton on screen 0/150 | heartbeat gaps over 16 ms 106 | 23 window requests in 5 s | 25 result versions seen
