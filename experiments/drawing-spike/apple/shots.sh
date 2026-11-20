#!/bin/sh
# Captures the SwiftUI spike's screens, light and dark, into shots/ (fixture data only; evidence goes on Forgejo #107).
set -eu
cd "$(dirname "$0")"
shoot() { name=$1; shift; ./run.sh run "$@" >/dev/null; sleep 4; ./run.sh shot "ios-$scheme-$name" >/dev/null; }
for scheme in light dark; do
  ./run.sh $scheme
  shoot rl-sidebar -app reading-list
  shoot rl-list -app reading-list -open All -view list
  shoot rl-board -app reading-list -open All -view board
  shoot rl-add -app reading-list -open Unread -sheet new
  shoot chat-sidebar -app calternal-chat
  shoot chat -app calternal-chat -item c1
  shoot chat-top -app calternal-chat -item c1 -top YES
  shoot chat-reply -app calternal-chat -item c1 -quote a26
  shoot chat-edit -app calternal-chat -item c1 -action edit:a22
  shoot chat-delete -app calternal-chat -item c1 -action delete:a22
  shoot chat-ask -app calternal-chat -item c1 -action ask:a25
done
ls shots
