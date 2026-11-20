#!/usr/bin/env bash
# Build the committed reseed module against the target Node N-API headers.
set -euo pipefail
source=$1 output=$2
# Node and its own musl runtime supply every symbol. Linking a separate libc
# dependency would require a development soname absent from the guest root.
"${CC_x86_64_unknown_linux_musl:?}" -Wall -Wextra -Werror -O2 -shared -fPIC \
  -nostdlib -I "${CALTERNAL_NODE_HEADERS:?}" "$source" -o "$output"
if readelf -d "$output" | grep NEEDED; then
  printf 'Unexpected native module dependency in %s\n' "$output" >&2
  exit 1
fi
