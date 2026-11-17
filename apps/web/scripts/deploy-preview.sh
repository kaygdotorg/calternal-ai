#!/bin/sh
# Builds the web app for the preview host and copies it to ~cua/preview/ai (served at https://preview.calternal.cloud/ai/, NetBird only).
# Run on calternal-dev-02-std, inside `nix develop`, from apps/web. Touches nothing else under ~cua/preview. rsync is not installed there, so tar.
set -eu
CAL_BASE=/ai/ npm run build
dest=/home/cua/preview/ai
tar -C dist -cf - . | sudo -n -u cua sh -c "find $dest -mindepth 1 -delete && tar -xf - -C $dest"
code=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/)
deep=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/threads/t-run)
font=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/fonts/google-sans-flex-latin-opsz-normal.woff2)
echo "preview: / $code, deep link $deep, font $font"
