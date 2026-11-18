#!/usr/bin/env bash
# Executed by the target's rootless Podman account. This uses only instance-owned
# units, roots, images and state. The old app is managed separately at cutover.
set -euo pipefail
umask 077
cd "$HOME"
die() { printf '%s\n' "$*" >&2; exit 1; }
log() { printf '%s\n' "$*"; }
[[ $# -ge 9 && $(id -u) != 0 ]] || die 'Use the target’s non-root deployment account.'
action=$1 name=$2 publish_ip=$3 publish_port=$4 root=$5 hdd=$6 hdd_mount=$7
secrets=$HOME/$8 sandbox_image=$9
shift 9
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}

image=localhost/$name
server=$name.service
machine=$name-machine.service
quadlets=$HOME/.config/containers/systemd
systemd=$HOME/.config/systemd/user
releases=$root/releases
public=$root/server.env
sysd() { systemctl --user "$@"; }
sysd show-environment >/dev/null || die 'Start the user systemd manager and enable linger.'
if [[ $hdd_mount != - ]]; then
  mountpoint -q -- "$hdd_mount" || die "Mount $hdd_mount before deploying."
fi
[[ $(realpath -m -- "$root") == "$root" && $(realpath -m -- "$hdd") == "$hdd" &&
   $(realpath -m -- "$root/appdata") == "$root/appdata" &&
   $(realpath -m -- "$root/userdata-ssd") == "$root/userdata-ssd" &&
   $hdd != "$root/appdata" && $hdd != "$root/userdata-ssd" &&
   $root != /srv/calternal-app && $root != /srv/calternal-app/* &&
   $hdd != /srv/calternal-app && $hdd != /srv/calternal-app/* ]] ||
  die 'Use canonical roots separate from the existing app.'
[[ -f $public && -f $secrets && $(stat -Lc %a -- "$secrets") == 600 ]] ||
  die "Provision $public and the mode-0600 $secrets before deploying."
origin=$(sed -n 's/^CALTERNAL_ORIGIN=//p' "$public")
[[ $origin =~ ^https://([a-zA-Z0-9.-]+)(:[0-9]{1,5})?$ ]] ||
  die "Set one HTTPS CALTERNAL_ORIGIN in $public."
authority=${origin#https://}
install -d -m 0700 "$releases" "$root/machine-handoff"
mkdir -p "$quadlets" "$systemd"
exec 9>"$releases/lock"
flock -n 9 || die 'Another deploy or rollback is running.'
[[ ! -L $releases/current || -d $releases/current ]] || die 'Current release pointer is broken.'
current=
if [[ -d $releases/current ]]; then
  current=$(realpath -- "$releases/current")
  [[ $current == "$releases"/release.* ]] || die 'Current release pointer leaves the release directory.'
fi

stop_unit() {
  local load
  load=$(sysd show --property=LoadState --value "$1") || return 1
  if [[ $load != not-found ]]; then sysd stop "$1"; fi
}

stop_roles() {
  stop_unit "$machine" && stop_unit "$server" || return 1
  if podman container exists "$name"; then
    log "The $name container survived the service stop."
    return 1
  fi
}

set_pointer() {
  ln -sfn -- "${1##*/}" "$releases/.$2-next" &&
    mv -Tf -- "$releases/.$2-next" "$releases/$2"
}

wait_for_http() {
  local deadline=$((SECONDS + 60))
  while ((SECONDS < deadline)); do
    if curl --noproxy '*' -fsS --max-time 2 -o /dev/null -H "Host: $authority" \
        "http://$publish_ip:$publish_port/api/v1/auth/status" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

activate() {
  local release=$1
  podman tag "$(<"$release/image.id")" "$image:current" &&
    set_pointer "$release" current &&
    install -m 0644 "$release/server.container" "$quadlets/$name.container" &&
    install -m 0644 "$release/machine.service" "$systemd/$machine" &&
    sysd daemon-reload || return 1
  if sysd is-failed --quiet "$server"; then sysd reset-failed "$server" || return 1; fi
  sysd start --no-block "$server" || return 1
  wait_for_http || { log 'The published HTTP listener did not become ready.'; return 1; }
  podman healthcheck run "$name" || { log 'The in-container health probe failed.'; return 1; }
  sysd enable "$machine" >/dev/null && sysd start "$machine"
}

restore() {
  stop_roles || return 1
  if [[ -n $current ]]; then
    activate "$current"
  else
    sysd disable "$machine" >/dev/null 2>&1 || true
    rm -f -- "$releases/current" "$quadlets/$name.container" "$systemd/$machine" || return 1
    if podman image exists "$image:current"; then
      podman untag "$image:current" "$image:current" || return 1
    fi
    sysd daemon-reload
  fi
}

discard_candidate() {
  local tag=$1 id=$2 present
  present=$(podman image inspect --format '{{.Id}}' "$image:$tag" 2>/dev/null || true)
  if [[ ${present#sha256:} == "$id" ]]; then
    podman untag "$id" "$image:$tag"
  fi
}

prepare_release() {
  local release=$1 id=$2
  local created engine
  engine=$(command -v podman) || return 1
  # Use a stopped, uniquely named container to extract the same static binary.
  created=$(podman create --network=none "$id") || return 1
  if ! podman cp "$created:/usr/local/bin/calternal" "$release/calternal"; then
    podman rm "$created" >/dev/null
    return 1
  fi
  podman rm "$created" >/dev/null || return 1
  chmod 0700 "$release/calternal" || return 1
  printf '%s\n' "$id" > "$release/image.id" || return 1
  sed -e "s|@NAME@|$name|g" -e "s|@PUBLISH_IP@|$publish_ip|g" \
    -e "s|@PUBLISH_PORT@|$publish_port|g" -e "s|@ROOT@|$root|g" \
    -e "s|@HDD_ROOT@|$hdd|g" -e "s|@HOME@|$HOME|g" \
    -e "s|@SECRETS_FILE@|${secrets##*/}|g" <<< "$server_template" > "$release/server.container" &&
    sed -e "s|@NAME@|$name|g" -e "s|@ROOT@|$root|g" \
      -e "s|@SANDBOX_IMAGE@|$sandbox_image|g" -e "s|@PODMAN@|$engine|g" -e "s|@ORIGIN@|$origin|g" \
      <<< "$machine_template" > "$release/machine.service"
}

initialize_roots() {
  local id=$1 bootstrap attempt ready=1
  install -d -m 0700 "$root/appdata" "$root/userdata-ssd" "$hdd" || return 1
  # Initialize under the same uid mapping, with no published socket and no --create
  # in the installed unit. The server's root guard rejects unsafe/foreign roots.
  bootstrap=$(podman run --detach --network=none --read-only --cap-drop=all \
    --security-opt=no-new-privileges --userns=keep-id:uid=10001,gid=10001 \
    --user=10001:10001 --stop-timeout=90 \
    --volume "$root/appdata:/data/appdata:Z" --volume "$root/userdata-ssd:/data/userdata-ssd:Z" \
    --volume "$hdd:/data/userdata-hdd:Z" --env-file "$secrets" --env-file "$public" "$id" --create) || return 1
  for ((attempt=0; attempt<30; attempt++)); do
    if podman healthcheck run "$bootstrap" >/dev/null 2>&1; then
      ready=0
      break
    fi
    [[ $(podman inspect --format '{{.State.Running}}' "$bootstrap") == true ]] || break
    sleep 1
  done
  podman stop --ignore "$bootstrap" >/dev/null && podman rm --ignore "$bootstrap" >/dev/null || return 1
  return "$ready"
}

remember_previous() {
  if [[ -n $current ]]; then
    podman tag "$(<"$current/image.id")" "$image:rollback" &&
      set_pointer "$current" previous
  fi
}

show_setup() {
  if curl --noproxy '*' -fsS --max-time 2 -H "Host: $authority" \
       "http://$publish_ip:$publish_port/api/v1/auth/status" |
       grep -q '"setup":true'; then
    log "On the target, run: podman exec $name /usr/local/bin/calternal setup-code --appdata /data/appdata"
    log "Open $origin/ to register the first owner."
    if [[ $1 == 1 ]]; then
      podman exec "$name" /usr/local/bin/calternal setup-code --appdata /data/appdata
    fi
  fi
}

case $action in
  deploy)
    [[ $# == 4 && $1 =~ ^[a-f0-9]{64}$ ]] || die 'Expected the transferred image ID.'
    id=$1 tag=$2 initialize=$3 reveal=$4
    podman image exists "$id" || die 'The transferred image is missing.'
    if [[ $initialize == 1 ]]; then
      [[ -z $current ]] || die 'Initialization is only for the first deployment.'
      initialize_roots "$id" || die 'Root initialization failed. Review the root guard and local journal.'
    fi
    [[ -d $root/appdata && -d $root/userdata-ssd && -d $hdd ]] ||
      die 'Storage directories are missing. Use --initialize only for first setup.'
    candidate=$(mktemp -d "$releases/release.XXXXXXXX")
    trap '[[ $(realpath -- "$releases/current" 2>/dev/null || true) == "$candidate" ]] || rm -rf -- "$candidate"' EXIT
    prepare_release "$candidate" "$id" || die 'Could not stage the image and units.'
    old_previous=
    if [[ -d $releases/previous ]]; then old_previous=$(realpath -- "$releases/previous"); fi
    stop_roles || die 'Could not stop the current Calternal-AI services.'
    if ! activate "$candidate"; then
      log "Release $tag failed to become healthy. Restoring the earlier services."
      restore || die 'Recovery failed. Keep the instance stopped and inspect its local journal.'
      discard_candidate "$tag" "$id"
      die 'Deployment failed.'
    fi
    remember_previous || die 'The new release is healthy, but the rollback pointer could not be updated.'
    if [[ $old_previous == "$releases"/release.* && $old_previous != "$current" ]]; then
      rm -rf -- "$old_previous"
    fi
    discard_candidate "$tag" "$id"
    if ! podman image prune --force --filter "label=org.calternal.deploy.repository=$image" >/dev/null; then
      log 'Could not reclaim untagged images for this instance.'
    fi
    log "Release $tag is healthy at $origin/."
    show_setup "$reveal"
    ;;
  rollback)
    [[ $# == 0 && -n $current && -d $releases/previous ]] || die 'No previous release is available.'
    previous=$(realpath -- "$releases/previous")
    stop_roles || die 'Could not stop the current Calternal-AI services.'
    if ! activate "$previous"; then
      restore || die 'Rollback and recovery both failed. Inspect the local journal.'
      die 'Rollback failed. The current release is back in service.'
    fi
    remember_previous || die 'Rollback is healthy, but its reverse pointer could not be updated.'
    log "Previous release is healthy at $origin/."
    ;;
  *) die 'Expected deploy or rollback.' ;;
esac
