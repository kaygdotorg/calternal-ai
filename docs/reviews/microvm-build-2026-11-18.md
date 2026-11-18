# Per-person microVM build proof

## Stage 1

The dev-01 build produced a digest-pinned read-only root with a normalized
OCI filesystem. Podman uses the filesystem as its overlay lower layer.
Its private uppers and the journaled home share the bounded account disk.
The machine opens KVM as its service user, verifies API 12, and creates a
VM before selecting Firecracker. A nested Proxmox guest can exercise the
same check. Podman fallback advertises `sandbox_fallback: "kvm_unavailable"`
through machine readiness.

- `nix develop .#microvm -c deploy/microvm/build-image.sh ...` built the guest.
  The first build exposed a Nix builder restriction on upstream set-ID modes.
  Rootless OCI unpack outside the derivation and a normalized archive resolved it.
- With `CALTERNAL_MICROVM_TEST_CONFIG` pointing to the 512 MiB guest,
  `scripts/check-podman-tests.sh cargo test --locked -j6 -p calternal-machine --lib sandbox::microvm -- --nocapture`
  reported five passes and the perf-only benchmark ignored. The live test
  covered separate boot IDs, thread root overlays, retained files after crash,
  and erasure. A readable non-KVM device was rejected.
- `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal` and the same guest config
  were used for the serial conformance filter `microvm_cases`. Both cases
  passed, including the entitlement check for container fallback readiness.
  Both commands passed the Podman and Firecracker leak checks.

## Stage 2

The 512 MiB profile declares two active threads in the machine protocol.
The server accounts for them across spaces and releases a place when a
lease closes. The machine checks the same profile before allocating local
work. The third thread sees the existing waiting state and queue position.
The test grants the pool to two accounts and checks that the other account
can still start while the first is waiting. The 1,024 MiB profile has no
measured per-account cap yet.

On dev-01 the plugin build, test-plugin server build, all-features
workspace tests, integration, plugins, GenUI, contract, docs, publication,
and formatting gates passed. The serial conformance rerun passed 117 cases
with the live KVM case visibly skipped when no image was configured. An
explicit 512 MiB image run under the leak checker passed all three
microVM conformance cases in 12.23 seconds. An earlier traced run also
passed. Two preceding live attempts failed with ENOENT at sandbox start.
A later reproduction traced the same symptom to a garbage-collected ext4
formatter path. The machine-level live suite passed five cases and ignored
the perf-only benchmark. Both leak checks passed.

## Stage 3

The operator installs one IPv4 policy in a dedicated firewall namespace.
The machine accepts its root-owned installation receipt only for the
current host boot and network namespace. It parses the policy once at
initialization and attaches one leased TAP to each networked VMM. The
microVM's rootless thread container uses its guest-local user-mode network.
Worker containers and the KVM fallback have no network. The role needs
access to KVM and its assigned TAPs. Firewall administration and namespace
setup stay with the operator.

On dev-01, `python3 deploy/microvm/test-egress.py` passed both policy
compiler tests. `tests/support/microvm-egress.sh VM_CONFIG MACHINE_TEST_BINARY`
passed with the real KVM guest under the leak checker. The thread resolved
an approved DNS record and opened its declared TCP service. Unlisted TCP,
the host gateway, the peer TAP route, the operator-denied control plane
address, and metadata were denied. nftables counters confirmed the peer
route and unsolicited ingress drops. The native suite tests explicit
missing policy and missing receipt. A separate mount namespace covering
`/dev/kvm` with `/dev/null` verified that an unusable KVM device produces the
visible isolation skip.

The negative tests used synthetic upstream services. A separate dev-01
run used the same guest image and firewall installer with real IPv4
egress. The installed policy allowed the resolved addresses for Anthropic
and the npm registry only, plus its configured DNS resolver. It returned
HTTP 200 from the npm registry. Official Claude Code 2.1.326 completed a
one-answer turn (3.04 seconds) and a coding turn (7.59 seconds) in rootless
thread containers. The coding turn read the fixture modules, corrected a
bug and passed the Node test suite, which the harness reran independently.
Both completed with zero cgroup OOM kills and no guest swap. These are
development-host observations. The fixture provisioned the Firecracker TAP
directly and exercised the shipped guest agent and container launcher.

The ordinary dev-01 backend gate passed all ten checks, with 117 serial
conformance cases and a visible skip for the hardware fixture when no
guest config was supplied. With the absolute guest config, the machine
microVM suite passed eight cases, with the perf benchmark ignored, and
all three filtered HTTP conformance cases passed. Both leak checks passed.

## Stage 4

The HTTP fixture now verifies that two accounts have different boot IDs
while their sibling threads share only their account's kernel. It kills the
first account's exact Firecracker process during a leased command, waits for
the interrupted state and calls `machines.resume`. It checks foreign resume
is refused and the owner sees the retained workspace with a different boot
ID. It then deletes the account through the public account operation and
checks that the durable machine erasure queue completes and the disk is
removed. The untouched account keeps its original boot ID. The capacity
fixture also cancels a waiting lease and checks that it never gets assigned.

The dev-01 command used `scripts/check-podman-tests.sh cargo test --locked
-j6 --manifest-path tests/conformance/Cargo.toml --test isolation microvm_cases --
--test-threads=1 --show-output` with the absolute stage 3 guest configuration.
The run after merging `45fa2c0` passed all three cases in 17.23 seconds.
Both leak checks passed. This exercises the retained workspace behind the
provider checkpoint flow.
The full provider resume example in ordinary conformance uses its separate
fixture.

## Release artifact checks

The first locked perf attempt stopped at KVM probing. The dev-built static
binary reproduced the failure under `strace -f -e trace=ioctl` on dev-01:
`KVM_GET_API_VERSION(0x1)` returned `EINVAL`. Passing an explicit zero in
both variadic KVM calls returned API version 12 and created a VM.

A separate `strace` reproduction traced the sandbox start `ENOENT` to
`execve` of a garbage-collected Nix ext4 formatter. The deployment command
now roots a dedicated Nix runtime profile. With that profile and the fixed
static test binary, the live isolation and lifecycle case passed on dev-01.
The Podman and Firecracker leak checks passed. A restricted PATH test also
confirmed that the Firecracker check still detects a fixture leak when the
host has no Podman.

The fixed native suite passed eight machine cases and ignored the
perf-only benchmark. All three live HTTP conformance cases passed
in 16.76 seconds. Both leak checks passed. The full backend run after the
`45fa2c0` merge passed the plugin build, test-plugin server build,
workspace, integration, plugin, GenUI, contract, docs and publication
checks. Ordinary conformance passed 119 of 120 cases. The failing
`filesystem_modes_and_complete_sql_inventory` case found mode 0755 on
`userdata-ssd/spaces` and one space directory. The newly merged Notes
rebuild calls the directory-creating `Folder::open` on its read path.
This was reported for the search lane to resolve.

## Locked performance run

The artifact was built on dev-01 and run on `calternal-perf-02-backup01`
with a 512 MiB guest, two vCPUs, a 4 GiB ext4 account disk and one warm
guest. Firecracker was 1.16.1. The host's existing ext4 formatter was
1.47.2. The TAP and nftables policy ran in a private namespace with a
synthetic upstream. Timing ends with completion of a `true` command.
The new MicroVM run ended in 140.76 seconds and left no scoped
Firecracker processes or scratch directories.

```sh
# Development VM
nix develop -c npm run build:plugins
nix develop .#deploy -c cargo test --locked -j6 --target x86_64-unknown-linux-musl \
  -p calternal-machine --lib --no-run --message-format=json

# Perf-02, in ~/calternal-ai-microvm-51/stage5 with the dev-built image and test binary
/usr/local/bin/perf-lock.sh env PATH="/usr/sbin:/sbin:$PATH" \
  ./tests/support/microvm-perf.sh config.json ./calternal-microvm-bench-fixed
```

| Metric, milliseconds | Samples | Current Firecracker p50 / p95 | Earlier host Podman p50 / p95 |
| --- | ---: | ---: | ---: |
| Cold first thread | 10 | 1,039.85 / 1,104.98 | 609.25 / 639.00 |
| Second thread in the same account | 10 | 129.44 / 153.47 | 558.18 / 604.65 |
| One command after startup | 300 | 85.50 / 100.36 | 298.99 / 320.93 |

One preboot took 4,824.01 ms from role initialization, and one warm pickup
took 169.81 ms. One wake after the idle sweep took 1,010.36 ms. The harness
advanced the account's idle timestamp to trigger the production sweep and
then timed the restart from the persisted disk. Warm pickup met the 2 s
target in this observation.

With 3,661.95 MiB initially available, the 8 GB host admitted 17 idle
accounts before the configured 1 GiB host reserve plus one 512 MiB guest
reservation prevented the next admission. Across five settled samples,
the host available-memory delta averaged 127.89 MiB per account and VMM
PSS averaged 128.52 MiB. Both were below the 150 MiB idle target. The
earlier host Podman run reached its configured cap of 20 at 14.02 MiB
per account. Its results are the earlier locked control from the same
host, so the table compares separate runs. Raw current results are in
`~/calternal-ai-microvm-51/stage5/perf-512-tap-fixed.log`.

### Ten idle wakes on perf-02

The follow-up used a dev-built artifact with the same 512 MiB guest
configuration, TAP firewall and locked perf host. The test repeated the actual
idle sweep and restart on the same persisted account disk, advancing the
idle timestamp before each sweep. Timing starts after the old guest stops
and ends when the first `true` command completes. It contains neither the
idle interval nor the sweep's wait. The test completed in 265.83 seconds.

| Samples | Minimum | Median | p90 | Maximum | Under 1 s |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 950.12 ms | 998.47 ms | 1,009.82 ms | 1,019.26 ms | 5 |

The idle-wake budget is **about one second, p90 under 1.1 s**. The locked
512 MiB benchmark enforces the p90 limit across ten wakes.

Samples in run order, in milliseconds: 1,019.26, 986.41, 993.02, 963.38,
1,009.82, 1,006.17, 1,002.73, 950.12, 994.21, and 1,006.10. Percentiles
use nearest rank, so p95 is the maximum at this sample count. The same
locked command above used `./calternal-microvm-bench-wakes` as its binary.
Raw output is in
`~/calternal-ai-microvm-51/stage5/perf-512-ten-wakes.log`.

The executable p90 gate passed on a separate locked perf-02 run. Ten
wakes measured a median of 1,000.86 ms, p90 of 1,004.61 ms and a range
of 957.14–1,008.21 ms. The locked command used the dev-built
`./calternal-microvm-bench-wake-gate` binary and completed in 267.03 seconds.
Raw output is in
`~/calternal-ai-microvm-51/stage5/perf-512-wake-gate.log`.

### 1,024 MiB concurrent Claude Code turns on dev-01

The development-host run used a fresh 1,024 MiB Firecracker guest for each
trial, two vCPUs, a 4 GiB disk, the pinned guest image and the installed
TAP policy. It reused the earlier 24-module coding fixture. All threads
in each trial started together. A completed turn required successful CLI
exit, four or more file-read calls, the expected test command and a passing
independent Node test rerun. The table combines three separate trials at
each concurrency. Turn time runs from Claude Code launch to exit inside
the thread container. Validation runs afterward. The dev host also built
the static wake benchmark during the early trials. The conformance build
started after the last trial.

| Active threads | Completed | Median turn | p90 turn | Median vs one thread | Peak VM PSS range | Settled VM PSS range | Per-container peak / settled ranges |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 3/3 | 14.85 s | 15.51 s | Reference | 494.20–498.23 MiB | 494.19–498.23 MiB | 339.98–342.39 / 177.71–178.79 MiB |
| 2 | 6/6 | 14.68 s | 15.49 s | −1.18% | 667.97–688.22 MiB | 667.97–688.21 MiB | 245.33–264.54 / 85.68–106.19 MiB |
| 3 | 9/9 | 16.43 s | 19.77 s | +10.64% | 835.17–850.54 MiB | 835.17–850.53 MiB | 218.38–242.52 / 62.34–79.53 MiB |
| 4 | 12/12 | 18.82 s | 21.99 s | +26.72% | 974.60–982.26 MiB | 974.60–982.24 MiB | 179.74–230.07 / 23.58–70.69 MiB |
| 5 | 15/15 | 24.69 s | 27.14 s | +66.28% | 980.15–986.64 MiB | 980.15–986.64 MiB | 177.27–225.65 / 17.85–52.48 MiB |

All 45 turns completed with zero observed guest or container OOM kills and
zero swap activity. The harness's literal `return total;` marker matched
41 turns. The other four used the edit tool and passed the same independent
test rerun. Two is the highest tested count within a 10% median
latency increase over this run's one-thread reference. Three completed
every turn and measured 10.64% above the reference. Four and five
completed with higher latency and peak VM PSS above 974 MiB. These
observations support a cap of two if the earlier 10% criterion applies.
The owner approved two active threads for the 1,024 MiB profile for now.
Three was at the cutoff on the busy development host.
The dev-01 harness and sanitized raw metrics are in
`~/Developer/lanes/microvm/cap1024/`.

### 1,024 MiB CPU matrix on locked perf-02

The matrix reused the 24-module Claude Code workload, pinned guest image,
4 GiB account disk and TAP policy. A fresh guest ran each trial. The
harness checked the vCPU count inside the guest before launching two,
three or four concurrent threads, then measured from CLI launch to exit.
It started the fixture threads through the guest agent so it could test
counts above the production admission cap. Three trials ran per cell,
with the CPU and thread order varied between repetitions.

The edit check compares the source bytes before and after the turn. It
also checks that the test file stayed unchanged and reruns the tests
independently. All 81 turns met those checks, including 43 that used the
Edit tool. All 27 guests recorded zero OOM kills and zero swap activity.

| vCPUs | Threads | Turns passed | Median turn | p90 turn | Peak VM PSS |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 2 | 6/6 | 12.50 s | 13.37 s | 685.02 MiB |
| 1 | 3 | 9/9 | 14.93 s | 17.11 s | 844.72 MiB |
| 1 | 4 | 12/12 | 20.31 s | 21.74 s | 980.75 MiB |
| 2 | 2 | 6/6 | 11.10 s | 12.76 s | 673.02 MiB |
| 2 | 3 | 9/9 | 12.83 s | 15.80 s | 853.65 MiB |
| 2 | 4 | 12/12 | 35.72 s | 41.07 s | 977.04 MiB |
| 4 | 2 | 6/6 | 11.02 s | 13.18 s | 687.09 MiB |
| 4 | 3 | 9/9 | 12.13 s | 13.52 s | 853.06 MiB |
| 4 | 4 | 12/12 | 15.20 s | 21.52 s | 978.48 MiB |

At three threads, more vCPUs reduced observed latency. At four threads,
four vCPUs had the lowest median and every CPU setting peaked above
977 MiB of VMM PSS. The four-thread medians on two vCPUs were 25.11,
35.72 and 40.91 seconds across the three trials. The overall pattern
supports considering CPU and RAM together for plan sizing. This matrix
leaves the cause of the two-vCPU four-thread slowdown open.

On perf-02, `/usr/local/bin/perf-lock.sh bash cap1024/launch.sh` ran from
`~/calternal-ai-microvm-51/cpu1024` with the dev-staged harness, guest
artifact and rootless TAP setup. Raw metrics are in `matrix.log` beside
the corrected fixtures. The run left no scoped Firecracker processes or
account disks.

### Conformance with the Notes directory-mode fix

The lane merged search commit `1ddd88a` before the dev-01 rerun. The
focused `filesystem_modes_and_complete_sql_inventory` case passed.
The ordinary serial suite passed all 122 cases in 212.55 seconds, with
a visible skip inside the hardware fixture for the unset guest config.
Both the Podman and Firecracker leak checks passed. The rerun uses the
backend commands below and the absolute test-plugin server path.
With the absolute guest config, all three filtered live microVM cases
passed in 10.89 seconds and both leak checks passed again. Workspace,
integration, plugin, GenUI, contract, docs, publication and formatting
checks also passed.

### Worker teardown after the dev merge

The merge of dev at `a7cd5dc` keeps both the machine slot and the
initiating account's pool slot reserved while a worker is stopping. The
server releases both after an authenticated report proves teardown or
the machine is durably revoked. The regression test checks the counts
through a restart with the lease intact, rejects an insufficient ack
and a report for another machine, and checks revocation after the
lease has been removed from memory.

Owner erasure followed by a server restart before teardown is an open
case inherited from dev. Erasure deletes the lease row, leaving the
stop record in memory until proof. A restart then has no row to
restore. A minimal durable stop record could preserve the IDs needed
for accounting while dropping the erased lease details. It would add
a placement record retained until teardown or revocation. Retaining
the existing stopping lease would avoid a separate record and would
retain more owner data through that interval.

On dev-01 the three targeted server worker tests passed. The machine
library passed 21 tests with one ignored, and the workspace passed 611
with two ignored. The focused Notes directory-mode regression passed.
The serial suite passed all 127 conformance cases in 225.81 seconds,
including all four live microVM cases. Both leak checks passed.
Integration, plugin, GenUI, contract, docs, publication and both formatting
checks also passed. The regenerated machines reference page matches its
source.

```sh
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked -j6 \
  -p calternal-server --lib machines::workers -- --test-threads=1
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked -j6 \
  -p calternal-machine --lib
nix develop -c env CALTERNAL_MICROVM_TEST_CONFIG="$PWD/../stage3-test.json" \
  CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 \
  --test isolation -- --test-threads=1 --show-output
```

The focused Notes run used the same absolute server and target paths
with `filesystem_modes_and_complete_sql_inventory` as the test filter.

## Backend gate commands

Run these in the development VM lane. Each cargo build or test uses six jobs.
The gate runner prints the ordinary conformance skip reason and keeps each
command's output in `apps/docs/.cache/gates/`.

```sh
nix develop -c npm run build:plugins
nix develop -c cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6
nix develop -c env CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 \
  --test isolation -- --test-threads=1 --show-output
nix develop -c npm run test:integration
nix develop -c npm run test:plugins
nix develop -c npm test --workspace @calternal/genui
nix develop -c cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
nix develop -c sh scripts/check-docs.sh
nix develop -c node --test scripts/docs-site/publication.test.mjs
nix develop -c cargo fmt --all -- --check
nix develop -c rustfmt --edition 2024 --check tests/conformance/microvm_cases.rs
```

The supplementary serial KVM run exports `CALTERNAL_MICROVM_TEST_CONFIG`
as an absolute operator config path. It runs the same isolation command with
the `microvm_cases` filter and this machine-side case group.

```sh
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked -j6 \
  -p calternal-machine --lib sandbox::microvm -- --nocapture
```

## Review gate

| Item | Verification |
| --- | --- |
| 1. Shared types | Uses existing account, space and thread IDs, machine lease and output types, and readiness contracts. |
| 2. Dependencies | Uses dependencies already available to the existing machine and server crates. New operator tooling uses Python's standard library and nftables. |
| 3. Lock scope | The server uses the existing installation-wide machine registry. One account's guest lock spans disk binding and release to keep a single disk owner. TAP and account maps hold short in-memory locks. |
| 4. Async I/O | Host file and process operations use Tokio. Policy verification, image hashing, directory sync and durable rename run on blocking tasks. |
| 5. Reads and flushes | Warm exec uses one authenticated vsock channel with no explicit host filesystem reads or flushes. The capacity check adds in-memory lookups and no SQL. Disk binding flushes the new disk and affected directories. |
| 6. Constant work | The role verifies and loads the immutable image and network policy at startup. Slots are in memory. The size table is constant. |
| 7. Bounds | The writable disk is capped per account, the config bounds guest count and the warm pool has at most two entries. Image operations retain one active and one rollback root. Account erasure removes its durable disk. |
| 8. Compatibility | The pool uses the current machine protocol. There are no database migrations or alternate image readers. |
| 9. Space identity | Space homes, account disks and thread workspaces use the existing UUID identities. |
| 10. Single declaration | Each size profile declares its active-thread limit. The machine and server both read it. One policy supplies DNS, allowed destinations and TAP slots. |
| 11. Callers | Machine CLI selects the backend. The server uses its existing admission, readiness and erasure paths. |
| 12. Reuse | Uses the existing queue and lease fence, durable fsio rename and directory sync, provider-home logic, and Podman exec. |
| 13. Public docs | The docs gate checks Cargo and TypeScript references. Public configuration fields document units, bounds and startup handling. |
| 14. Documentation | Deployment covers rollback, image updates, KVM access and operator-managed firewall policy. Live proof is recorded here and on issue 51. |
| 15. Formatting and size | The machine workspace and the changed conformance fixture have explicit formatting gates. Capacity accounting moved out of the server's state module. |

## Simplification opportunities

Keep the immutable root shared across all guests and ship a new image as one
operator update. The writable account disk and existing lease queue are
sufficient for workspace recovery and admission. The firewall receipt lets
the machine role open its assigned TAPs without running a firewall manager.
Installing the provider adapter under the standard executable path could
remove the extra PATH entry from both the OCI image and guest launcher.
