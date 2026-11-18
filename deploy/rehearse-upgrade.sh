#!/usr/bin/env bash
# Ten disposable dev-01 handoffs using the production deployment driver.
set -euo pipefail
umask 077
[[ $(hostname -s) == calternal-dev-01-compute02 && $(id -un) == claude ]] ||
  { printf '%s\n' 'Run this only on dev-01 as claude, inside nix develop.' >&2; exit 2; }
[[ $# == 3 && $1 =~ ^calternal-ai-u46-[a-z0-9]+$ && -f $3 ]] ||
  { printf '%s\n' 'Usage: rehearse-upgrade.sh INSTANCE NETBIRD_IP PROOF' >&2; exit 2; }
name=$1 publish_ip=$2 proof=$(realpath -- "$3")
image=localhost/$name
host=claude@localhost
root=$(mktemp -d "$HOME/$name.XXXXXXXX")
secrets=$HOME/$name.env
(secrets_file=$secrets; set -C; : > "$secrets_file")
proxy=
cleanup() {
  local result=$?
  trap - EXIT
  systemctl --user stop "$name-machine.service" "$name.service" 2>/dev/null || true
  systemctl --user disable "$name-machine.service" 2>/dev/null || true
  unlink "$HOME/.config/containers/systemd/$name.container" 2>/dev/null || true
  unlink "$HOME/.config/systemd/user/$name-machine.service" 2>/dev/null || true
  systemctl --user daemon-reload
  if [[ -n $proxy ]]; then kill "$proxy" 2>/dev/null || true; wait "$proxy" 2>/dev/null || true; fi
  unlink "$secrets"
  if ((result)); then
    journalctl --user -u "$name.service" -u "$name-machine.service" -n 35 --no-pager >&2 || true
  fi
  find "$root" -depth -delete
  exit "$result"
}
trap cleanup EXIT

# Same-VM shell transport for the driver. The operator's SSH hop to dev-01
# runs this entire rehearsal. The shared VM's loopback SSH policy is untouched.
mkdir "$root/bin"
cat > "$root/bin/ssh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ $# == 4 && $1 == -o && $2 == BatchMode=yes && $3 == claude@localhost ]]
exec bash -c "$4"
SH
chmod 0700 "$root/bin/ssh"
export PATH="$root/bin:$PATH" CALTERNAL_REHEARSAL_UNIT=$name
read -r backend tls machine < <(python3 - "$publish_ip" <<'PY'
import socket
import sys
sockets = [socket.socket() for _ in range(3)]
for sock, ip in zip(sockets, [sys.argv[1], "127.0.0.1", "127.0.0.1"]):
    sock.bind((ip, 0))
print(*(sock.getsockname()[1] for sock in sockets))
PY
)
origin=https://localhost:$tls
mkdir "$root/cert"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=localhost \
  -addext subjectAltName=DNS:localhost -keyout "$root/cert/proxy.key" \
  -out "$root/cert/proxy.crt" >/dev/null 2>&1
node deploy/rehearsal-proxy.mjs "localhost:$tls" "$publish_ip" "$backend" "$tls" "$machine" \
  "$root/cert" 127.0.0.1 &
proxy=$!

printf 'CALTERNAL_INSTALLATION_ID=%s\nCALTERNAL_ORIGIN=%s\nCALTERNAL_TRUSTED_PROXIES=%s/32\n' \
  "$(cat /proc/sys/kernel/random/uuid)" "$origin" "$publish_ip" > "$root/server.env"
cat > "$root/target.conf" <<TARGET
HOST=$host
NAME=$name
PUBLISH_IP=$publish_ip
PUBLISH_PORT=$backend
ROOT=$root
HDD_ROOT=$root/userdata-hdd
HDD_MOUNT=-
SECRETS_FILE=${secrets##*/}
SANDBOX_IMAGE=$image:sandbox
TARGET
podman tag "$image:baseline-source" "$image:baseline"
deploy/deploy.sh --target "$root/target.conf" --image baseline --initialize
node deploy/rehearse.mjs upgrade-setup "$origin" "$root"
for ((attempt=0; attempt<20; attempt++)); do
  if [[ -f $root/machine/credentials ]]; then break; fi
  sleep 1
done
# The disposable server uses a self-signed browser certificate. The machine
# uses a private plain HTTP route to the same proxy and authenticated backend.
systemctl --user stop "$name-machine.service"
sed -i "s|^server: .*|server: http://127.0.0.1:$machine|" "$root/machine/credentials"
chmod 0600 "$root/machine/credentials"
systemctl --user start "$name-machine.service"
podman tag "$image:candidate-source" "$image:candidate"
node deploy/rehearse.mjs upgrade "$origin" "$root" "$root/target.conf" candidate "$proof"
