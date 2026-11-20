# iOS keyboard spike (throwaway)

Proves, on a real iPhone, the approaches a Calternal keyboard needs before any architecture
work (Forgejo #105; facts in #104 and `docs/research/ios-keyboard-study-2026-11-20.md`). Plain
Swift, no plugins, no shared Rust core. Not product code: delete it once the findings are in.

## What is in it

| Part | Where | What it proves |
| --- | --- | --- |
| In-keyboard dictation | `Keyboard/WebMicCapture.swift`, `Keyboard/KeyboardDictation.swift`, `Shared/Transcriber.swift` | Microphone capture inside the keyboard through a hidden `WKWebView` (`getUserMedia`, PCM to Swift), transcribed in the extension by Apple's on-device model (`SpeechTranscriber`, iOS 26+) while you speak. The method is Hex's (MIT, see `THIRD_PARTY_NOTICES.md`). |
| Fallback dictation | `App/WarmSession.swift` | The app keeps a microphone session for 5 minutes; the keyboard's mic key then records through the app over Darwin notifications, with no app switch. Used automatically when in-keyboard capture cannot start (no permission, low memory); the first time, the keyboard opens the app (`kbspike://dictate`). |
| Clipboard history | `Keyboard/KeyboardViewController.swift`, `Shared/ClipboardHistory.swift` | Reads the pasteboard when the keyboard appears (Full Access), keeps 50 items in the shared store, shows them as chips; the system Paste control (`UIPasteControl`) inserts without a prompt. |
| Clipboard watcher | `App/ClipboardWatcher.swift` | PastePal's trick: a picture-in-picture "video" keeps the app alive off screen while it watches `changeCount`. Logs whether the text can be read in the background and the battery level each minute. |
| Keyboard | `Keyboard/KeyboardViewController.swift`, `Shared/Tokens.swift` | Letters, numbers, symbols, shift and caps lock, delete with repeat, return, the globe key when iOS asks for it, the dictation key, the strip. Every colour, the key radius and the fonts come from `Shared/Tokens.swift`. |

Keys are drawn by the keyboard because iOS gives a keyboard extension no system key control;
everything else (input view background, globe menu, Paste control, input clicks, the app's
`Form`) is the system's.

## Evidence

Every step writes a line to the evidence log in the shared store: dictation tap → microphone live,
tap → first words, stop → final text, memory (footprint, peak, headroom), clipboard reads and
their duration, picture-in-picture state and battery. The app shows the log under **Log** with
a share button.

## Build (Mac Studio)

Sources are edited on hd01 and copied to `~/Developer/Calternal-AI/experiments/ios-keyboard` on
`kayg@std`. Identifiers live in the untracked `Local.xcconfig` (see `Local.xcconfig.example`);
the repository holds placeholders only.

| Script | Does |
| --- | --- |
| `./sim.sh [name]` | Builds for this lane's Simulator (`build/sim-udid`), installs, launches with the test field focused, screenshots to `build/shots/`. Launch flags: `-SpikeFocus YES`, `-SpikeTranscribeFile <wav>`. |
| `./archive.sh [build]` | Release archive for iPhone, unsigned: `build/KBSpike.xcarchive`. |
| `./install-on-phone.sh [udid]` | Archives, exports for the registered iPhone with Apple cloud signing (registers the two bundle identifiers if missing) and installs over the network with `devicectl`. |

**No App Group.** The App Store Connect key gets 401 on `appGroups`, so cloud signing cannot
register one. The app and keyboard share data through a keychain access group instead
(`SPIKE_KEYCHAIN_GROUP`, allowed by ordinary profiles; `SharedStore` in `Shared/Spike.swift`).
A product build needs the App Group created on developer.apple.com by the account holder.

## On the iPhone

1. Open **KB Spike**: tap **Allow microphone**, then **Download speech model**.
2. Settings › Apps › KB Spike › Keyboards: turn on **Calternal Spike** and **Allow Full Access**.
3. In any app, hold the globe and pick Calternal Spike. Tap the mic key, speak, tap it again.
4. Copy something elsewhere, come back to a field: it appears in the strip.
5. In KB Spike: turn on **Watch the clipboard**, tap **Start picture in picture**, leave the app,
   copy a few things, come back, look at **Clipboard history** and **Log**.
6. Share the log (**Log › Share log**) to the coordinator. It holds timings and counts, not text,
   except the replay test line.

## Simulator results (2026-11-20, iOS 27.0 Simulator)

- Builds clean; the keyboard extension loads, draws all rows (11 MB footprint). Keys were not tapped: there is no touch input on that Simulator.
- Apple's speech model does not run there: `SpeechTranscriber.isAvailable` is false,
  `DictationTranscriber` starts but returns no text for a recorded file. The microphone and
  speech paths are device-only.
- The first keyboard build crashed on load (a width constraint before the key was in the
  view); iOS silently showed the system keyboard instead. A crash report is the only sign.
