# Native UI study runner

A throwaway XCUITest project used by `docs/native-ui-study.md` to drive system apps in an iOS Simulator you create yourself
(never another device). It is documentation tooling, not product code, and is not part of any build.

- `Runner/Study.swift`: UI tests that launch Settings, Messages, Contacts, tap, swipe and long-press, and write each element's
  frame (`app.debugDescription`) and a screenshot into the app's tmp directory.
- `Host/main.swift`: an app that prints its own safe-area insets and every text style's real size and line height.
- `runtest.sh`: records the simulator with `simctl io recordVideo`, runs one test, and copies the results out.
- `bursts2.py`, `prog.py`: frame-difference analysis of a recording (`ffmpeg signalstats`) to time transitions.

Run (on the Mac Studio, in `~/Developer/lanes/native-ui-study`, simulator created with
`xcrun simctl create ... iPhone-18-Pro iOS-27-0`):

    xcodebuild build-for-testing -project Study.xcodeproj -scheme Host -destination id=$UDID -derivedDataPath ../dd CODE_SIGNING_ALLOWED=NO
    sh runtest.sh testPush
