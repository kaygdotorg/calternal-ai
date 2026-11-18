# Shared by the two build-host entry points. Target configs are operator-owned Bash.
load_target() {
  # shellcheck disable=SC1090
  source "$1"
  [[ $HOST =~ ^[a-z_][a-z0-9_-]*@[a-zA-Z0-9][a-zA-Z0-9.-]*$ &&
     $NAME =~ ^calternal-ai(-[a-z0-9-]+)?$ &&
     $PUBLISH_IP =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ &&
     $PUBLISH_PORT =~ ^[1-9][0-9]{0,4}$ && $PUBLISH_PORT -le 65535 &&
     $ROOT =~ ^/[a-zA-Z0-9_./-]+$ && $HDD_ROOT =~ ^/[a-zA-Z0-9_./-]+$ &&
     ( $HDD_MOUNT == - || $HDD_MOUNT =~ ^/[a-zA-Z0-9_./-]+$ ) &&
     $SECRETS_FILE =~ ^[a-zA-Z0-9_.-]+\.env$ &&
     $SANDBOX_IMAGE =~ ^[a-zA-Z0-9_./:@-]+$ ]] ||
    die 'Invalid target configuration. See deploy/cloud.conf.'
  image=localhost/$NAME
  target_args=("$NAME" "$PUBLISH_IP" "$PUBLISH_PORT" "$ROOT" "$HDD_ROOT" "$HDD_MOUNT" "$SECRETS_FILE" "$SANDBOX_IMAGE")
}

remote() {
  local command
  printf -v command '%q ' "$@"
  if ((dry_run)); then
    printf 'ssh -o BatchMode=yes %q %q\n' "$HOST" "$command"
  else
    ssh -o BatchMode=yes "$HOST" "$command"
  fi
}

print_remote_script() {
  printf '  stdin: %s\n' "$here/remote.sh"
}
