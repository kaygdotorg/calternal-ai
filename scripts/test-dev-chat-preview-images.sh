#!/usr/bin/env bash
# Verify that building another release cannot retarget a previous release's image.
set -euo pipefail

checkout=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
fixture=$(mktemp -d "$checkout/.preview-image-test.XXXXXXXX")
trap 'rm -rf -- "$fixture"' EXIT
cd "$fixture"
mkdir -p scripts bin target/debug apps/web/dist packages/plugin-sdk/dist \
    plugins-v2/calternal-chat plugins-v2/calternal-chat-claude-code \
    plugins-v2/calternal-reading-list plugins-v2/calternal-tasks \
    release.ABCDE123 release.FGHIJ456
cp "$checkout/scripts/dev-chat-preview.sh" scripts/
touch target/debug/calternal-host packages/plugin-sdk/package.json

cat >scripts/dev-chat-host.sh <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${CALTERNAL_DEV_FAKE_IMAGE:-missing}" >> image-builds
MOCK
cat >bin/podman <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ $1 == image && $2 == inspect && $3 == --format ]]
printf '%s' "$5" | sha256sum | cut -d' ' -f1
MOCK
chmod +x bin/podman scripts/dev-chat-host.sh
export PATH="$fixture/bin:$PATH" CARGO_TARGET_DIR="$fixture/target"
for name in release.ABCDE123 release.FGHIJ456; do
    scripts/dev-chat-preview.sh --build-release "$fixture/$name"
    tag="localhost/calternal-dev-chat-fake:$name"
    [[ $(cat "$name/image.tag") == "$tag" ]]
    expected=$(podman image inspect --format '{{.Id}}' "$tag")
    grep -Fxq "CALTERNAL_RUN_IMAGE=$expected" "$name/runtime.env"
done
mapfile -t builds <image-builds
[[ ${#builds[@]} == 2 && ${builds[0]} != "${builds[1]}" ]]
! cmp -s release.ABCDE123/runtime.env release.FGHIJ456/runtime.env
