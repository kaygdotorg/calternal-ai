#!/bin/sh
# Builds the web app for the preview host and copies it to ~cua/preview/ai (served at https://preview.calternal.cloud/ai/, NetBird only).
# Run on calternal-dev-02-std, inside `nix develop`, from apps/web. Touches nothing else under ~cua/preview. rsync is not installed there, so tar.
set -eu
# CAL_FIXTURES_ONLY=1 skips the preview host (for looking at a branch on the Simulator without replacing what the preview serves).
if [ -z "${CAL_FIXTURES_ONLY:-}" ]; then
# Built into dist-preview: `dist` (base /) stays what the e2e scripts serve.
# The preview talks to its real server by default (no `?server=1` needed); `?server=0` shows the fixtures.
VITE_SERVER_DEFAULT=1 CAL_BASE=/ai/ CAL_OUT=dist-preview npm run build
# CAL_BUILD_LABEL (optional) says what this build is, written to BUILD.txt beside the app (for example "ui-shell 8398de73, not yet in dev").
[ -n "${CAL_BUILD_LABEL:-}" ] && printf "%s\n" "$CAL_BUILD_LABEL" > dist-preview/BUILD.txt
dest=/home/cua/preview/ai
# The other user cannot enter this directory, so sudo starts from / (otherwise `find` fails after nothing was copied and the target is left empty).
tar -C dist-preview -cf - . | (cd / && sudo -n -u cua sh -c "find $dest -mindepth 1 -delete && tar -xf - -C $dest")
code=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/)
deep=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/threads/t-run)
font=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1/ai/fonts/google-sans-flex-latin-opsz-normal.woff2)
echo "preview: / $code, deep link $deep, font $font"
fi
# The same tip on fixtures data (no server, nothing to sign in to), served over NetBird only on 10.69.69.222:4310 for the iPhone Simulator. Built at base / into a stable directory, then served by serve-fixtures.mjs.
CAL_BASE=/ CAL_OUT=dist-fixtures npm run build >/dev/null
[ -n "${CAL_BUILD_LABEL:-}" ] && printf "%s\n" "$CAL_BUILD_LABEL" > dist-fixtures/BUILD.txt
fx=$HOME/fixtures-preview; rm -rf "$fx.new" && cp -r dist-fixtures "$fx.new" && rm -rf "$fx" && mv "$fx.new" "$fx"
pkill -f "serve-fixtures.mjs $fx" 2>/dev/null || true
setsid nohup node scripts/serve-fixtures.mjs "$fx" 10.69.69.222 4310 > "$HOME/fixtures-preview.log" 2>&1 < /dev/null &
sleep 1
echo "fixtures: http://10.69.69.222:4310/threads/t-run $(curl -s -o /dev/null -w '%{http_code}' http://10.69.69.222:4310/threads/t-run)"
