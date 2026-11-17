#!/usr/bin/env bash
# Manage only the disposable dev-01 installation and its user units.
set -euo pipefail
umask 077
cd -- "$(dirname -- "$0")/../.."
checkout=$PWD
root=$HOME/calternal-e2e
binary=$root/bin/calternal
local_port=34135
peer_port=34134
base_url=http://127.0.0.1:$local_port
origin=https://preview.calternal.cloud
installation=91c2ed9a-5a98-4b01-ac58-7376ab499c34
browser_installation=825bd622-44d8-4171-b5f7-c90a8702b34e
browser_port=34136
owner_token=$root/appdata/dev-tokens/owner.token

die() { echo "$*" >&2; exit 1; }
[[ $(hostname -s) == calternal-dev-01-compute02 && $(id -un) == claude ]] ||
  die 'Run on claude@calternal-dev-01-compute02.'

active() { systemctl --user is-active --quiet "calternal-e2e-$1.service"; }
start_unit() {
  local unit=$1
  shift
  if ! active "$unit"; then
    systemd-run --user --quiet --collect --unit="calternal-e2e-$unit" \
      --property=Type=exec --property=UMask=0077 \
      --setenv=NO_PROXY=127.0.0.1 --setenv=no_proxy=127.0.0.1 -- "$@"
  fi
}
stop_unit() {
  local state
  state=$(systemctl --user show -p LoadState --value "calternal-e2e-$1.service")
  if [[ $state == loaded ]]; then
    systemctl --user stop "calternal-e2e-$1.service"
  fi
}

# Only rotate a binary while both roles are stopped. Start reuses the last
# installed binary after the build target has been removed.
build() {
  local destination=$binary
  case ${1:-fixture} in
    fixture) ! active server && ! active machine || die 'Stop the installation before rebuilding.' ;;
    browser)
      ! active browser-server && ! active browser-machine ||
        die 'Stop the browser installation before rebuilding.'
      destination=$root/browser/bin/calternal
      ;;
    *) die 'Choose fixture or browser.' ;;
  esac
  nix develop -c bash -c '
    set -e
    npm ci --ignore-scripts --no-audit --no-fund
    npm run build:plugins
    cargo build --locked -j6 -p calternal-server
  '
  install -d -m 0700 "$(dirname -- "$destination")"
  install -m 0700 "${CARGO_TARGET_DIR:-target}/debug/calternal" "$destination"
}

cal() {
  CALTERNAL_URL=$base_url CALTERNAL_TOKEN_FILE=$owner_token "$binary" --json "$@"
}

wait_for() {
  local attempt
  for ((attempt=0; attempt<80; attempt++)); do
    if "$@" >/dev/null 2>&1; then return; fi
    sleep .25
  done
  die "Timed out waiting for $1. See service.sh logs."
}
server_ready() {
  active server && [[ -s $owner_token ]] &&
    curl -fsS --noproxy '*' --max-time 2 "$base_url/api/v1/auth/status"
}
machine_ready() {
  active machine &&
    printf 'Authorization: Bearer %s\n' "$(<"$owner_token")" |
      curl -fsS --noproxy '*' --max-time 2 --header @- "$base_url/api/v1/machines" |
      jq -e --arg id "$machine_id" '.machines[] | select(.id == $id and .state == "online")'
}
relay_ready() {
  active "$1" &&
    ss -ltnH "sport = :$peer_port" |
      awk -v listener="$address%wt0:$peer_port" '$4 == listener { found = 1 } END { exit !found }'
}

netbird_address() {
  local address
  address=$(ip -4 -o addr show dev wt0 scope global | awk '{split($4,a,"/"); print a[1]}')
  [[ $address =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'Expected one IPv4 address on NetBird wt0.'
  printf '%s\n' "$address"
}

start_relay() {
  start_unit "$1" "$(command -v socat)" \
    "TCP4-LISTEN:$peer_port,bind=$address,so-bindtodevice=wt0,reuseaddr,fork,max-children=64" \
    "TCP4:127.0.0.1:$local_port"
  wait_for relay_ready "$1"
}

enroll_machine() {
  if [[ ! -e $root/machine/credentials ]]; then
    # Pass the enrollment code directly to stdin. Neither role logs it.
    "$binary" --json machines code --appdata "$root/appdata" \
      --userdata-ssd "$root/userdata-ssd" --userdata-hdd "$root/userdata-hdd" \
      --installation-id "$installation" --account "$account" |
      jq -er '.data.code' |
      NO_PROXY=127.0.0.1 no_proxy=127.0.0.1 "$binary" machine enroll \
        --server "$base_url" --dir "$root/machine" \
        --name calternal-e2e-dev-01 --code - >/dev/null
  fi
  local identity
  identity=$("$binary" --json machine status --dir "$root/machine")
  [[ $(jq -er '.data.owner' <<<"$identity") == "account:$account" &&
     $(jq -er '.data.server' <<<"$identity") == "$base_url" ]] ||
    die 'The enrolled machine does not belong to the disposable account.'
  install -d -m 0700 "$root/machine/sandboxes/accounts/$account/home"
  jq -er '.data.machine_id' <<<"$identity"
}

start() {
  local mode=$1 address node current account machine_id
  [[ -x $binary ]] || die 'Run service.sh build first.'
  podman image exists "localhost/calternal-e2e-$mode:dev" ||
    die "Build the $mode image with image.sh $mode first."
  address=$(netbird_address)
  node=$(nix develop -c which node)
  start_unit server "$binary" serve --appdata "$root/appdata" --userdata-ssd "$root/userdata-ssd" \
    --userdata-hdd "$root/userdata-hdd" --installation-id "$installation" --create --development \
    --port "$local_port" --origin "$origin" --plugin-root "$checkout/plugins" --node "$node"
  wait_for server_ready
  account=$(cal describe | jq -er '.data.principal.id')
  machine_id=$(enroll_machine)
  current=$(systemctl --user show -p ExecStart --value calternal-e2e-machine.service)
  if active machine && [[ $current != *"localhost/calternal-e2e-$mode:dev"* ]]; then
    die 'Stop the installation before switching provider images. Use a fresh thread after switching.'
  fi
  start_unit machine "$binary" machine run --dir "$root/machine" --backend podman \
    --podman "$(command -v podman)" --image "localhost/calternal-e2e-$mode:dev" --max-sandboxes 2
  wait_for machine_ready
  start_relay relay
  printf 'NetBird listener  %s:%s\nAccount           %s\nSandbox home      %s\n' \
    "$address" "$peer_port" "$account" "$root/machine/sandboxes/accounts/$account/home"
}

browser_ready() {
  active browser-server &&
    curl -fsS --noproxy '*' --max-time 2 "http://127.0.0.1:$local_port/api/v1/auth/status"
}

# Start with an empty protected account directory. Only the operator redeems
# the resulting setup token with their actual browser authenticator.
start_browser() {
  local browser=$root/browser local_port=$browser_port peer_port=34137 address
  [[ -x $browser/bin/calternal ]] || die 'Run service.sh build browser first.'
  address=$(netbird_address)
  start_unit browser-server "$browser/bin/calternal" serve --appdata "$browser/appdata" \
    --userdata-ssd "$browser/userdata-ssd" --userdata-hdd "$browser/userdata-hdd" \
    --installation-id "$browser_installation" --create \
    --port "$local_port" --origin "$origin" --plugin-root "$checkout/plugins"
  wait_for browser_ready
  start_relay browser-relay
  printf 'NetBird listener  %s:%s\nSetup token path  %s\n' \
    "$address" "$peer_port" "$browser/appdata/setup.token"
}

# Browser enrollment uses the owner's one-time code. Switching images retains
# the enrollment and returns the home derived from the enrolled machine identity.
start_browser_machine() {
  local mode=$1 current identity owner account home code
  local root=$root/browser
  local binary=$root/bin/calternal base_url=http://127.0.0.1:$browser_port
  [[ $mode == fixture || $mode == claude ]] || die 'Choose fixture or claude.'
  active browser-server || die 'Start the browser installation first.'
  if [[ ! -e $root/machine/credentials ]]; then
    [[ $mode == fixture ]] || die 'Enroll the browser machine and verify the fixture first.'
    printf 'Enrollment code from Settings > Machines: ' >&2
    IFS= read -rs code || die 'Enter an enrollment code and press Enter.'
    printf '\n' >&2
    [[ -n $code ]] || die 'Enter the one-time enrollment code.'
    printf '%s\n' "$code" |
      NO_PROXY=127.0.0.1 no_proxy=127.0.0.1 "$binary" machine enroll \
        --server "$base_url" --dir "$root/machine" \
        --name calternal-e2e-dev-01 --code - >/dev/null
    unset code
  fi
  identity=$("$binary" --json machine status --dir "$root/machine")
  owner=$(jq -er '.data.owner' <<<"$identity")
  account=${owner#account:}
  [[ $owner == account:* && $account =~ ^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$ &&
     $(jq -er '.data.server' <<<"$identity") == "$base_url" ]] ||
    die 'Expected an account-owned machine enrolled with this browser server.'
  home=$root/machine/sandboxes/accounts/$account/home
  install -d -m 0700 "$home"
  if [[ -f ${home%/home}/fixture-verified ]]; then
    mode=claude
  elif [[ $mode == fixture ]]; then
    (shopt -s dotglob nullglob; entries=("$home"/*); ((${#entries[@]} == 0))) ||
      die 'Finish the fixture proof before placing provider sign-in files.'
  else
    die 'Finish the fixture proof before switching to Claude Code.'
  fi
  podman image exists "localhost/calternal-e2e-$mode:dev" ||
    die "Build the $mode image with image.sh $mode first."
  current=$(systemctl --user show -p ExecStart --value calternal-e2e-browser-machine.service)
  if active browser-machine && [[ $current != *"localhost/calternal-e2e-$mode:dev"* ]]; then
    stop_unit browser-machine
  fi
  start_unit browser-machine "$binary" machine run --dir "$root/machine" --backend podman \
    --podman "$(command -v podman)" --image "localhost/calternal-e2e-$mode:dev" --max-sandboxes 2
  printf '%s\n' "$home"
}

stop() {
  stop_unit relay
  stop_unit machine
  stop_unit server
}

# Reset the installation without traversing the account's provider login home.
# That old UUID's home remains on disk for the operator to handle.
reset() {
  [[ ${1:-} == --yes ]] || die 'Reset removes the disposable server roots and workspaces. Use reset --yes.'
  stop
  if [[ -e $root/machine/credentials ]]; then
    local identity owner account containers
    identity=$("$binary" --json machine status --dir "$root/machine")
    owner=$(jq -er '.data.owner' <<<"$identity")
    account=${owner#account:}
    [[ $owner == account:* && $account =~ ^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$ ]] ||
      die 'Cannot identify the disposable machine account.'
    containers=$(podman ps -aq --filter "name=^calternal-account-$account-thread-")
    if [[ -n $containers ]]; then
      printf '%s\n' "$containers" | xargs -r podman rm -f --ignore
    fi
    rm -rf -- "$root/machine/sandboxes/accounts/$account/threads"
    rm -f -- "$root/machine/credentials"
    printf 'Retained provider home for former account %s.\n' "$account"
  fi
  rm -rf -- "$root/appdata" "$root/userdata-ssd" "$root/userdata-hdd"
}

case ${1:-} in
  build) build "${2:-fixture}" ;;
  start)
    case ${2:-fixture} in
      fixture|claude) start "${2:-fixture}" ;;
      *) die 'Choose fixture or claude.' ;;
    esac
    ;;
  stop) stop ;;
  start-browser) start_browser ;;
  browser-machine) start_browser_machine "${2:-}" ;;
  stop-browser)
    stop_unit browser-relay
    stop_unit browser-machine
    stop_unit browser-server
    ;;
  reset) reset "${2:-}" ;;
  logs)
    journalctl --user --no-pager -n 100 -u 'calternal-e2e-*.service'
    ;;
  status)
    systemctl --user --no-pager status calternal-e2e-{server,machine,relay}.service
    ;;
  *) die 'Usage: service.sh build [fixture|browser]|start [fixture|claude]|stop|start-browser|stop-browser|reset --yes|logs|status' ;;
esac
