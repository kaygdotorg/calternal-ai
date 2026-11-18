#!/bin/sh
# Root exists only inside the guest. Following the measured hybrid image, run
# Podman and its control agent as uid 1000 in a delegated cgroup.
set -eu
export PATH=/usr/bin:/bin:/sbin
mount -t proc proc /proc
mount -t sysfs sysfs /sys
# The pinned Firecracker kernel mounts devtmpfs before launching PID 1.
if ! awk '$2 == "/dev" && $3 == "devtmpfs" { found=1 } END { exit !found }' /proc/mounts; then
  mount -t devtmpfs devtmpfs /dev
fi
mkdir -p /dev/pts /dev/shm
mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm
mount -t tmpfs tmpfs /tmp
mount -t tmpfs tmpfs /run
mount -t cgroup2 none /sys/fs/cgroup
mount -t ext4 -o nodev,nosuid /dev/vdb /data
chown 1000:1000 /dev/vdc
chmod 400 /dev/vdc
ip link set lo up
for arg in $(cat /proc/cmdline); do
  case "$arg" in
    calternal_net=*)
      settings=${arg#calternal_net=}
      address=${settings%%,*}
      settings=${settings#*,}
      gateway=${settings%%,*}
      dns=${settings#*,}
      # slirp runs as the guest agent. It opens the guest-local TUN device
      # and does not receive the host TAP or host network privileges.
      chown 1000:1000 /dev/net/tun
      chmod 600 /dev/net/tun
      ip link set eth0 up
      ip addr add "$address" dev eth0
      ip route add default via "$gateway" dev eth0
      printf 'nameserver %s\n' "$dns" > /run/calternal-resolv.conf
      export CALTERNAL_GUEST_NETWORK=1
      ;;
  esac
done
if [ ! -e /run/calternal-resolv.conf ]; then
  touch /run/calternal-resolv.conf
fi

echo "+cpu +memory +pids" > /sys/fs/cgroup/cgroup.subtree_control
mkdir /sys/fs/cgroup/agent
echo "+cpu +memory +pids" > /sys/fs/cgroup/agent/cgroup.subtree_control
mkdir /sys/fs/cgroup/agent/ctl /sys/fs/cgroup/agent/ctr
echo "+cpu +memory +pids" > /sys/fs/cgroup/agent/ctr/cgroup.subtree_control
chown -R 1000:1000 /sys/fs/cgroup/agent

mkdir -p /run/agent /run/user/1000 /data/home /data/sandboxes
chown 1000:1000 /run/agent /run/user/1000 /data/home /data/sandboxes
chmod 700 /run/agent /run/user/1000
sh -ec '
  echo "$$" > /sys/fs/cgroup/agent/ctl/cgroup.procs
  exec chpst -u agent /sbin/agent-start
' &
agent=$!
socat VSOCK-LISTEN:5000,reuseaddr,fork,max-children=512 UNIX-CONNECT:/run/agent/control.sock &
relay=$!
trap 'kill "$relay" "$agent" 2>/dev/null || true; sync; poweroff -f' TERM
wait "$agent"
kill "$relay"
sync
poweroff -f
