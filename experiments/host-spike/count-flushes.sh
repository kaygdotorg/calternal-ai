#!/bin/sh
# Counts flushes (fsync, fdatasync) and file writes per call kind, under strace.
# A count, not a timing: run it on a dev VM. Usage: count-flushes.sh <host-spike binary>
set -eu
BIN=$1
DIR=$(mktemp -d)
SPACE=019a0000-0000-7000-8000-000000000001
ANA=019a0000-0000-7000-8000-0000000000a1
LEE=019a0000-0000-7000-8000-0000000000b2
"$BIN" init --root "$DIR" --space $SPACE --member $ANA:Ana:owner --member $LEE:Lee:editor >/dev/null
strace -f -qq -e trace=fsync,fdatasync,pwrite64,write,renameat2,rename -o "$DIR/trace" "$BIN" serve --root "$DIR" >"$DIR/out" &
PID=$!
sleep 1
ADDR=$(sed -n 's/^listening //p' "$DIR/out")
call() { curl -s -X POST "$ADDR/call" -H "x-person: $1" -H "x-space: $SPACE" -H "x-plugin: $2" -H 'content-type: application/json' -d "$3"; }
here() { wc -l <"$DIR/trace"; }
count() { # label from-line
  tail -n +"$(($2 + 1))" "$DIR/trace" | awk -v l="$1" -v n="$3" '
    /fsync\(|fdatasync\(/ {f++} /pwrite64\(/ {p++} /rename/ {r++}
    END {printf "%-16s per call: %.1f flushes, %.1f pwrite, %.1f renames\n", l, f/n, p/n, r/n}'
}
DECL=$(cat "$(dirname "$0")/plugins/spike-chat.json")
call $ANA host "{\"name\":\"plugins.install\",\"input\":{\"declaration\":$DECL}}" >/dev/null
FILE=$(call $LEE spike-chat '{"name":"files.create","input":{"kind":"chats","fields":{"title":"Count"}}}' | sed 's/.*"id":"\([^"]*\)".*/\1/')
call $LEE spike-chat "{\"name\":\"record.append\",\"input\":{\"file\":\"$FILE\",\"body\":\"warm\"}}" >/dev/null
sleep 0.2; S=$(here)
for i in 1 2 3 4 5 6 7 8 9 10; do call $LEE spike-chat "{\"name\":\"record.append\",\"input\":{\"file\":\"$FILE\",\"body\":\"message $i\"}}" >/dev/null; done
sleep 0.2; count record.append "$S" 10
S=$(here)
for i in 1 2 3 4 5; do call $LEE spike-chat "{\"name\":\"files.create\",\"input\":{\"kind\":\"chats\",\"fields\":{\"title\":\"C$i\"}}}" >/dev/null; done
sleep 0.2; count files.create "$S" 5
S=$(here)
for i in 1 2 3 4 5; do call $LEE spike-chat '{"name":"query","input":{"collection":"All"}}' >/dev/null; done
sleep 0.2; count query "$S" 5
S=$(here)
ANCHOR=$(call $LEE spike-chat "{\"name\":\"record.append\",\"input\":{\"file\":\"$FILE\",\"body\":\"to be deleted soon\"}}" | sed 's/.*"anchor":"\([^"]*\)".*/\1/')
sleep 0.2; S=$(here)
call $LEE spike-chat "{\"name\":\"record.delete\",\"input\":{\"file\":\"$FILE\",\"anchor\":\"$ANCHOR\"}}" >/dev/null
sleep 0.2; count record.delete "$S" 1
kill $(pgrep -f "host-spike serve --root $DIR") $PID 2>/dev/null || true
wait $PID 2>/dev/null || true
rm -rf "$DIR"
