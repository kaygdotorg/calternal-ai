# Plugin worker development observations — 2026-11-17

Issue [#29](https://git.kayg.org/kayg/calternal-ai/issues/29), implementation
`1abe29a`, based on `origin/dev` at `25878bd`. These are **development-host
numbers, not a benchmark pass**. No perf VM or production system was used.

## Environment and method

Assigned shared development VM: `claude@calternal-dev-01-compute02`, x86_64,
12 available CPUs, Linux `6.12.116+deb13-amd64`, Podman `5.8.9`. Builds and
measurements used `nix develop`, Cargo `-j6`, and disposable data under
`~/Developer/lanes/plugin-workers`. Other lanes share the VM; no quiet-host or
load-isolation claim applies. A nearby load-average observation was 5.85/8.32/9.31.

The image was already provisioned; image pulls are excluded. Image config ID:
`b791832e17c3a8a5897bdb5087381ffbc07b77bc634d204bf603b5f63c1ffe57`.
Reported repository digests:

- `docker.io/library/node@sha256:116b800c3754b440ee0aa3ed2f20f5bf3d99a554bfd9adeea09ea8017f6e85f5`
- `docker.io/library/node@sha256:d8d97e479cdc64f052f81f129362168717c59783ade8dcc1abd22e1cca8dbd7c`

The adapter probe uses the same `Backend`/`Sandbox`/interactive `Exec` interfaces
as workers. Each backend starts five fresh workers and performs 100 sequential
warm exchanges per worker (500 total). Cold time includes staging, sandbox
creation, exec and the first protocol-v2-shaped echo response. It excludes server
registry/hash checks, HTTP, machine transport, schema validation and host calls.
Warm time is one newline-delimited invoke/complete round trip. Both use Node
permission flags, read-only code access, disabled string code generation and a
128 MiB V8 heap; Podman also uses the documented OS resource profile.

```sh
nix develop -c sh -c 'CALTERNAL_MEASURE_WORKERS=1 cargo test --locked -p calternal-machine -j6 --test plugin_workers development_worker_measurements -- --nocapture'
```

## Adapter results

| Observation | Direct development adapter | Podman adapter |
| --- | --- | --- |
| Node | v24.21.0 | v24.22.3 |
| Cold samples, ms (sorted) | 56.65, 69.21, 80.91, 85.29, 87.83 | 578.19, 663.91, 862.53, 926.55, 1010.07 |
| Cold median, ms | 80.91 | 862.53 |
| Warm p50, ms (500 calls) | 0.222 | 0.401 |
| Warm p95, ms (500 calls) | 0.375 | 1.896 |
| Idle memory after first worker's 100 calls | Node `VmRSS: 57404 kB` | Podman `MemUsage: 12.48MB / 268.4MB` |

The observed warm differences are +0.179 ms at p50 and +1.521 ms at p95;
the cold-median difference is +781.62 ms. Different Node patch versions, libc,
process environments and shared-host scheduling prevent attributing the whole
difference to containment. Memory accounting is **different**: direct reads one
process's RSS, while Podman reports container cgroup usage including its init and
idle process. Neither includes all host engine overhead; these values must not
be used as a like-for-like memory saving claim. The probe passed in 14.60 seconds.

## Full machine path

The remote conformance probe starts a real server and enrolled operator pool
machine. Its `jobs-demo.write` operation makes one authorized broker write.
Timing includes HTTP, authority/schema checks, machine transport, worker frames,
broker calls and durable content storage. This workload differs from the local
echo probe, so subtracting their timings would not isolate machine RPC overhead.

```sh
nix develop -c sh -c 'CALTERNAL_MEASURE_WORKERS=1 CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation remote_worker -- --test-threads=1 --nocapture'
```

| Observation | Result |
| --- | --- |
| First successful write, ms | 725.29 |
| Warm write p50, ms (10 calls) | 60.24 |
| Warm write p95, ms (10 calls) | 74.01 |
| Probe including death/hang/replacement checks | Pass, 9.08 seconds |

Placement was initially unavailable while the machine connected. The first-success
sample can follow such attempts and is not guaranteed to be a fresh container
start; the adapter probe above supplies the explicit cold samples. Server and
machine communicated over loopback on this VM; these are not WAN observations.

## Limits and qualification observations

The full gate checks 256 MiB memory/swap ceilings, one CPU's quota and 64 PIDs.
Separate probes observe kernel `memory.events` OOM kills and `pids.events`
refusals. They also deny socket creation, host appdata and foreign-worker reads,
and code writes without relying on Node permission flags. These are functional
isolation checks, not performance-budget qualification.

One earlier shared-VM container suite timed out a cold child-job invocation at
the existing three-second deadline (17/18); the unchanged suite then passed
18/18 standalone and in the complete gate. A final pressure-test run took
122.95 seconds, including slow Podman cleanup. Admission capacity remains
reserved until cleanup succeeds; it is not released after a timer.

## Simplification opportunities

- Measure the machine channel's existing output coalescing separately before
  changing its batching policy for interactive workers.
- A later controlled perf-host run can align Node builds and memory accounting;
  these development samples do not establish a production budget.
