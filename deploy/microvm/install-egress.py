#!/usr/bin/env python3
"""Install a bounded allowlist in an operator-provisioned network namespace."""
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

BLOCKED = (
    "0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8",
    "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16",
    "224.0.0.0/4", "240.0.0.0/4",
)

def name(value, prefix=""):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_]{1,15}", value) or not value.startswith(prefix):
        raise ValueError("invalid network interface or namespace name")
    return value

def rules(policy):
    uplink = name(policy["uplink"])
    slots = policy["slots"]
    allowed = policy["allow"]
    deny = policy.get("deny", [])
    if not 1 <= len(slots) <= 64 or len(allowed) + len(deny) > 256:
        raise ValueError("use at most 64 TAPs and 256 egress rules")
    blocked = tuple(ipaddress.IPv4Network(cidr) for cidr in (*BLOCKED, *deny))
    names, addresses, networks = set(), set(), []
    forward = []
    for slot in slots:
        tap = name(slot["name"], "calvm")
        ip = ipaddress.IPv4Address(slot["address"])
        prefix = slot["prefix"]
        if type(prefix) is not int or not 24 <= prefix <= 30:
            raise ValueError("TAP prefixes must be in the range 24–30")
        net = ipaddress.IPv4Network(f"{ip}/{prefix}", strict=False)
        gateway = ipaddress.IPv4Address(slot["gateway"])
        dns = ipaddress.IPv4Address(slot["dns"])
        usable = {net.network_address, net.broadcast_address}
        if (tap == uplink or tap in names or ip == gateway or gateway not in net
                or not usable.isdisjoint({ip, gateway}) or any(net.overlaps(other) for other in networks)):
            raise ValueError("TAPs require distinct routed subnets and usable addresses")
        if any(dns in block for block in blocked):
            raise ValueError("use an operator-approved public DNS resolver")
        names.add(tap)
        addresses.add(str(ip))
        networks.append(net)
        tagged = f'iifname "{tap}" oifname "{uplink}" ip saddr {ip}'
        for proto in ("udp", "tcp"):
            forward.append(f"{tagged} ip daddr {dns} {proto} dport 53 accept")
        forward.append(f"{tagged} jump allowed_egress")
        forward.append(
            f'iifname "{uplink}" oifname "{tap}" ip daddr {ip} ct direction reply ct state established,related accept'
        )
        forward.append(f'oifname "{tap}" counter drop')
    egress = ["ip daddr { " + ", ".join(map(str, blocked)) + " } drop"]
    for entry in allowed:
        network = ipaddress.IPv4Network(entry["destination"])
        transport, port = entry["protocol"], entry["port"]
        if transport not in ("tcp", "udp") or type(port) is not int or not 1 <= port <= 65535 or port == 53:
            raise ValueError("use explicit TCP or UDP destination ports; DNS follows the TAP settings")
        egress.append(f"ip daddr {network} {transport} dport {port} accept")
    quoted_taps = ", ".join(f'"{tap}"' for tap in sorted(names))
    guest_ips = ", ".join(sorted(addresses))
    return "\n".join([
        "table inet calternal_guests {",
        "  chain allowed_egress {", *egress, "    drop", "  }",
        "  chain input { type filter hook input priority filter; policy accept;",
        f"    iifname {{ {quoted_taps} }} drop", "  }",
        "  chain output { type filter hook output priority filter; policy accept;",
        f"    oifname {{ {quoted_taps} }} drop", "  }",
        "  chain forward { type filter hook forward priority filter; policy drop;",
        *forward, "  }",
        "  chain postrouting { type nat hook postrouting priority srcnat; policy accept;",
        f'    oifname "{uplink}" ip saddr {{ {guest_ips} }} masquerade',
        "  }", "}", "",
    ])

def install(path):
    path = path.absolute()
    if os.geteuid() != 0:
        raise ValueError("the namespace operator must install the policy")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as src:
        meta = os.fstat(src.fileno())
        if not stat.S_ISREG(meta.st_mode):
            raise ValueError("the policy must be a regular file")
        raw = src.read(65537)
    if meta.st_uid != 0 or meta.st_mode & 0o022 or len(raw) > 65536:
        raise ValueError("install an operator-owned immutable policy of at most 64 KiB")
    policy = json.loads(raw)
    namespace = name(policy["namespace"], "calvm")
    inode = os.stat("/proc/self/ns/net").st_ino
    try:
        initial = os.stat("/proc/1/ns/net").st_ino
    except PermissionError:
        initial = None
    if inode == initial:
        raise ValueError("use a dedicated network namespace")
    if os.stat(Path("/run/netns") / namespace).st_ino != inode:
        raise ValueError("run this command from the declared dedicated namespace")
    compiled = rules(policy)
    receipt = path.with_suffix(".installed")
    receipt.unlink(missing_ok=True)
    existing = subprocess.run(["nft", "list", "table", "inet", "calternal_guests"],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if existing.returncode == 0:
        compiled = "delete table inet calternal_guests\n" + compiled
    subprocess.run(["nft", "-f", "-"], input=compiled, text=True, check=True)
    boot = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    value = f"{boot} {inode} {hashlib.sha256(raw).hexdigest()}\n"
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as tmp:
        name_on_disk = Path(tmp.name)
        try:
            tmp.write(value.encode())
            tmp.flush()
            os.fchmod(tmp.fileno(), 0o644)
            os.fsync(tmp.fileno())
            name_on_disk.replace(receipt)
        finally:
            name_on_disk.unlink(missing_ok=True)
    print("Guest firewall installed in the declared namespace.")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: install-egress.py POLICY.json")
    try:
        install(Path(sys.argv[1]))
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        sys.exit(f"Guest egress is closed: {error}")
