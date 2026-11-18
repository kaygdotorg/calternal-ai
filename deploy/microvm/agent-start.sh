#!/bin/sh
set -eu
export HOME=/data/home USER=agent XDG_RUNTIME_DIR=/run/user/1000 TMPDIR=/tmp
exec /opt/calternal-microvm-agent
