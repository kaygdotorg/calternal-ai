#!/bin/sh
set -eu
export HOME=/data/home USER=agent XDG_RUNTIME_DIR=/run/user/1000
# The disposable image store is separate from durable homes and workspaces.
if ! cmp -s /opt/standard.sha256 /data/home/.standard-image; then
  podman system reset --force
  podman load --input /opt/standard.tar
  cp /opt/standard.sha256 /data/home/.standard-image.pending
  mv /data/home/.standard-image.pending /data/home/.standard-image
  sync -f /data
fi
exec /opt/calternal-microvm-agent
