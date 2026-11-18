#!/usr/bin/env bash
# Restore the target's previous server image, machine binary and service templates.
set -euo pipefail
here=$(cd -- "$(dirname -- "$0")" && pwd)
die() { printf '%s\n' "$*" >&2; exit 2; }
usage() { printf '%s\n' 'Usage: deploy/rollback.sh --target FILE [--dry-run]'; }
dry_run=0 config=
while (($#)); do
  case $1 in
    --target) config=${2:?--target needs a file}; shift ;;
    --dry-run) dry_run=1 ;;
    --help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
  shift
done
[[ -f $config ]] || die 'Supply an operator-owned target file with --target.'
source "$here/target.sh"
load_target "$config"
if ((dry_run)); then
  remote bash -s -- rollback "${target_args[@]}"
  print_remote_script
else
  remote bash -s -- rollback "${target_args[@]}" < "$here/remote.sh"
fi
