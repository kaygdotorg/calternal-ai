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

installation=$(sed -n 's/^CALTERNAL_INSTALLATION_ID=//p' "$public")
[[ $installation =~ ^[a-fA-F0-9-]{36}$ ]] || die 'Set the installation UUID in server.env.'

run_upgrade() {
  local release=$1
  shift
  "$release/calternal" --json upgrade --appdata "$root/appdata" --userdata-ssd "$root/userdata-ssd" \
    --userdata-hdd "$hdd" --installation-id "$installation" --machine-dir "$root/machine" "$@"
}

upgrade_quiet() {
  local output
  if ! output=$(run_upgrade "$@"); then
    printf '%s\n' "$output" >&2
    return 1
  fi
}

refers_to_candidate() {
  local status=$1 id=$2
  grep -Eq '"phase"[[:space:]]*:[[:space:]]*"committed"' <<< "$status" &&
    grep -Eq '"candidate"[[:space:]]*:[[:space:]]*"'"$id"'"' <<< "$status"
}

clear_recovery_pin() {
  unlink "$releases/pending-recovery" && sync "$releases"
}

seal_activation() {
  sync "$quadlets/$name.container" "$systemd/$machine" "$quadlets" "$systemd" "$releases" &&
    clear_recovery_pin
}

stop_probe() {
  podman stop --ignore "$name-upgrade-probe" >/dev/null &&
    podman rm --ignore "$name-upgrade-probe" >/dev/null
}

recovered_activation=
recover_pending() {
  [[ -n $current ]] || return 0
  local status pinned id
  if [[ -L $releases/pending-recovery ]]; then
    pinned=$(realpath -- "$releases/pending-recovery") || return 1
    [[ $pinned == "$releases"/release.* && -d $pinned ]] || return 1
    id=$(<"$pinned/image.id")
    [[ $id =~ ^[a-f0-9]{64}$ ]] || return 1
    status=$(run_upgrade "$pinned" status) || return 1
    if refers_to_candidate "$status" "$id"; then
      # The committed decision wins even when activation was interrupted.
      if [[ $current != "$pinned" ]]; then
        remember_previous || return 1
      fi
      stop_roles && upgrade_quiet "$pinned" recover && activate "$pinned" || return 1
      current=$pinned
      seal_activation || return 1
      recovered_activation=$id
      return 0
    elif grep -Eq '"needs_recovery"[[:space:]]*:[[:space:]]*true' <<< "$status"; then
      stop_roles && stop_probe && upgrade_quiet "$pinned" recover && activate "$current" || return 1
    fi
    clear_recovery_pin || return 1
    if [[ $pinned != "$current" ]]; then rm -rf -- "$pinned" || return 1; fi
    return 0
  fi
  status=$(run_upgrade "$current" status) || return 1
  if grep -Eq '"needs_recovery"[[:space:]]*:[[:space:]]*true' <<< "$status"; then
    stop_roles && upgrade_quiet "$current" recover && activate "$current"
  fi
}

pin_recovery() {
  local release=$1 stage
  printf '%s\n' "$name-upgrade-probe" > "$release/probe.name" || return 1
  # The shipped Quadlet calls the current release's launcher. Update that
  # launcher atomically before publishing any v2 intent.
  stage=$(mktemp "$current/.recover-start.XXXXXXXX") || return 1
  install -m 0700 "$release/recover-start.sh" "$stage" &&
    sync "$stage" && mv -Tf -- "$stage" "$current/recover-start.sh" &&
    sync "$current" "$release/calternal" "$release/probe.name" "$release/image.id" \
      "$release/server.container" "$release/machine.service" "$release/recover-start.sh" "$release" &&
    set_pointer "$release" pending-recovery && sync "$releases"
}

probe_candidate() {
  local id=$1 attempt result=1
  podman run --detach --name "$name-upgrade-probe" --network=none --read-only --cap-drop=all \
    --security-opt=no-new-privileges --userns=keep-id:uid=10001,gid=10001 \
    --user=10001:10001 --stop-timeout=90 \
    --volume "$root/appdata:/data/appdata:Z" --volume "$root/userdata-ssd:/data/userdata-ssd:Z" \
    --volume "$hdd:/data/userdata-hdd:Z" --env-file "$secrets" --env-file "$public" \
    --entrypoint /usr/local/bin/calternal "$id" serve --bind-ip 127.0.0.1 --port 8080 \
    --appdata /data/appdata --userdata-ssd /data/userdata-ssd --userdata-hdd /data/userdata-hdd \
    --web-dir /app/web --plugin-root /app/plugins --upgrade-probe "$id" >/dev/null || return 1
  for ((attempt=0; attempt<60; attempt++)); do
    if podman healthcheck run "$name-upgrade-probe" >/dev/null 2>&1; then
      result=0
      break
    fi
    [[ $(podman inspect --format '{{.State.Running}}' "$name-upgrade-probe") == true ]] || break
    sleep 1
  done
  if ((result)); then podman logs "$name-upgrade-probe" >&2 || true; fi
  stop_probe && return "$result"
}

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
  printf '%s\n' "$recovery_template" > "$release/recover-start.sh" || return 1
  chmod 0700 "$release/recover-start.sh" || return 1
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
    [[ $# == 5 && $1 =~ ^[a-f0-9]{64}$ && $5 =~ ^[01]$ ]] || die 'Expected the transferred image ID.'
    id=$1 tag=$2 initialize=$3 reveal=$4 incompatible=$5
    podman image exists "$id" || die 'The transferred image is missing.'
    if [[ $initialize == 1 ]]; then
      [[ -z $current ]] || die 'Initialization is only for the first deployment.'
      initialize_roots "$id" || die 'Root initialization failed. Review the root guard and local journal.'
    fi
    [[ -d $root/appdata && -d $root/userdata-ssd && -d $hdd ]] ||
      die 'Storage directories are missing. Use --initialize only for first setup.'
    recover_pending || die 'Could not recover the pending deployment. Keep both roles stopped.'
    if [[ $recovered_activation == "$id" ]]; then
      discard_candidate "$tag" "$id"
      log "Release $tag is healthy at $origin/."
      show_setup "$reveal"
      exit 0
    fi
    if [[ -n $current && $incompatible == 0 && -z ${compatibility_proof:-} ]]; then
      die 'Supply the compatibility proof for the two release images.'
    fi
    candidate=$(mktemp -d "$releases/release.XXXXXXXX")
    trap '[[ $(realpath -- "$releases/current" 2>/dev/null || true) == "$candidate" ||
             $(realpath -- "$releases/pending-recovery" 2>/dev/null || true) == "$candidate" ]] ||
             rm -rf -- "$candidate"' EXIT
    prepare_release "$candidate" "$id" || die 'Could not stage the image and units.'
    old_previous=
    if [[ -d $releases/previous ]]; then old_previous=$(realpath -- "$releases/previous"); fi
    if [[ -n $current ]]; then
      declare -a approval
      previous_id=$(<"$current/image.id")
      if [[ $incompatible == 1 ]]; then
        approval=(--incompatible)
      else
        printf '%s\n' "$compatibility_proof" > "$candidate/upgrade-proof.json"
        approval=(--proof "$candidate/upgrade-proof.json")
      fi
      upgrade_quiet "$candidate" preflight --previous "$previous_id" --candidate "$id" \
        "${approval[@]}" || die 'Upgrade preflight refused. The current services are still running.'
      if [[ $incompatible == 1 ]]; then
        pin_recovery "$candidate" || die 'Could not pin the candidate recovery executable.'
      fi
    fi
    stop_roles || die 'Could not stop the current Calternal-AI services.'
    if [[ -n $current ]]; then
      if ! upgrade_quiet "$candidate" prepare --previous "$previous_id" --candidate "$id" "${approval[@]}" ||
         { [[ $incompatible == 1 ]] && ! probe_candidate "$id"; }; then
        upgrade_quiet "$candidate" recover &&
          restore || die 'Data recovery failed. Keep the instance stopped.'
        if [[ $incompatible == 1 ]]; then
          clear_recovery_pin || die 'Could not retire the completed recovery pin.'
        fi
        die 'Upgrade preparation failed. The previous release is serving its original data.'
      fi
      if ! upgrade_quiet "$candidate" commit --candidate "$id"; then
        status=$(run_upgrade "$candidate" status) || die 'Cannot inspect the cutover decision. Keep both roles stopped.'
        if refers_to_candidate "$status" "$id"; then
          die 'The cutover is committed. Retry deployment to finish recovery and activation.'
        fi
        upgrade_quiet "$candidate" recover &&
          restore || die 'Data recovery failed. Keep the instance stopped.'
        if [[ $incompatible == 1 ]]; then
          clear_recovery_pin || die 'Could not retire the completed recovery pin.'
        fi
        die 'Upgrade commit refused. The previous release is serving its original data.'
      fi
    fi
    if [[ $incompatible == 1 ]]; then
      remember_previous || die 'The cutover is committed. Retry deployment to finish activation.'
    fi
    if ! activate "$candidate"; then
      if [[ $incompatible == 1 ]]; then
        stop_roles || true
        die 'The incompatible release could not start. Keep it stopped for the owner.'
      fi
      log "Release $tag failed to become healthy. Restoring the earlier services."
      restore || die 'Recovery failed. Keep the instance stopped and inspect its local journal.'
      discard_candidate "$tag" "$id"
      die 'Deployment failed.'
    fi
    if [[ $incompatible == 1 ]]; then
      seal_activation || die 'The new release is healthy, but its recovery pin could not be retired.'
    else
      remember_previous || die 'The new release is healthy, but the rollback pointer could not be updated.'
    fi
    if [[ $old_previous == "$releases"/release.* && $old_previous != "$current" ]]; then
      rm -rf -- "$old_previous"
    fi
    discard_candidate "$tag" "$id"
    log "Release $tag is healthy at $origin/."
    show_setup "$reveal"
    ;;
  rollback)
    [[ $# == 0 && -n $current ]] || die 'No current release is available.'
    recover_pending || die 'Could not recover the pending deployment.'
    [[ -d $releases/previous ]] || die 'No previous release is available.'
    previous=$(realpath -- "$releases/previous")
    stop_roles || die 'Could not stop the current Calternal-AI services.'
    if ! upgrade_quiet "$previous" verify || ! activate "$previous"; then
      restore || die 'Rollback and recovery both failed. Inspect the local journal.'
      die 'Rollback failed. The current release is back in service.'
    fi
    remember_previous || die 'Rollback is healthy, but its reverse pointer could not be updated.'
    log "Previous release is healthy at $origin/."
    ;;
  *) die 'Expected deploy or rollback.' ;;
esac
