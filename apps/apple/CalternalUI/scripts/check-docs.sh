#!/bin/sh
# Fails when a public declaration in Sources/CalternalUI has no doc comment (a line starting with `///` or an attribute right above it).
# Initialisers, enum cases, `body` and protocol witnesses (`encode`, `sizeThatFits`, `placeSubviews`) are exempt (their type's comment and parameter docs carry them), and consecutive declarations with no blank line
# between them share the comment above the first, so a group of related fields or accessors is documented once.
set -eu
cd "$(dirname "$0")/.."
bad=$(find Sources/CalternalUI -name '*.swift' ! -path '*/Generated/*' -print0 | xargs -0 awk '
  FNR == 1 { prev = ""; ok = 0 }
  /public var body: some View/ || /public func (encode|sizeThatFits|placeSubviews)\(/ { next }
  /^[[:space:]]*public (struct|enum|final class|class|protocol|func|static|var|let|typealias|actor)/ || /^[[:space:]]*public (final )?(struct|class|enum)/ {
    if (prev ~ /^[[:space:]]*\/\/\// || prev ~ /^[[:space:]]*@/) ok = 1
    else if (!(ok && !blank)) { print FILENAME ":" FNR ": " $0; ok = 0 }
  }
  { blank = ($0 ~ /^[[:space:]]*$/); if (!blank) { if ($0 !~ /^[[:space:]]*public /) ok = ($0 ~ /^[[:space:]]*\/\/\//) ? ok : 0; prev = $0 } }
')
if [ -n "$bad" ]; then echo "public items without a doc comment:"; echo "$bad"; exit 1; fi
echo "docs: every public item in CalternalUI is documented"
