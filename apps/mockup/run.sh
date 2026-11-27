#!/bin/sh
# Restarts the mockup server on this machine (dev-02: http://10.69.69.222:4313/). Build first: `npm run build` in apps/mockup.
cd "$(dirname "$0")" || exit 1
systemctl --user restart calternal-mockup.service 2>/dev/null && { echo "restarted (systemd): http://10.69.69.222:4313/"; exit 0; }
pkill -f "node serve.mjs" 2>/dev/null; sleep 1
nohup node serve.mjs 4313 > "${MOCKUP_LOG:-$HOME/mockup.log}" 2>&1 &
echo "started: http://10.69.69.222:4313/"
