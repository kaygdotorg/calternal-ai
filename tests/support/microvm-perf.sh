#!/usr/bin/env bash
# Run the dev-built microVM benchmark under the perf host lock with a real TAP
# and nftables policy. The upstream network is private to the benchmark.
set -euo pipefail
if [[ $# != 2 || ${CALTERNAL_PERF_HELD:-0} != 1 ]]; then
    echo 'Usage: /usr/local/bin/perf-lock.sh microvm-perf.sh VM_CONFIG MACHINE_TEST_BINARY' >&2
    exit 2
fi
config=$(realpath "$1")
binary=$(realpath "$2")
installer=$(cd -- "$(dirname -- "$0")/../.." && pwd)/deploy/microvm/install-egress.py

exec unshare -Urnm bash -s -- "$config" "$binary" "$installer" <<'ISOLATED'
set -euo pipefail
umask 077
config=$1
binary=$2
installer=$3
# Account disks are sparse files. Use disk-backed storage and a short Unix
# socket path on perf hosts where /tmp is a memory-backed filesystem.
scratch=$(mktemp -d "$HOME/.calvm.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
export TMPDIR="$scratch"
export CALTERNAL_MICROVM_TEST_CONFIG="$scratch/vm.json"
mount --make-rprivate /
mount -t tmpfs tmpfs /run
ip netns attach calvm "$$"
ip netns add upstream
ip link set lo up
ip link add wan0 type veth peer name wan1
ip link set wan1 netns upstream
ip addr add 192.0.2.1/24 dev wan0
ip link set wan0 up
ip -n upstream addr add 192.0.2.2/24 dev wan1
ip -n upstream link set wan1 up
printf '1\n' > /proc/sys/net/ipv4/ip_forward

python3 - "$config" "$scratch" <<'PY'
import ipaddress
import json
from pathlib import Path
import subprocess
import sys

config = json.loads(Path(sys.argv[1]).read_text())
scratch = Path(sys.argv[2])
slots = []
for index in range(config["max_guests"]):
    address = ipaddress.IPv4Address("10.213.0.0") + 4 * index
    name = f"calvm{index}"
    gateway = str(address + 1)
    subprocess.run(["ip", "tuntap", "add", "dev", name, "mode", "tap", "user", "0"], check=True)
    subprocess.run(["ip", "addr", "add", f"{gateway}/30", "dev", name], check=True)
    subprocess.run(["ip", "link", "set", name, "up"], check=True)
    slots.append(dict(name=name, address=str(address + 2), prefix=30, gateway=gateway, dns="192.0.2.2"))
policy = scratch / "policy.json"
policy.write_text(json.dumps(dict(namespace="calvm", uplink="wan0", slots=slots, allow=[])))
config["network_policy"] = str(policy)
(scratch / "vm.json").write_text(json.dumps(config))
PY
python3 "$installer" "$scratch/policy.json"
"$binary" sandbox::microvm::measure::compare_microvm_and_podman_under_perf_lock --exact --ignored --nocapture
ISOLATED
