#!/usr/bin/env bash
# Verify rollback loads the old CLI arguments before the old release starts.
set -euo pipefail

checkout=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
fixture=$(mktemp -d "$checkout/.preview-rollback-test.XXXXXXXX")
trap 'rm -rf -- "$fixture"' EXIT
export HOME="$fixture/home" MOCK_ROOT="$fixture"
root="$HOME/Developer/lanes/chat-preview"
unit_dir="$HOME/.config/systemd/user"
old="$root/build/release.OLD12345"
mkdir -p "$fixture/bin" "$HOME/bin" "$unit_dir" "$old"
unit="$unit_dir/calternal-chat-preview.service"
printf 'ExecStart=previous-host previous-arguments\n' >"$fixture/old.service"
cp "$fixture/old.service" "$unit"
cp "$unit" "$fixture/loaded.service"
ln -s "$old" "$root/current"
cat >"$HOME/bin/lock-shared" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
touch "${!#}/calternal-host"
MOCK
cat >"$fixture/bin/systemctl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
root="$HOME/Developer/lanes/chat-preview"
unit="$HOME/.config/systemd/user/calternal-chat-preview.service"
case "$2" in
    is-active|stop) ;;
    daemon-reload) cp "$unit" "$MOCK_ROOT/loaded.service" ;;
    start)
        if [[ $(readlink "$root/current") == "$root/build/release.OLD12345" ]]; then
            cmp "$MOCK_ROOT/old.service" "$MOCK_ROOT/loaded.service"
            touch "$MOCK_ROOT/previous-started"
        else
            exit 1
        fi
        ;;
    *) exit 2 ;;
esac
MOCK
chmod +x "$HOME/bin/lock-shared" "$fixture/bin/systemctl"
export PATH="$fixture/bin:$PATH"
if "$checkout/scripts/dev-chat-preview.sh"; then
    printf 'The mock replacement should fail to start.\n' >&2
    exit 1
fi
test -f "$fixture/previous-started"
cmp "$fixture/old.service" "$unit"
test "$(readlink "$root/current")" = "$old"
