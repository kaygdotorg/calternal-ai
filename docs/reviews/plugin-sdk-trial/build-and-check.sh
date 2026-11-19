#!/bin/sh
set -eu
if [ -f worker.mjs ]; then
  set -- worker.mjs
else
  set --
fi
node node_modules/@calternal/plugin-sdk/node_modules/typescript/bin/tsc \
  --noEmit --strict --skipLibCheck --module nodenext --target es2022 plugin.ts
./node_modules/.bin/calternal-build-plugin plugin.ts dist "$@"
node check.mjs dist
