#!/bin/bash
# Restart the demo server on dev-02 (port 4312). The built harness must exist (npm run harness:build -w @calternal/description).
cd ~/Developer/calternal-ai-lanes/v2-renderer-web/packages/description || exit 1
pkill -f "node demo/server.mjs" 2>/dev/null; sleep 1
DEMO_PORT=4312 setsid nohup node demo/server.mjs > ~/Developer/calternal-ai-lanes/demo.log 2>&1 < /dev/null &
sleep 1; curl -s -o /dev/null -w "demo http %{http_code}\n" http://127.0.0.1:4312/
