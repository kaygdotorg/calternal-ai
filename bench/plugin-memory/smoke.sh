#!/usr/bin/env bash
# Bench-only correctness smoke on dev-01 (no timings are used from it).
set -euo pipefail
cd ~/Developer/lanes/plugin-memory
export BENCH_FIXTURE=$PWD/image/root/fixture
N=$BENCH_FIXTURE/node
all='["threads","notes","guide","threads-claude-code","submission","settings","trivial"]'
four='["threads","notes","trivial","faults"]'
{
for mode in ${SMOKE_MODES:-process shared}; do
  echo "{\"op\":\"layout\",\"mode\":\"$mode\",\"plugins\":$all}"
  echo '{"op":"mem"}'
  echo '{"op":"bench","plugin":"threads","n":20,"warmup":2}'
  for k in throw exit loop heap forge peek abort; do
    echo "{\"op\":\"layout\",\"mode\":\"$mode\",\"plugins\":$four}"
    echo "{\"op\":\"fault\",\"kind\":\"$k\",\"watch_ms\":2500}"
  done
done
echo '{"op":"layout","mode":"lazy","runtime":"shared","plugins":["threads","notes"],"idle_ms":300}'
echo '{"op":"cold_call","plugin":"threads","following":3}'
echo '{"op":"layout","mode":"lazy","runtime":"process","plugins":["threads","notes"],"idle_ms":300}'
echo '{"op":"cold_call","plugin":"notes","following":3}'
echo '{"op":"close"}'
} | timeout 300 "$N" "$BENCH_FIXTURE/host.mjs" stdin
