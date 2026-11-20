#!/bin/sh
# Copies the tracked files of apps/ios and apps/apple to the Mac Studio's checkout directory (kayg@std:~/Developer/Calternal-AI).
# That directory is a plain rsync target, not a git checkout: it keeps its own untracked Local.xcconfig,
# .env.speedflight and build/, which this never touches or deletes.
# Usage: ./sync-to-mac.sh   (from any machine that can ssh to kayg@std)
set -eu
cd "$(dirname "$0")/../.."
git ls-files apps/ios apps/apple contract | rsync -a --files-from=- . kayg@std:Developer/Calternal-AI/
