#!/bin/sh
# Regenerates the Swift tokens from the web design system's one token file; `scripts/tokens.sh --check` fails when the checked-in file is stale.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/../../.." && pwd)
cd "$here"
exec swift run -c release --quiet generate-tokens "$@" "$root/packages/ui/tokens/tokens.json" "$root/docs/native-ui-study.json" "$here/Sources/CalternalUI/Generated/Tokens.generated.swift"
