# iPhone Simulator driver (`ios-sim.sh`)

Every phone-size UI change is looked at in **Safari on an iPhone Simulator** before it is reported, with the keyboard up where a text field is involved. Chromium's phone emulation does not have iOS Safari's keyboard, bars or viewport rules, so it is not a substitute. The simulators run on the Mac Studio (`kayg@std`); this script drives one that belongs to your lane through the Studio's XCUITest runner (`~/Developer/calternal-sim/ui.sh`, which holds the Studio's lock, so lanes never touch each other's runs).

```bash
S=apps/web/scripts/ios-sim.sh
export CAL_LANE=ui-threads            # defaults to the repository folder's name; keeps one simulator per lane
$S create                              # your own iPhone 18 Pro, iOS 27.0, booted (never another lane's)
$S appearance dark                     # or light
$S serve                               # vite preview of this lane's build on a dev VM, tunnelled to the Studio's 127.0.0.1:4180
$S open http://127.0.0.1:4180/threads/t-idle     # or a deployed fixtures build: http://10.69.69.222:4310/threads/t-idle (VPN, plain http)
$S onboard                             # a fresh Safari shows the one-time "Choose a look" panel: keep the default
$S tap 0.4 0.85 2                      # normalized (0..1 across the screen, y down), then wait 2 s
$S type "hello"                        # types with the software keyboard into the focused field (tap the field first)
$S swipe 0.5 0.4 0 0.3 0.5             # one finger: x y dx dy seconds
$S keys 'xy~200~760|wait~1|type~hi|key~return'   # a raw pipeline (xy is in points; also tap~Label, hold, pinch, two, shot)
$S shot name                           # screenshot to ./sim-shots/name.png
$S record start; $S tap 0.4 0.85 3; $S record stop focus 12   # a video, and with ffmpeg here its frames in ./sim-shots/focus-frames/
$S setup                               # once per Studio (idempotent): adds the rotate test to the runner and rebuilds it (a few minutes)
$S rotate landscapeLeft                # portrait | landscapeLeft | landscapeRight | upsideDown (the page lays out again; take a shot)
$S home http://127.0.0.1:4180/threads/t-idle "Lane preview"   # Add to Home Screen from Safari's share sheet and open it from there (slow: the runner waits through Safari's animations; give the command 10 minutes)
$S unserve; $S delete                  # clean up: stop the tunnels and the preview, delete your simulator
```

Rules: create and delete your own simulator and touch nothing else on the Studio; take the frames that matter (rest, keyboard shown, after the change), attach them to the lane's issue (not the repository), and say what you drove. Before you push UI: the screens you changed at phone size, light and dark, with the keyboard shown where a field is involved. A change to motion needs a video (`record`): a single screenshot cannot show a jump.

Notes: `home` drives the runner's own `testAddToHomeScreen` through Safari's Share sheet; if the command is cut short the sheet is left open: `$S tap 0.905 0.108` closes it. Landscape is a different layout (three panes in a short window): look at it too. Needs: ssh access to `kayg@std` and a dev VM, and (for `record stop` frames) `ffmpeg` on the machine you run it from. State is in `~/.cache/calternal-ios-sim` (the simulator id per lane); screenshots go to `$CAL_SIM_SHOTS` (default `./sim-shots`; do not commit them: `sim-shots/` is ignored).
