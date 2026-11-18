#!/usr/bin/env bash
# Run one test command with a private TMPDIR. Fail on orphan containers, VMMs or
# provider sign-in processes (the adapter and the official `claude auth login`).
set -euo pipefail
if (($# == 0)); then
  echo 'Usage: check-podman-tests.sh COMMAND [ARG ...]' >&2
  exit 2
fi
test_root=$(realpath "$(mktemp -d)")
trap 'rmdir -- "$test_root" 2>/dev/null || true' EXIT
# Query the engine before running tests so an unavailable tool cannot mask leaks.
has_podman=
if command -v podman >/dev/null; then
  podman ps -aq >/dev/null
  has_podman=1
else
  echo 'SKIP Podman leak check: Podman is unavailable.'
fi
status=0
TMPDIR=$test_root "$@" || status=$?
if [[ -n $has_podman ]]; then
  leaked=$(podman ps -a --filter label=org.calternal.sandbox-root \
    --format '{{.ID}} {{.Label "org.calternal.sandbox-root"}}' |
    awk -v scope="$test_root" '
      { root=substr($0,index($0," ")+1) }
      root == scope || index(root, scope "/") == 1 { print $1 }
    ')
  if [[ -n $leaked ]]; then
    printf 'Test command left Podman containers in %s:\n%s\n' "$test_root" "$leaked" >&2
    mapfile -t ids <<< "$leaked"
    podman rm -f -t 0 --ignore "${ids[@]}" >/dev/null ||
      echo 'Could not remove the leaked test containers.' >&2
    status=1
  else
    printf '%s\n' 'Podman leak check passed.'
  fi
fi
if [[ -d /proc ]]; then
  python3 - "$test_root" <<'PYTHON'
import os
from pathlib import Path
import sys

root = Path(sys.argv[1])
leaked = []
sign_in = []


def home_of(proc):
    for item in (proc / "environ").read_bytes().split(b"\0"):
        if item.startswith(b"HOME="):
            return Path(os.fsdecode(item[5:]))
    return None


for proc in Path("/proc").iterdir():
    if not proc.name.isdecimal():
        continue
    try:
        if proc.stat().st_uid != os.geteuid():
            continue
        argv = (proc / "cmdline").read_bytes().rstrip(b"\0").split(b"\0")
        if not argv:
            continue
        if Path(os.fsdecode(argv[0])).name == "firecracker":
            config = Path(os.fsdecode(argv[argv.index(b"--config-file") + 1]))
            if config.is_relative_to(root):
                leaked.append(proc.name)
        # A sign-in left running after its test ends holds shared resources for
        # minutes. Match the adapter (`main.js --sign-in`) and the login it
        # starts (`... auth login`) by the private home inside this test root.
        is_adapter = argv[-1:] == [b"--sign-in"] and argv[-2:-1] and argv[-2].endswith(b"main.js")
        is_login = argv[-2:] == [b"auth", b"login"]
        if is_adapter or is_login:
            home = home_of(proc)
            if home is not None and home.is_relative_to(root):
                sign_in.append(proc.name)
    except (OSError, ValueError, IndexError):
        continue
if leaked:
    sys.exit(f"Test command left Firecracker processes in {root}: {', '.join(leaked)}")
print("Firecracker leak check passed.")
if sign_in:
    sys.exit(f"Test command left provider sign-in processes in {root}: {', '.join(sign_in)}")
print("Provider sign-in leak check passed.")
PYTHON
fi
exit "$status"
