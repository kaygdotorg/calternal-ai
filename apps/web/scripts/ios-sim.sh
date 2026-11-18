#!/bin/bash
# Drives a lane's own iPhone Simulator on the Mac Studio (kayg@std) in Safari, from any machine that can ssh there: create it, open an address, tap, type, swipe, take screenshots, delete it.
# It wraps `~/Developer/calternal-sim/ui.sh` (XCUITest runner on std: testOpenURL, testTap, testSwipe, testTopCheck), which holds the Studio's lock, so lanes never touch each other's runs. README.md in this folder.
# CAL_SIM_DEVICE picks the device type for `create` (default iPhone-18-Pro, 393 pt; iPhone-18-Pro-Max is 440 pt, iPhone-17e and others exist: `xcrun simctl list devicetypes`).
# Usage: ios-sim.sh <command> [args]   (state lives in $CAL_SIM_STATE, default ~/.cache/calternal-ios-sim)
set -euo pipefail
STD="${CAL_STD:-kayg@std}"
SIMDIR="${CAL_SIM_DIR:-Developer/calternal-sim}"
LANE="${CAL_LANE:-$(basename "$(git rev-parse --show-toplevel 2>/dev/null || echo lane)")}"
STATE="${CAL_SIM_STATE:-$HOME/.cache/calternal-ios-sim}"; mkdir -p "$STATE"
SHOTS="${CAL_SIM_SHOTS:-$PWD/sim-shots}"
UDIDFILE="$STATE/$LANE.udid"
udid() { [[ -s "$UDIDFILE" ]] || { echo "no simulator for lane '$LANE': run '$0 create' first" >&2; exit 2; }; cat "$UDIDFILE"; }
# one preview port per lane (4200 to 4399 from the lane's name), so lanes sharing the Studio and the dev VMs never meet on one port; CAL_SIM_PORT overrides
port() { echo "${CAL_SIM_PORT:-$((4200 + $(printf %s "$LANE" | cksum | cut -d' ' -f1) % 200))}"; }
# runs ui.sh on the Studio against this lane's simulator; prints the last lines of its output
ui() { local U; U=$(udid); ssh "$STD" "cd ~/$SIMDIR && SIM_UDID=$U ./ui.sh $(printf '%q ' "$@")" 2>&1 | grep -E "OUTPUT_DIR|error|failed|Executed|TEST (EXECUTE )?FAILED" | tail -3; }
cmd="${1:-help}"; shift || true
case "$cmd" in
  create)   # create + boot a simulator of its own (iPhone 18 Pro, iOS 27.0); never reuses another lane's
    name="${1:-$LANE-$(date +%m%d%H%M)}"
    U=$(ssh "$STD" "xcrun simctl create '$name' com.apple.CoreSimulator.SimDeviceType.${CAL_SIM_DEVICE:-iPhone-18-Pro} com.apple.CoreSimulator.SimRuntime.iOS-27-0")
    ssh "$STD" "xcrun simctl boot $U; xcrun simctl bootstatus $U >/dev/null"
    echo "$U" > "$UDIDFILE"; echo "created $name ($U)" ;;
  appearance) ssh "$STD" "xcrun simctl ui $(udid) appearance ${1:?dark or light}" ;;
  open)     # open an address in Safari (http or https): open http://127.0.0.1:$PORT/threads/t-idle
    ui testOpenURL "URL=${1:?url}" ;;
  onboard)  # a fresh Safari shows the one-time "Choose a look" panel: keep the default so the app is visible
    ui testTap X=0.734 Y=0.866 WAIT=1 ;;
  tap)      # normalized 0..1 across the screen (x right, y down): tap 0.5 0.9; optional wait seconds
    ui testTap "X=${1:?x}" "Y=${2:?y}" "WAIT=${3:-1}" ;;
  tappx)    # screen points (393 x 852 on this device): tappx 200 760
    ui testTopCheck BUNDLE=com.apple.mobilesafari "ACTIONS=xy~${1:?px}~${2:?py}|wait~1" ;;
  type)     # types into the focused field with the software keyboard (tap the field first): type "hello"
    ui testTopCheck BUNDLE=com.apple.mobilesafari "ACTIONS=type~${1:?text}|wait~1" ;;
  keys)     # a raw pipeline: ios-sim.sh keys 'xy~200~760|wait~1|type~hi|key~return|shot~after'
    ui testTopCheck BUNDLE=com.apple.mobilesafari "ACTIONS=${1:?actions}" ;;
  swipe)    # one finger, normalized: swipe 0.8 0.3 -0.5 0 [seconds]
    ui testSwipe "X=${1:?x}" "Y=${2:?y}" "DX=${3:-0}" "DY=${4:-0}" "SECONDS=${5:-0.3}" ;;
  shot)     # a screenshot straight from the simulator to $CAL_SIM_SHOTS/<name>.png (prints the path)
    name="${1:?name}"; mkdir -p "$SHOTS"
    ssh "$STD" "xcrun simctl io $(udid) screenshot /tmp/$LANE-$name.png" >/dev/null 2>&1
    scp -q "$STD:/tmp/$LANE-$name.png" "$SHOTS/$name.png"; ssh "$STD" "rm -f /tmp/$LANE-$name.png"; echo "$SHOTS/$name.png" ;;
  record)   # a video of what the simulator shows: record start, then act (tap, type, ...), then record stop NAME [fps]: copies it to $CAL_SIM_SHOTS/NAME.mp4 and, with ffmpeg here, writes NAME-frames/f-%03d.png at fps (default 15)
    case "${1:-}" in
      start) ssh "$STD" "rm -f /tmp/$LANE-rec.mp4; nohup xcrun simctl io $(udid) recordVideo --codec h264 --force /tmp/$LANE-rec.mp4 >/tmp/$LANE-rec.log 2>&1 & echo \$! > /tmp/$LANE-rec.pid"; sleep 2; echo recording ;;
      stop) name="${2:?name}"; mkdir -p "$SHOTS"
        ssh "$STD" "kill -INT \$(cat /tmp/$LANE-rec.pid); sleep 3"; scp -q "$STD:/tmp/$LANE-rec.mp4" "$SHOTS/$name.mp4"
        if command -v ffmpeg >/dev/null; then mkdir -p "$SHOTS/$name-frames"; ffmpeg -loglevel error -y -i "$SHOTS/$name.mp4" -vf "fps=${3:-15},scale=393:-1" "$SHOTS/$name-frames/f-%03d.png"; fi
        echo "$SHOTS/$name.mp4" ;;
      *) echo "record start | record stop NAME [fps]" >&2; exit 2 ;;
    esac ;;
  setup)    # once per Studio: adds the rotate test to the runner (testRotate), allows ORIENTATION in ui.sh and rebuilds the runner; harmless to repeat
    HERE="$(cd "$(dirname "$0")" && pwd)"
    scp -q "$HERE/runner-rotate.swift.txt" "$STD:/tmp/runner-rotate.swift.txt"
    ssh "$STD" "cd ~/$SIMDIR && bash -s" <<'REMOTE'
set -e
if grep -q testRotate UITests/Runner.swift; then echo "runner already has testRotate"; else
  python3 - <<'PY'
p='UITests/Runner.swift'; s=open(p).read(); add=open('/tmp/runner-rotate.swift.txt').read()
i=s.rstrip().rfind('}'); open(p,'w').write(s[:i]+add+'}\n'); print('added testRotate')
PY
fi
grep -q ORIENTATION ui.sh || sed -i '' -e 's/URL|NAME|X|Y|DX|DY|SECONDS|WAIT|ACTIONS|BUNDLE|STRICT/URL|NAME|X|Y|DX|DY|SECONDS|WAIT|ACTIONS|BUNDLE|STRICT|ORIENTATION/' -e 's/testOpenURL|testAddToHomeScreen|testTap|testSwipe|testInspect|testTopCheck)/testOpenURL|testAddToHomeScreen|testTap|testSwipe|testInspect|testTopCheck|testRotate)/' ui.sh
./build-runner.sh 2>&1 | tail -3
REMOTE
    ;;
  rotate)   # portrait | landscapeLeft | landscapeRight | upsideDown
    ui testRotate "ORIENTATION=${1:-landscapeLeft}" "WAIT=${2:-2}" ;;
  home)     # add a page to the Home Screen (as an installed web app) and open it from there: home URL [name]
    ui testAddToHomeScreen "URL=${1:?url}" "NAME=${2:-Calternal lane}" STRICT=1 ;;
  serve)    # serve this lane's build to the simulator: vite preview on a dev VM, tunnelled to the Studio's 127.0.0.1:$PORT (VM: $CAL_DEV_VM; the lane's checkout there: $CAL_DEV_LANE, default Developer/lanes/<lane>; port: $CAL_SIM_PORT, default one per lane)
    VM="${CAL_DEV_VM:-ubuntu@calternal-dev-02-std}"; PORT="$(port)"; LD="${CAL_DEV_LANE:-Developer/lanes/$LANE}"
    ssh "$VM" "cd ~/$LD/apps/web && (nohup node ../../node_modules/.bin/vite preview --port $PORT --strictPort --host 127.0.0.1 >/tmp/preview-$PORT.log 2>&1 &); sleep 3; curl -s -o /dev/null -w 'vm %{http_code}\n' http://127.0.0.1:$PORT/"
    ssh -fN -L $PORT:127.0.0.1:$PORT "$VM"; ssh -fN -R $PORT:127.0.0.1:$PORT "$STD"
    ssh "$STD" "curl -s -o /dev/null -w 'studio %{http_code}\n' http://127.0.0.1:$PORT/"; echo "open: http://127.0.0.1:$PORT/<path>" ;;
  unserve)  PORT="$(port)"; pkill -f "ssh -fN -[LR] $PORT:127.0.0.1:$PORT" || true; pkill -f "ssh -N -[LR] $PORT:127.0.0.1:$PORT" || true
    ssh "${CAL_DEV_VM:-ubuntu@calternal-dev-02-std}" "pkill -u \$USER -f 'vite preview --port $PORT'" 2>/dev/null || true; echo "tunnels and preview stopped" ;;
  delete)   U=$(udid); ssh "$STD" "xcrun simctl shutdown $U 2>/dev/null; xcrun simctl delete $U"; rm -f "$UDIDFILE"; echo "deleted $U" ;;
  *) sed -n 2,5p "$0"; echo "commands: setup | rotate portrait|landscapeLeft|landscapeRight | home URL [name] | create [name] | appearance dark|light | open URL | onboard | tap x y [wait] | tappx px py | type TEXT | keys ACTIONS | swipe x y dx dy [s] | shot NAME | record start|stop NAME [fps] | serve | unserve | delete" ;;
esac
