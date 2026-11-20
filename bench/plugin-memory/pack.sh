#!/usr/bin/env bash
# Build the bench guest image on dev-01 (never on a perf VM).
# Usage: pack.sh <dev checkout> <threads-plugin checkout> <output dir>
# Both checkouts must already have their plugins built.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
dev="$(cd "$1" && pwd)"; threads="$(cd "$2" && pwd)"; out="$3"
mkdir -p "$out"; out="$(cd "$out" && pwd)"
root="$out/root"
[ -d "$root" ] && chmod -R u+w "$root"; rm -rf "$root"; mkdir -p "$root"/{bin,dev,proc,sys,run,tmp,fixture,nix/store}
node="$(dirname "$(dirname "$(readlink -f "$(command -v node)")")")"
nix-store -qR "$node" >"$out/node-closure"
while IFS= read -r path; do cp -a "$path" "$root/nix/store/"; done <"$out/node-closure"
busybox="$(nix build --no-link --print-out-paths nixpkgs#pkgsStatic.busybox)"
cp "$busybox/bin/busybox" "$root/bin/"; ln -sfn busybox "$root/bin/sh"
ln -sfn "$node/bin/node" "$root/fixture/node"
install -m 755 "$here/guest/init" "$root/init"
for f in host.mjs shared-runtime.mjs thread-boot.mjs runner.mjs trivial.mjs faults.mjs; do install -m 644 "$here/guest/$f" "$root/fixture/"; done
cp "$dev/plugins/calternal-notes-insights/dist/worker.js" "$root/fixture/notes.mjs"
cp "$dev/plugins/calternal-plugin-guide/dist/worker.js" "$root/fixture/guide.mjs"
cp "$dev/plugins/calternal-plugin-submission-guide/dist/worker.js" "$root/fixture/submission.mjs"
cp "$dev/tests/plugins/composer-settings/dist/worker.js" "$root/fixture/settings.mjs"
cp "$threads"/plugins/calternal-threads/dist/worker-*.mjs "$root/fixture/threads.mjs"
cp "$threads"/plugins/calternal-threads-claude-code/dist/worker-*.mjs "$root/fixture/threads-claude-code.mjs"
cp "$threads/plugins/calternal-threads/test/fixtures/v1/tool-output.md" "$root/fixture/thread-v1.md"
glibc="$(nix build --no-link --print-out-paths nixpkgs#glibc.static)"
for c in relay mounts; do cc -O2 -Wall -Wextra -static -L"$glibc/lib" "$here/native/$c.c" -o "$root/fixture/$c"; strip "$root/fixture/$c"; done
e2fs="$(nix build --no-link --print-out-paths nixpkgs#e2fsprogs.bin)"
size=$(du -sm "$root" | cut -f1)
rm -f "$out/root.ext4"; truncate -s "$((size+64))M" "$out/root.ext4"
"$e2fs/bin/mkfs.ext4" -q -F -m0 -E lazy_itable_init=0,lazy_journal_init=0 -d "$root" "$out/root.ext4"
(cd "$out" && ls -l root/fixture >fixtures.txt && sha256sum root.ext4 root/fixture/*.mjs >SHA256SUMS)
echo "image: $out/root.ext4 (node $("$node/bin/node" --version))"
