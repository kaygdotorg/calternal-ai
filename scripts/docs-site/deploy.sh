#!/bin/sh
# Deploy only validated docs output; all other preview directories are out of scope.
set -eu
cd "$(dirname "$0")/../.."
node scripts/docs-site/verify.mjs
if command -v rsync >/dev/null 2>&1; then
  sudo -n -u cua rsync -a --delete apps/docs/dist/ /home/cua/preview/ai-docs/
else
  # Fixed destination, no symlink traversal and no paths outside ai-docs.
  sudo -n -u cua sh -c 'test -d /home/cua/preview/ai-docs && test ! -L /home/cua/preview/ai-docs && find /home/cua/preview/ai-docs -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +'
  tar -C apps/docs/dist -cf - . | sudo -n -u cua tar --no-same-owner -C /home/cua/preview/ai-docs -xf -
fi
