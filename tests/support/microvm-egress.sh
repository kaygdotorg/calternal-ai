#!/usr/bin/env bash
# Dev-only real TAP test. A private network namespace contains fake upstream
# services. The host network and firewall are outside this namespace.
set -euo pipefail
if [[ $# != 2 ]]; then
    echo 'Usage: microvm-egress.sh VM_CONFIG MACHINE_TEST_BINARY' >&2
    exit 2
fi
config=$(realpath "$1")
binary=$(realpath "$2")
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
exec unshare -Urnm bash -s -- "$config" "$binary" "$repo" <<'ISOLATED'
set -euo pipefail
umask 077
config=$1
binary=$2
repo=$3
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
ip -n upstream addr add 169.254.169.254/32 dev wan1
ip -n upstream addr add 203.0.113.3/32 dev wan1
ip -n upstream link set wan1 up
ip -n upstream link set lo up
ip route add 169.254.169.254/32 via 192.0.2.2
ip route add 203.0.113.3/32 via 192.0.2.2
ip tuntap add dev calvm0 mode tap user "$(id -u)"
ip addr add 10.213.0.1/30 dev calvm0
ip link set calvm0 up
ip tuntap add dev calvm1 mode tap user "$(id -u)"
ip addr add 10.213.0.5/30 dev calvm1
ip link set calvm1 up
ip -n upstream route add 10.213.0.0/30 via 192.0.2.1
printf "1\n" > /proc/sys/net/ipv4/ip_forward
scratch=$(mktemp -d "${TMPDIR:-/tmp}/calvm.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
cat > "$scratch/policy.json" <<'JSON'
{
  "namespace": "calvm",
  "uplink": "wan0",
  "slots": [
    { "name": "calvm1", "address": "10.213.0.6", "prefix": 30, "gateway": "10.213.0.5", "dns": "192.0.2.2" },
    { "name": "calvm0", "address": "10.213.0.2", "prefix": 30, "gateway": "10.213.0.1", "dns": "192.0.2.2" }
  ],
  "allow": [
    { "destination": "192.0.2.2/32", "protocol": "tcp", "port": 18080 },
    { "destination": "169.254.169.254/32", "protocol": "tcp", "port": 18080 },
    { "destination": "203.0.113.3/32", "protocol": "tcp", "port": 18080 }
  ],
  "deny": ["203.0.113.3/32"]
}
JSON
python3 "$repo/deploy/microvm/install-egress.py" "$scratch/policy.json"
python3 - "$config" "$scratch" <<'PY'
from pathlib import Path
import json,sys
scratch=Path(sys.argv[2])
config=json.loads(Path(sys.argv[1]).read_text())
config.update(network_policy=str(scratch/"policy.json"),max_guests=2,warm_guests=0)
(scratch/"vm.json").write_text(json.dumps(config))
PY
cat > "$scratch/service.py" <<'PY'
from pathlib import Path
import selectors
import socket
import struct
import sys

ip, marker = sys.argv[1:3]
mux = selectors.DefaultSelector()
for port in (18080, 18081):
    sock = socket.socket()
    sock.bind((ip, port))
    sock.listen(5)
    mux.register(sock, selectors.EVENT_READ, "tcp")
dns = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
dns.bind((ip, 53))
mux.register(dns, selectors.EVENT_READ, "dns")
Path(marker).touch()
while True:
    for key, _ in mux.select():
        if key.data == "tcp":
            key.fileobj.accept()[0].close()
        else:
            query, peer = dns.recvfrom(512)
            end = query.index(b"\0", 12) + 5
            header = query[:2] + struct.pack("!HHHHH", 0x8180, 1, 1, 0, 0)
            record = struct.pack("!HHHIH4s", 0xc00c, 1, 1, 1, 4, socket.inet_aton("192.0.2.2"))
            dns.sendto(header + query[12:end] + record, peer)
PY
ip netns exec upstream python3 "$scratch/service.py" 192.0.2.2 "$scratch/upstream" &
upstream=$!
python3 "$scratch/service.py" 10.213.0.1 "$scratch/router" &
router=$!
ip netns exec upstream python3 "$scratch/service.py" 169.254.169.254 "$scratch/metadata" &
metadata=$!
ip netns exec upstream python3 "$scratch/service.py" 203.0.113.3 "$scratch/control" &
control=$!
services=("$upstream" "$router" "$metadata" "$control")
trap 'kill "${services[@]}" 2>/dev/null || true; wait "${services[@]}" 2>/dev/null || true; rm -rf -- "$scratch"' EXIT
ready() { [[ -f "$scratch/upstream" && -f "$scratch/router" && -f "$scratch/metadata" && -f "$scratch/control" ]]; }
for ((i=0;i<25;i++)); do
    if ready; then break; fi
    sleep .1
done
ready
ip netns exec upstream python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.settimeout(0.4)
    try:
        sock.connect(("10.213.0.2", 18080))
    except TimeoutError:
        pass
    else:
        raise SystemExit("unexpected guest ingress")
PY
nft list chain inet calternal_guests forward | grep -E 'oifname "calvm0" counter packets [1-9][0-9]* bytes .* drop'
export TMPDIR="$scratch" CALTERNAL_MICROVM_EGRESS_TEST_CONFIG="$scratch/vm.json"
"$binary" sandbox::microvm::tests::networked_guest_can_use_only_the_declared_egress --exact --nocapture
nft list chain inet calternal_guests forward | grep -E 'oifname "calvm1" counter packets [1-9][0-9]* bytes .* drop'
ISOLATED
