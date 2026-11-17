#!/bin/sh
# Builds the web app for the preview host and copies it to ~cua/preview/ai (served at https://preview.calternal.cloud/ai/, NetBird only).
# Run on calternal-dev-02-std, inside `nix develop`, from apps/web. Touches nothing else under ~cua/preview. rsync is not installed there, so tar.
set -eu
# Built into dist-preview: `dist` (base /) stays what the e2e scripts serve.
# The preview talks to its real server by default (no `?server=1` needed); `?server=0` shows the fixtures.
VITE_SERVER_DEFAULT=1 CAL_BASE=/ai/ CAL_OUT=dist-preview npm run build
dest=/home/cua/preview/ai
# The other user cannot enter this directory, so sudo starts from / (otherwise `find` fails after nothing was copied and the target is left empty).
tar -C dist-preview -cf - . | (cd / && sudo -n -u cua sh -c "find $dest -mindepth 1 -delete && tar -xf - -C $dest")
code=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/)
deep=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/threads/t-run)
font=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/fonts/google-sans-flex-latin-opsz-normal.woff2)
echo "preview: / $code, deep link $deep, font $font"
