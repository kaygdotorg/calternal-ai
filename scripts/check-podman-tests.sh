#!/usr/bin/env bash
# Run one test command with a private TMPDIR. Fail if its Podman artifacts survive.
set -euo pipefail
if (($# == 0)); then
  echo 'Usage: check-podman-tests.sh COMMAND [ARG ...]' >&2
  exit 2
fi
if ! command -v podman >/dev/null; then
  echo 'SKIP Podman leak check: Podman is unavailable.'
  exec "$@"
fi
test_root=$(realpath "$(mktemp -d)")
trap 'rmdir -- "$test_root" 2>/dev/null || true' EXIT
# Query the engine before running tests so an unavailable tool cannot mask leaks.
podman ps -aq >/dev/null
status=0
TMPDIR=$test_root "$@" || status=$?
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
  exit 1
fi
printf '%s\n' 'Podman leak check passed.'
exit "$status"
