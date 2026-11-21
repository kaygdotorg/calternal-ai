# A Calternal keyboard on iOS: feasibility study (2026-11-20)

Study lane for Forgejo #104. No product code. Public sources only. Every fact has a link and a
label:

| Label | Meaning |
| --- | --- |
| **A** | Documented by Apple (docs, App Review Guidelines, WWDC, Apple DTS on the forums) |
| **D** | Reported by developers or vendors (their docs, issues, blog posts); not confirmed by Apple |
| **U** | Unverified: a claim I could not confirm, or my own inference |

Nothing here was run on a device. Items that only a real iPhone can settle are collected in
section 7.

## 0. Short answers

1. **Your working view is right, with two corrections.** The keyboard is a native surface of the iOS
   app, and plugins contribute to it by description. But (a) the keyboard cannot call the
   plugin runtime while you type: it can only draw descriptions the app has already cached in the
   shared container; (b) the keyboard engine itself (typing, swipe, themes, dictation hand-off)
   is not a plugin. Clipboard history, notes and chat are plugins.
2. **Dictation without leaving the app is possible, and the WKWebView method is real.** It was
   unconfirmed in my first pass. Hex for iPhone (MIT, open source) ships it: a hidden 2x2 pt
   WKWebView in the keyboard captures with `getUserMedia` and Apple SpeechTranscriber transcribes
   on device (section 9.2). It needs iOS 26, Full Access and about 24–32 MB of free memory.
   The app-session method (Raycast, Wispr Flow) is the fallback and also exists in Hex.
3. **iOS 26.4 made the "go back to the previous app" step manual**: Apple removed the host-app
   bundle ID, and Apple DTS says there is no public API for it.
4. **App Review 4.4.1 is the biggest policy risk.** It says keyboards must not launch other apps
   besides Settings, and must work without Full Access. Raycast and Wispr Flow ship anyway, but
   a developer asking Apple for a compliant method got no public answer I could find.
5. **Swipe typing is a decoder problem, not an iOS problem.** The extension draws its own keys and
   reads touches, so any decoder can run. Usable open-source decoders are on Android (FlorisBoard,
   AnySoftKeyboard, Apache-2.0). No maintained open-source iOS glide decoder was found.
   FUTO's is not usable (source-first licence; GPL inference library and a model licence).
6. **Memory is the hard constraint.** No documented limit; reports range from about 50 MB to
   about 90 MB. Models (speech, big language models) cannot live in the extension. They run in
   the app, or on the person's server.

## 1. What iOS allows a keyboard extension

| Topic | What is known | Source | Label |
| --- | --- | --- | --- |
| Where it lives | Ships inside the containing app as an app extension; cannot be installed at run time. The person enables it in Settings. | [App Extension Programming Guide: Custom Keyboard](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) | A |
| Memory limit | Apple publishes no number. Reports: ~30 MB (old), ~50 MB, 70 MB "what actually works", 90 MB hard limit on an iPad A12X (killed at 92 MB), "30–40 MB up to 120 MB+ on iPhone 15 and later" (a Reddit post). Kill is silent. Measure `phys_footprint`; dirty heap counts, memory-mapped files are nearly free. | [Dictus issue 202](https://github.com/getdictus/dictus-ios/issues/202), [dev.to: three constraints](https://dev.to/tbds_2dadf2b626f315902eae/the-three-hard-constraints-of-an-ios-keyboard-extension-46af), [Reddit](https://www.reddit.com/r/ios/comments/1ezbepy/is_it_true_that_ios_limits_the_ram_of_thirdparty/) | D (Reddit figure U) |
| Plan for | Treat ~50 MB as the budget until measured on our oldest supported phone. | Dictus `CLAUDE.md` target, quoted in the issue above | D |
| Microphone | The 2014 guide says keyboards have no microphone access. The Raycast manual repeats it today: "iOS keyboards can't use the microphone on their own". A developer reports an audio session activates but input never starts, even with Full Access. | [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html), [Raycast manual](https://manual.raycast.com/ai/dictation), [Kevin Picchi post (X; I could read only the search snippet)](https://x.com/KevinPicchi/status/2123541498885018096) | A (old), D |
| Network, shared container | Off by default. `RequestsOpenAccess = YES` (the "Allow Full Access" toggle) enables network, a shared container with the app, Location and Address Book with permission. Without it a keyboard has no container shared with the app. | [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) | A |
| Full Access in practice | Most people decline the scary warning; without it even an App Group write may silently fail. Design for "declined". | [dev.to](https://dev.to/tbds_2dadf2b626f315902eae/the-three-hard-constraints-of-an-ios-keyboard-extension-46af) | D |
| Pasteboard | Reading and writing the general pasteboard needs Full Access (reported). Since iOS 16, programmatic reads show a paste prompt unless the person uses the system Paste menu or `UIPasteControl`; iOS 16.1 added a per-app "Paste from Other Apps" setting. Whether a Full-Access keyboard also gets the prompt: not found. | [Apple forums: pasteboard writes need Full Access](https://developer.apple.com/forums/thread/715328), [UIPasteControl](https://developer.apple.com/documentation/uikit/uipastecontrol), [MacRumors](https://macrumors.com/2022/10/17/ios-16-1-paste-from-other-apps-settings) | D / A (UIPasteControl); keyboard prompt U |
| Not offered | Secure text fields (password dots) and phone-pad fields: the system swaps in its own keyboard. An app can refuse all custom keyboards (`shouldAllowExtensionPointIdentifier`). | [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) | A |
| Text context | Through `UITextDocumentProxy`: text before and after the cursor, selected text, a document identifier, insert, delete, move the cursor. The keyboard cannot select text or show inline correction near the cursor. Context length is not documented; one report says about 200 characters before the cursor. | [UITextDocumentProxy](https://developer.apple.com/documentation/uikit/uitextdocumentproxy), [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html), [Stack Overflow report (found through a mirror; not opened)](https://stackoverflow.com/questions/76112961/ios-custom-keyboard-extension-full-context-in-field) | A / D (200 chars) |
| Drawing | Only inside the keyboard's own view; no key art above its top edge. A "next keyboard" (globe) key is required. | [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) | A |
| Host app identity | Through iOS 26.3 the host app's bundle ID could be found by a private route. On 26.4 it is nil. Apple DTS: no public API. | [Apple forums 836168](https://developer.apple.com/forums/thread/836168), [forums 843118](https://developer.apple.com/forums/thread/843118), [Superwhisper note](https://superwhisper.com/docs/common-issues/ios-26-keyboard-changes) | A (DTS answer) / D |
| Launch time | No documented budget. The dev.to author parsed a word list on the first keystroke and says it was visible. Lesson: map files, build nothing at launch. | [dev.to](https://dev.to/tbds_2dadf2b626f315902eae/the-three-hard-constraints-of-an-ios-keyboard-extension-46af) | D |
| Autocorrect lexicon | Every keyboard may read the system `UILexicon` (names, shortcuts), with or without Full Access. | [Apple guide](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/CustomKeyboard.html) | A |

### App Review rules for keyboards

[App Review Guidelines 4.4.1](https://developer.apple.com/app-store/review/guidelines/) (A). Keyboards **must**:
provide keyboard input; follow Sticker rules for images or emoji; offer a way to the next keyboard;
**remain functional without full network access and without requiring Full Access**; collect user
activity only to improve the keyboard on the device. They **must not**: launch other apps besides
Settings; repurpose keyboard buttons for other behaviours. Also relevant: 2.5.18 (no ads in keyboards),
2.5.16 (extensions related to the app), 5.2.5 (no Apple emoji), and the Apple guide's rule not to
store keystroke or voice data beyond what is needed when Full Access sends data to a server.

What follows for Calternal (U, my reading): typing, swipe, themes and any cached panel must work with
Full Access off. Clipboard sync, server AI and dictation hand-off are Full Access features and must
be optional and honestly labelled.

## 2. Dictation without leaving the app

### 2.1 Known methods

| # | Method | What it needs | Reported reliability | App Review risk | Evidence |
| --- | --- | --- | --- | --- | --- |
| M1 | **Open the app once, keep an audio session alive.** The mic key opens the containing app (URL scheme or `NSExtensionContext.open`); the app records; later taps start recording by an App Group command plus a Darwin notification, with no switch. The result returns through the App Group and the keyboard inserts it. | Full Access; App Group; mic permission in the app; Audio background mode; the person returns to the host app by hand since 26.4. | Used in shipping products: Raycast keeps the session 5 minutes (configurable up to 1 hour or "Immediately") and shows a Live Activity; Wispr Flow opens the app, and returns on its own only when it can; people complain of the manual swipe back. Apps get suspended if the audio session is not truly active (developer question). | **High-ish.** 4.4.1 "must not launch other apps"; the owner of one keyboard compiled the launch path out of Release and asked Apple for a compliant method; the thread has one reply, not from Apple. Products still ship. Keeping the mic open between dictations is also a privacy complaint. | [Raycast](https://manual.raycast.com/ai/dictation), [Wispr Flow](https://docs.wisprflow.ai/articles/7453988911-set-up-the-flow-keyboard-on-iphone), [Wispr 26.4](https://docs.wisprflow.ai/articles/6269634092-adapting-to-ios-26-4), [KeyboardKit dictation (Apple-review text)](https://docs.keyboardkit.com/documentation/keyboardkit/developer-dictation/), [Apple forums 843118](https://developer.apple.com/forums/thread/843118), [forums 800500](https://developer.apple.com/forums/thread/800500) (all D) |
| M2 | **Hidden WKWebView in the extension** calls `getUserMedia({audio:true})`; WebKit captures in its own process; PCM goes to Swift by `WKScriptMessageHandler`; on-device transcription (SpeechTranscriber, iOS 26). | Full Access; iOS 26+; the containing app's microphone permission; about 24–32 MB free memory. | **Confirmed in open source** (Hex for iPhone, MIT; details in section 9.2). Hex falls back to opening the app when memory is short or iOS is older. Earlier I found only one X post; Raycast's manual describes M1, and I could not find the Raycast employee post, so whether Raycast also uses M2 is unverified. | Unknown: no review report found for Hex's path (it is an internal TestFlight app per its README). The mic indicator still shows. | [Hex source](https://github.com/anomalyco/hex/blob/dev/ios/HEXKeyboard/KeyboardLocalDictation.swift), [Hex iOS README](https://github.com/anomalyco/hex/blob/dev/ios/README.md) (D); [X post](https://x.com/KevinPicchi/status/2123541504153051540) (U) |
| M3 | **App Intents `AudioRecordingIntent` + Live Activity**, started from the keyboard, the Action Button, a Shortcut or Siri. | iOS 18+ intent that tells the system the app records and shows the recording indicator. | Works for the Action Button (Wispr Flow ships "Quick Dictation to Notes" from the Action Button, shown in the Dynamic Island). A keyboard extension cannot run an intent on its own; the intent runs in the app. Whether a keyboard tap can fire it without opening the app: not found. | Low for the intent itself (public API). Does not remove the launch question for a keyboard button. | [Apple: AudioRecordingIntent](https://developer.apple.com/documentation/appintents/audiorecordingintent) (A), [Wispr what's new](https://wisprflow.ai/whats-new) (D), [forums 843118](https://developer.apple.com/forums/thread/843118) |
| M4 | **System dictation** (the globe-adjacent mic): Apple's own; a third-party keyboard cannot trigger it, and the system hides or shows the mic by the person's setting. | None | Reliable but it is Apple's engine, not ours. | None | [Apple support](https://support.apple.com/guide/iphone/dictate-text-iph2c0651d2/ios) (A), [Stack Overflow](https://stackoverflow.com/questions/79890664/hide-dictation-microphone-button-in-custom-ios-keyboard-extension) (D) |
| M5 | **Dictate in the app, paste from the keyboard** (voice memo in Calternal, text arrives in the clipboard panel or in notes). | Nothing special. | Works; slowest. | None | (U, my reading) |

Common limits of M1: the app is what the person sees for about a second on the first dictation;
after iOS 26.4 they must swipe back by hand ([Wispr](https://docs.wisprflow.ai/articles/6269634092-adapting-to-ios-26-4), [Raycast](https://manual.raycast.com/ai/dictation), D);
the app must not be killed (a kill ends the session).

### 2.2 The WKWebView claim: verdict

**Confirmed to exist (reported by developers, in open source).** See section 9.2 for what Hex's
code does. What is still open: reliability across iOS 26.x point releases, review outcome, and the
real memory headroom on a phone (Hex's thresholds are its own estimates, measured partly in the
Simulator). These are tests for the #105 spike.

### 2.3 Engines: what is fast enough, and where it can run

| Engine | Licence | Speed and size (as reported) | Fits the extension? | Source |
| --- | --- | --- | --- | --- |
| Apple `SpeechAnalyzer` / `SpeechTranscriber` (iOS 26) | System API | Apple says the model is on device, low latency, and "does not increase your app's size or memory usage" and updates itself. | Memory looks fine (the model is outside the process, A), but a keyboard cannot hold the mic, so audio must come from the app (M1) or WebKit (M2). A shipping keyboard's author puts capture, `SpeechAnalyzer` and Foundation Models all in the containing app. | [WWDC25 277](https://developer.apple.com/videos/play/wwdc2025/277/) (A), [forums 843118](https://developer.apple.com/forums/thread/843118) (D) |
| Parakeet TDT v3 (0.6B) / Parakeet EOU (120M, streaming, English) via **FluidAudio** (Core ML, Neural Engine) | Apache-2.0 SDK; models on Hugging Face, "permissive" | ~110x real time on M4 Pro (1 minute of audio in about 0.5 s); iPhone numbers not found. Several iOS voice keyboards use it (NanoVoice, VivaDicta). | **No.** Several hundred MB of models; run in the app. | [FluidAudio](https://github.com/FluidInference/FluidAudio), [model card](https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v2-coreml) (D) |
| Whisper via **WhisperKit** | MIT | 100+ languages; Argmax lists Parakeet v2/v3 at about 476–494 MB and Whisper large v3 turbo. | **No.** Dictus runs it in the app because "the keyboard extension has a ~50 MB memory limit". | [WhisperKit](https://github.com/argmaxinc/WhisperKit), [Argmax docs](https://app.argmaxinc.com/docs/guides/managing-models), [Dictus](https://github.com/getdictus/dictus-ios) (D) |
| whisper.cpp | MIT | Common on Mac and iOS. | No (same reason). | [repo](https://github.com/ggerganov/whisper.cpp) (D) |
| Server transcription (the person's own server) | Ours | Network time; keeps audio off device only if the person trusts their server. | Yes: the extension sends audio through the app or directly (Full Access). | (U) |
| Hex (Mac) | MIT, now moved to `anomalyco/hex` | Mac reference: hold a hotkey, Parakeet TDT v3 through FluidAudio or WhisperKit, paste. It is fast because the model stays loaded and nothing crosses an extension boundary. | Not applicable on iOS. | [Hex](https://github.com/kitlangton/Hex) (D) |

Conclusion (U, my reading): all engines run **in the app** (kept alive by the dictation session), or
on the server. The extension only shows the waveform, sends start/stop, and inserts text. Speed on
iPhone is unmeasured: a lane must time Parakeet EOU and SpeechTranscriber from "last word" to
"text in field" on a real iPhone (section 7).

## 3. Swipe (glide) typing

### 3.1 How a decoder works, in plain words

1. The extension records the finger path as points over time. (It draws its own keys, so it can
   read every touch; Raycast's iOS keyboard ships swipe typing this way: [Digital Trends](https://www.digitaltrends.com/phones/raycast-primary-ios-keyboard-lets-you-dictate-swipe-type-and-use-ai-commands-without-switching-apps/), D.)
2. For each word in a vocabulary, build an "ideal" path through the key centres of its letters.
3. Compare the finger path with each ideal path (shape and place); keep a short list of close
   words. This is the classic SHARK2 template method ([Kristensson and Zhai](https://www.pokristensson.com/gesturekeyboard.html), A-literature; summary of SHARK2 as a baseline in [arXiv 2410.18099](https://arxiv.org/html/2410.18099v1)).
4. Rerank the list with a language model: how likely is this word after the previous words.
5. Newer systems replace step 3 with a small neural net that scores letters directly. Google
   describes a spatial model plus a finite-state lexicon and an n-gram, searched with a beam, and
   trains the spatial model from reverted corrections and chosen suggestions ([Google Research](https://research.google/blog/the-machine-intelligence-behind-gboard/), reported by Google).

### 3.2 Open-source decoders and licences

| Project | Licence | Swipe | Usable in Calternal (AGPL-3.0) and the App Store? | Source |
| --- | --- | --- | --- | --- |
| FlorisBoard (Android, Kotlin) | Apache-2.0 | Glide typing (`StatisticalGlideTypingClassifier`) | Code can be ported (Apache-2.0 is compatible with AGPL-3.0 in that direction; keep notices). Statistical, not neural. | [repo](https://github.com/florisboard/florisboard), [commit](https://git.tjdev.de/mirror/florisboard/commit/535b48e5b413bb69378981552d3f1ef336a28a5e) (D) |
| AnySoftKeyboard (Android) | Apache-2.0 | Gesture typing (not read in detail) | Same. | [repo](https://github.com/AnySoftKeyboard/AnySoftKeyboard) (D) |
| FUTO Keyboard / FUTO Swipe | Keyboard: FUTO Source First 1.1 (modify and distribute only non-commercially; payment code must stay). Swipe: separate model licence; C++ inference library GPL. | Neural encoder (~635K parameters) plus optional context LM | **No** for the keyboard (not open source; not AGPL-compatible). The swipe models' licence and the GPL library need a legal reading before any use; the study found no personalisation in them. Use as a design reference only. | [LICENSE](https://github.com/futo-org/android-keyboard/blob/master/LICENSE.md), [swipe.futo.tech](https://swipe.futo.tech/), [HF card](https://huggingface.co/futo-org/swipe) (D) |
| KeyboardKit (iOS) | Closed source, commercial licence; Pro unlocks autocomplete and themes | The study did not find swipe | **No.** Not open source; licence keys. | [LICENSE](https://github.com/KeyboardKit/KeyboardKit/blob/master/LICENSE) (D) |
| Fleksy VirtualKeyboardSDK | Not checked | Swiping | Unknown; check before use. | [repo](https://github.com/FleksySDK/VirtualKeyboardSDK) (U) |
| iSwipe, JustType, Slidden | Old (iOS 6–8) | Hook system keyboard, or gestures not glide | No: obsolete. | [iSwipe](https://github.com/wyndwarrior/iSwipe) (D) |
| Dictus (iOS), VivaDicta | MIT | Dictation keyboards, not swipe | Reference for the app-extension split. | [Dictus](https://github.com/getdictus/dictus-ios) (D) |

No maintained open-source iOS glide decoder was found. A realistic path: port FlorisBoard's
classifier to Rust (our shared client core), then improve it.

**AGPL and the App Store (U, needs the owner or a lawyer).** App Store terms and the GPL family
have a known conflict (see [FSF on App Store GPL enforcement](https://www.fsf.org/blogs/licensing/more-about-the-app-store-gpl-enforcement)).
The owner holds the copyright of his own code and could offer it under App Store terms; code
from other authors (even Apache-2.0 or MIT) is fine, but copyleft code from others (GPL) would be
a problem for the App Store build. This is a decision for the owner, not for this lane.

### 3.3 What "self-learning" needs

| Need | What it is | Where it can live |
| --- | --- | --- |
| Personal vocabulary | Words, names, abbreviations the person uses, with counts. Drives the language model rerank. | Extension reads a compact file (mapped); the app updates it. |
| Next-word model | An n-gram or small model learned from what the person writes. FUTO trains its transformer while the phone is idle, and says nothing leaves the device ([FUTO docs](https://docs.keyboard.futo.org/print), D). | Train in the app (idle, charging) or on the person's server; ship a small mapped file to the extension. |
| Gesture adaptation | Per-person shape of the finger path: speed, size, typical key offsets. Gboard personalises its spatial model ([Google Research](https://research.google/pubs/spatial-model-personalization-in-gboard/), D). Training signal: a suggestion the person picked, or a correction they made. | Collect pairs (path, final word) on the device; fit a few parameters (key offsets, scale) in the app. A neural model would need real training; start with offsets. |
| Fresh text | Without reading what is typed, the keyboard sees only the field's context (`UITextDocumentProxy`). | Learn from words typed through the keyboard (counts only), plus the sources below. |

**What Calternal can add (the differentiator).** The person's own notes and chats are a large,
private text corpus already on their server. A server-side job (a plugin) can build, per person:
names (people, projects), terms and phrases, and a bigram table; ship them as a small signed file
to the app; the app installs the mapped file for the extension. Notes the person marks private
are left out. Nothing needs the keyboard to read other apps' text. Costs: the file must be small
(budget: a few MB mapped, not loaded), and Full Access is not needed because the app, not the
extension, syncs it (the shared container needs Full Access to be reliable — see section 1; this
is a real constraint: without Full Access the extension may not read the app's file at all (D). U
until a device test).

## 4. Fit with the architecture

### 4.1 Split of work

| Part | Where | Why |
| --- | --- | --- |
| Key layout, touch handling, haptics, theme drawing, panels drawn from descriptions | **Native Swift/SwiftUI in the extension** | Only UIKit view code can draw in the keyboard; this matches the rule "Swift native UI through UniFFI". |
| Swipe decoder, language model lookup, personal vocabulary | **Shared Rust core, linked into the extension** | Pure compute on mapped files; no network. Rust has no garbage collector; memory is what we map. U until measured: each linked Rust library adds code size; mapped data costs little ([dev.to](https://dev.to/tbds_2dadf2b626f315902eae/the-three-hard-constraints-of-an-ios-keyboard-extension-46af), D). |
| Sync, plugin runtime calls, search, vocabulary build, clipboard upload | **App (and the server)** | Network and heavy work. The extension would need Full Access and tight memory. |
| Description renderer for panels | **Shared Rust core + SwiftUI** | Same renderer as the app; the contract already says each platform draws descriptions natively ([`AGENTS.md`](../../AGENTS.md)). |
| Dictation: capture, transcribe, clean-up | **App** (session) or server | Section 2. |
| Result insertion | **Extension** (`textDocumentProxy.insertText`) | Only the keyboard can write to the field. |

### 4.2 What a "keyboard" surface looks like in the contract

Contract status (read from `origin/lane/v2-host-base`, revision 14.44; not changed by this lane):

* An app may publish **typed, versioned extension points**; other plugins contribute panels, row
  fields, actions and editors through the same manifest mechanism; the host validates the
  resulting native description and rechecks the data scope ([plugin-api-v2 §1](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/v2-host-base/docs/plugin-api-v2.md)).
* Chrome today has `sidebar`, `header`, `footer`, `panel` ([description-contract §4](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/v2-host-base/docs/description-contract.md)).
  No keyboard region exists.
* The description has no free layout: spacing, type and motion come from the renderer's tokens
  (§4, "No free layout"). Good for themes: a keyboard theme is a set of tokens.
* Device cache (§6): data-free templates persist by account; row records persist only for
  opted-in, signed, start-anchored windows. A keyboard panel that must paint at once needs
  this cache; "paint from the inventory left by the last reconciliation" is already the rule.

**Proposal (for the contract owner; I did not edit the contract).** A `calternal-keyboard` app (the
iOS/macOS/Android keyboard shell) publishes these points, and clipboard, notes and chat contribute:

| Point | What a plugin contributes | Notes |
| --- | --- | --- |
| `keyboard.panel` | A tray panel: title, icon, and a content-node tree (list of rows from a Bind, search field, buttons). | The renderer draws it with the keyboard's tokens. Bind windows come from the device cache; a live fetch needs the app (Full Access). |
| `keyboard.action` | A key-like action. Outcome type `insert_text` (literal text from a checked row field), `replace_selection`, or `open_app` (see 4.4). | Text comes from a field on a row the person can read; the host checks it, as for any action. No plugin code runs per keystroke. |
| `keyboard.suggest` | A source of word candidates (names from notes). | Batch built on the server, shipped as a vocabulary file (section 3.3), not a live call. |
| `keyboard.theme` | A token set: colours, key shape, font, haptics, sound. | Themes are descriptions of tokens, shared with the app's theme. |

Rules that follow from the principle "server simple, first-party no special treatment": the
keyboard app uses the same manifest mechanism as any other app; clipboard (`calternal-clipboard`),
notes and chat are ordinary plugins that declare a contribution; a third-party plugin could add a
panel the same way, and may be disabled without breaking typing.

**Gap for issue #78 (to post):** "No keyboard surface and no `insert_text` outcome exist in the
description contract; a keyboard needs panels that render from cached windows with Full Access off,
and an outcome that writes to another app's text field." I will post this as one comment.

### 4.3 Clipboard as its own plugin

* Items: each copied text is an item in the person's space (a kind `clips`), synced to their
  server like any item; retention bounded (review gate 7).
* Capture: iOS lets an app or keyboard read the pasteboard only in the foreground, with the paste
  prompt (section 1). So history records only what the person copies while the keyboard is
  showing or the app is open, or when they press a "save clipboard" control (`UIPasteControl`
  avoids the prompt). **No background capture** is the expected limit (U; check on a device).
  That is weaker than macOS or Android clipboard managers, and must be said in the product.
* Sensitive text: password managers mark pasteboard items; an app should honour concealed types
  (U, not researched).

### 4.4 The same surface on macOS and Android (so the plugin side is not iOS-only)

| Platform | Equivalent | What to know | Source |
| --- | --- | --- | --- |
| Android | Input method service (IME) | An IME is a normal installed app; runs in its own process; can use mic, network and storage with ordinary permissions (no Full Access toggle). Dictation, swipe and the clipboard are easier. Existing open-source decoders are here. | [Android: creating an input method](https://developer.android.com/develop/ui/views/touch-and-input/creating-input-method) (A) |
| macOS | Input Method Kit (an input method app, not a keyboard layout) | A macOS input method can offer candidates and insert text; Calternal's Mac app already handles global hotkeys for dictation (the Hex pattern), so on macOS the surface is a panel window plus paste. | [Input Method Kit](https://developer.apple.com/documentation/inputmethodkit) (A), [Hex](https://github.com/kitlangton/Hex) (D) |
| iOS | Keyboard extension | This study. | |

The plugin contract stays the same: panels, actions with `insert_text`, a vocabulary source, and
a token theme. Each platform's shell decides how to show them (a tray on iOS, a popup on macOS, an
IME candidate or toolbar view on Android). On Android and macOS the "cached window only" limit
disappears, so a platform may offer richer panels.

## 5. Staged plan (riskiest unknowns first)

| Stage | Goal | What is risky | Exit test |
| --- | --- | --- | --- |
| 0 | **Hidden-WebView capture test (M2)** and **app-session test (M1)** in a bare keyboard with no UI. | Is M2 real? Does the session survive a suspended app on iOS 26.x? | On a real iPhone: tap mic in Notes, speech reaches the extension; record time to text. Decide M1 or M2. |
| 0b | **Memory and launch**: empty extension plus Rust core plus a mapped 20 MB file. | The real memory limit on our oldest phone; launch time. | Footprint by `phys_footprint`; no jetsam in 30 minutes of use. |
| 1 | **Typing**: layout, haptics, theme tokens, globe key, Full-Access-off mode. | App Review 4.4.1 conformance. | Works with Full Access off, in fields, and swaps out in a password field. |
| 2 | **Swipe**: port FlorisBoard's statistical decoder to Rust; compare with Apple's on a fixed phrase set. | Accuracy; latency in the extension. | Words per minute and error rate vs Apple's, on a real phone, with 10 testers. |
| 3 | **Panels from descriptions**: clipboard, notes, chat from the cache; `insert_text`. | Cache paints with Full Access off; shared container reliability. | Panel opens within one frame budget; an insert lands in Notes, Messages, Safari. |
| 4 | **Dictation product**: engine in the app (Parakeet or SpeechTranscriber), session, Live Activity, hand-back. | 26.4 manual return; App Review. | 20 dictations in a row without switching; TestFlight review feedback. |
| 5 | **Learning**: personal vocabulary and bigrams from the person's notes and chats; gesture offsets. | Privacy of the vocabulary file; size. | Measured gain on the person's own phrases. |
| 6 | **Contract points** (`keyboard.*`) and third-party panels, once the contract owner agrees; macOS and Android shells. | Contract design. | A community panel renders with no app change. |

## 6. What can only be checked on a real iPhone

1. Whether `getUserMedia` in a hidden WKWebView inside the extension gets mic input (M2), with and without Full Access, and whether iOS shows the mic indicator.
2. How long a background audio session survives after the person returns to the host app; does iOS 26.x end it; does the Live Activity change that.
3. The actual extension memory limit by device (oldest supported iPhone, newest), and the effect of a mapped 20 MB file.
4. Latency from last spoken word to text in the field, for each engine.
5. Whether a Full-Access keyboard triggers the iOS 16 paste prompt, and `UIPasteControl` inside a keyboard.
6. Whether the App Group is readable without Full Access.
7. Swipe feel: latency under load, touch coalescing, haptic timing.
8. Return-to-app behaviour on iOS 26.4 and later, if any automatic route appears.
9. Secure-field and phone-pad swap behaviour, and apps that refuse custom keyboards.
10. App Review: only a TestFlight external review and then submission shows how 4.4.1 is applied to the launch step.

## 7. Questions for the owner

Each stands on its own.

**Q1. May the keyboard open the Calternal app to start dictation, given App Review rule 4.4.1?**
*What it is:* Apple's rule says keyboards must not launch other apps besides Settings. All
known fast dictation keyboards (Raycast, Wispr Flow) still open their own app for the first
dictation, then keep a session alive for a few minutes. *Why now:* it decides the whole
dictation design and whether the first release can be refused. *Example:* the person taps the
mic in Messages; the Calternal app opens for a second; they swipe back; the next four
dictations in 5 minutes do not switch. *Options:* (A) use this method and accept the review risk
(costs: possible rejection; a Live Activity and a mic session that stays open; a manual
swipe back on iOS 26.4+); (B) no switch at all: dictate in the app and paste (costs: slow, not
"super fast"); (C) use the hidden-WebView capture, which Hex ships, so no switch at all on iOS 26 (costs: needs Full
Access, 24–32 MB free memory, an unknown review result), with A as the fallback when memory is short.
*Recommendation:* C with A as fallback, as Hex does; confirm both in the #105 spike.

**Q2. How private must dictation and learning be: only on the phone, or may the person's own
server do it?**
*What it is:* on-device engines are fast and private but live in the app (not the extension);
the person's server can run bigger models and learn from notes and chats. *Why now:* it sets
Full Access requirements and what we build first. *Example:* "Please meet Priya at the Kochi
office" is transcribed on the phone, but the name "Priya" came from the person's notes via the
server. *Options:* (A) device only (costs: smaller vocabulary, no server AI clean-up); (B) device
by default, server optional (costs: two paths to test, a setting); (C) server by default (costs:
network delay; needs Full Access). *Recommendation:* B: it gives speed by default and lets
notes and chats improve it when the person turns it on.

**Q3. Should the keyboard work fully without "Allow Full Access", and what do we do when it is off?**
*What it is:* Full Access shows a scary warning. App Review 4.4.1 requires the keyboard to work
without it. Without it we lose network, pasteboard, and maybe the shared container. *Why now:*
it decides whether panels and the vocabulary can reach the extension at all. *Example:*
Full Access off: typing, swipe and themes work; clipboard and chat panels show "Turn on Full
Access" (as the dev.to author does). *Options:* (A) Full Access required for all Calternal
features, plain typing without it (costs: most people will see an empty keyboard); (B) design the
extension to read only a file the app writes, and test whether the shared container is readable
without it (costs: needs a device test; if not readable, fall back to A). *Recommendation:* B,
with A as the fallback.

**Q4. Which swipe decoder do we build on?**
*What it is:* the part that turns a finger path into a word. *Why now:* it is the biggest
engineering cost, and it decides licence and language support. *Example:* the person swipes
"hello"; the decoder must not return "jello". *Options:* (A) port FlorisBoard's
Apache-2.0 statistical decoder to Rust (costs: weeks; accuracy near FlorisBoard's, below
Gboard's); (B) train a small neural decoder (costs: a training set and months; best ceiling,
FUTO and Gboard show it works); (C) license a commercial SDK such as Fleksy (costs: money; its licence was not checked and closed source may not fit AGPL-3.0). *Recommendation:* A first, with the vocabulary and gesture
learning hooks, then B as a later replacement.

**Q5. AGPL-3.0 and the App Store: what is the plan for the iOS build?**
*What it is:* the product is AGPL-3.0; the App Store terms conflict with the GPL family (the FSF
wrote about it; I did not read the page in full). *Why now:* it limits which third-party code the keyboard (and the
iOS app) may link. *Example:* porting Apache-2.0 FlorisBoard code is fine; linking a GPL swipe
library such as FUTO's C++ code is not. *Options:* (A) keep AGPL; ship only code that is
permissive-or-owned in the iOS build (costs: care in choosing dependencies); (B) offer the iOS
app under a separate licence you grant yourself (costs: contributors' code needs their
permission, a CLA); (C) distribute outside the App Store (alternative marketplaces exist only in some regions; not researched). *Recommendation:* A, and say so in the dependency review.

**Q6. Where should clipboard history be captured?**
*What it is:* iOS gives no background pasteboard access, so history only records what is
copied while Calternal is on screen or the person taps "save". *Why now:* it decides if
"clipboard synced to the server" can match the Mac. *Example:* the person copies in Safari,
switches to Messages with the Calternal keyboard: history will not have the copy unless
the keyboard reads it at that moment (with a possible paste prompt). *Options:* (A) the keyboard
reads the pasteboard when it appears, with Full Access (costs: the paste prompt, privacy
worry); (B) a "paste from clipboard" button using `UIPasteControl` (costs: one tap; no prompt);
(C) iOS clipboard history only from in-app copy and the share sheet (costs: little). 
*Recommendation:* B plus A as an opt-in, and state the limit in the app. A fourth option exists (see 9.1): a PiP-based background monitor like PastePal and Yoink (costs: a visible PiP window, review risk under 2.5.4); offer it only as an opt-in in the app, not in the keyboard.

**Q7. A "keyboard" extension point in the contract: who writes it and when?**
*What it is:* the contract has no keyboard surface or insert-text outcome (section 4.2); I
would post a short comment on #78. *Why now:* the plugin API is being sealed; a point added
later may need a new contract revision. *Example:* the clipboard plugin contributes a panel and
a "paste item" action that inserts text. *Options:* (A) add `keyboard.*` points now as
provisional (costs: one more review round); (B) delay until the keyboard stage 3 (costs: a later
revision). *Recommendation:* B, since the stage 0 tests may change what panels can show, and
the existing extension mechanism already allows an app to publish points.

## 8. Pending decisions after the addendum

Q6 (clipboard) changes: see 9.1. A new question follows at the end of section 9.

## 9. Addendum (2026-11-20, evening): owner's follow-up questions

### 9.1 Background clipboard watching (PastePal, Yoink)

| Fact | Source | Label |
| --- | --- | --- |
| Since iOS 14 an app cannot listen to the pasteboard in the background; PastePal's own docs say so. | [PastePal docs](https://docs.indiegoodies.com/pastepal/iOS/background-monitoring) | D |
| PastePal's "Background Monitor" shows a small Picture-in-Picture player to keep the app running, so it can read the clipboard. Other routes it lists: Slide Over on iPad, and the keyboard extension with Full Access. It is off by default and the person turns it on. | same | D |
| The technique was introduced by Yoink (iOS 15): Picture in Picture driven by `AVSampleBufferDisplayLayer` and `AVPictureInPictureController.ContentSource` shows a static banner ("monitoring clipboard"); PiP keeps the app alive, so it reads the pasteboard on copy. It runs for hours or days. The person starts it by hand (three taps), can pause it with the PiP play button, and can close the window. | [MacStories on Yoink](https://www.macstories.net/ios/yoink-brings-background-clipboard-monitoring-to-ios-and-ipados-15-via-picture-in-picture-workaround/) | D |
| What iOS shows: the PiP window on screen (it can be shrunk and tucked in a corner), plus the system's paste alert each time the app reads the pasteboard. PastePal tells people to set Settings > PastePal > Paste from Other Apps > Allow to avoid the prompt (the option may appear only after the prompt has been seen once). | MacStories, [PastePal: Allow Paste](https://docs.indiegoodies.com/pastepal/iOS/allow-paste-popup) | D |
| Review risk: older tricks were stopped. Tapbots' Pastebot kept alive by silent audio, and Apple shut it down. The Yoink author's PiP method was approved, Apple later featured the app, and MacStories judged it "not technically breaking any App Store rule" (a writer's opinion, not Apple's). Relevant rule: 2.5.4 lets multitasking apps use background services only for their intended purposes. A PiP that shows no video is a use against the intent, so the risk is real but, going by Yoink and PastePal, tolerated. | MacStories; [App Review Guidelines 2.5.4](https://developer.apple.com/app-store/review/guidelines/) | D / A (rule text) |

For Calternal (U): background capture is possible only as an opt-in "Background clipboard"
mode with a visible PiP banner and a PiP pause button, and with the iOS paste prompt unless the
person allows "Paste from Other Apps". Keep it out of the keyboard extension (the keyboard's
review rules are stricter than the app's) and say plainly what it does. Capture while the
keyboard is on screen, or a paste button, works without it.

### 9.2 What Hex for iPhone does (dictation and its keyboard)

The owner's pointer led to the new repository `anomalyco/hex` (MIT; the old Swift Mac repo moved).
It now has an iOS app, `ios/`: `HEXMobile` (app), `HEXKeyboard` (keyboard extension), `HEXWidgets`
(Live Activity and Control Center), `HEXShared`. Read at commit `178c16fd`, 2026-11-20. Everything
below is reported by the source, not run by me (label D).

| Topic | What it does | Where |
| --- | --- | --- |
| Capture inside the keyboard | Hidden WKWebView (2x2 pt, alpha 0.02, no interaction); `getUserMedia` with mono, echo cancellation, noise suppression and auto gain all off; `ScriptProcessor` (4096 frames) turns audio into 16-bit PCM, base64, sent by `WKScriptMessageHandler`; Swift writes a temporary WAV and deletes it after use. WKUIDelegate grants microphone capture only. Source comment: the extension itself "doesn't have entitlements to record audio" (audiomxd). | [KeyboardLocalDictation.swift](https://github.com/anomalyco/hex/blob/dev/ios/HEXKeyboard/KeyboardLocalDictation.swift) |
| Audio session | `.playAndRecord`, mode `.measurement`, options mix-with-others, A2DP, default-to-speaker; prefers the built-in microphone so car or headset music keeps playing; deactivates with "notify others" afterwards. | same |
| Memory | `os_proc_available_memory()` before starting: 32 MB headroom for the first dictation, 24 MB later; else it opens the app. Capture costs 1–5 MB (dictation bar) up to 8 MB (full keyboard) in the Simulator. README: limit "about 60–90 MB depending on the model". | same, [README](https://github.com/anomalyco/hex/blob/dev/ios/README.md) |
| Model | Apple `SpeechTranscriber` (preset `.transcription`) on iOS 26+; `DictationTranscriber` (preset `.longDictation`, punctuated, for runs up to a minute) for languages `SpeechTranscriber` lacks (Norwegian, Danish, Swedish, Hebrew); speech assets downloaded on first use with `AssetInventory`, reserving locales (the system caps reserved locales, so Hex releases the oldest). Also FluidAudio Parakeet V2 (iOS 18+, English) in the app. Audio is transcribed from the finished file (`analyzer.start(inputAudioFile:finishAfterFile:)`), not streamed. | [AppleSpeechModule.swift](https://github.com/anomalyco/hex/blob/dev/ios/HEXShared/AppleSpeechModule.swift), [AppleSpeechTranscriber.swift](https://github.com/anomalyco/hex/blob/dev/ios/HEXMobile/AppleSpeechTranscriber.swift) |
| Vocabulary | The person's word list goes to the analyzer as `AnalysisContext.contextualStrings[.general]`. Hex also respells likely misheard words to the vocabulary's spelling after the fact (`VocabularySpelling`, windows of up to 4 words, whole-word, never changes text outside the match). | AppleSpeechTranscriber.swift, VocabularySpelling.swift |
| Post-processing (4 styles) | Polished (sentence case, punctuation, filler trim), Casual (lowercase, no final period), Notes (spoken "bullet" and "next line"), Verbatim. Cursor-aware spacing when dictating mid-sentence. Fillers and spoken corrections ("3, I mean, 4") are removed by language rules (Spanish, German, Norwegian) or, as an off-by-default experiment, by a small on-device BERT tagger (4 layers, int8 Core ML, 128 tokens, keeps or deletes each word, delete threshold 0.98). A second experiment runs one on-device Apple Intelligence pass to fix misheard words. | TranscriptFormatter.swift, SpokenCleanupRules.swift, CleanupTagger.swift, AppleIntelligenceCleanupSettings.swift |
| Learning from your edits | A "typing journal" stores word-level dictation fixes: what the model heard, what was inserted, what you left, with counts, contradictions and one word of context; it never stores the dictation itself. Learned fixes are applied after transcription. | TypingJournalLearning.swift, DictationCorrections.swift |
| Warm session | "Keep mic warm" Off/5m/15m/1h: the app starts an `AVAudioEngine` session; the keyboard sends commands and gets text over an App Group with Darwin notifications; a Live Activity shows it. Opening the app from the keyboard uses the `UIApplication` at the end of the responder chain, because `Link`, `openURL` and `extensionContext.open` do nothing in a keyboard. Before iOS 26: the app opens, with a 10 s handoff and a "swipe bar to return" cue. | README |
| The keyboard itself | A full keyboard, not just a mic: GPU-drawn waveform ("dot scope") only while recording (no Metal objects built at launch), autocorrect with a 3-slot suggestion bar on `UITextChecker` (so no custom language model), clipboard strip, accent picker, trails of typed text in a local typing log. I found no swipe typing in it. | `ios/HEXShared/FullKeyboard/` |

Why Hex feels better than built-in dictation (U, my reading of the above): it uses the newer
SpeechTranscriber model, a vocabulary hint, a long-form preset, a file-based pass with finalisation,
style post-processing, and learned corrections. Those are all copyable: the engine part is public
Apple API, the rest is text code that could live in the shared Rust core. Because Hex is MIT, its
ideas and code may be reused with its notice (AGPL-3.0 compatible, as with Apache-2.0).
The Mac app (Parakeet via FluidAudio, optional Whisper) is a different path; "Apple speech" is
the iOS path.

### 9.3 Live voice conversation ("gpt-live") from a keyboard

What the owner described: a spoken conversation with a model while brainstorming, and at the end
the model types the result into the field. Raycast's keyboard has typed Ask AI, Quick Fix and
Translate (Pro): the question is typed, the answer is inserted or replaces text; no voice
conversation ([Raycast keyboard manual](https://manual.raycast.com/ios/keyboard), D).

What is needed and what is known:

| Need | Known | Label |
| --- | --- | --- |
| Audio in | Same as dictation: the extension cannot record; WebKit can (Hex). Echo cancellation: Hex turns it off for dictation; a conversation with speaker output needs it on (the `getUserMedia` flag exists; effect inside WKWebView not tested). | D / U |
| Audio out | Hex sets a `.playAndRecord` session with default-to-speaker inside the extension, so playback in the extension's process is plausible. Playback through the hidden WebView is also plausible. Not tested. | D / U |
| Speech-to-speech transport | OpenAI recommends WebRTC for browser and mobile clients (the backend mints an ephemeral key; model audio returns as a remote media stream) and WebSocket for server-to-server; names of the models change (`gpt-realtime-2.1`, and "GPT-Live" for full-duplex), so confirm on live docs. Function calling exists in the Realtime API, which is how "type the result now" can be a tool call. | [OpenAI Realtime guides](https://developers.openai.com/api/docs/guides/realtime-conversations), [WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc), [function calling](https://developers.openai.com/api/docs/guides/realtime-function-calling) (D, vendor docs) |
| Where the session can live | See below. | |

Three places for the live session (U, my reasoning; all need a device test):

| Place | How | Good | Bad |
| --- | --- | --- | --- |
| A. In the keyboard's hidden WebView | A WebRTC peer connection runs inside the same hidden WKWebView that captures audio; Swift only inserts the final text from the model's tool call. WebKit's process does audio and network. | No app switch; simple; mic and speaker in one place. Whether WebKit's process counts against the keyboard's memory is not known. | Ends when iOS closes the keyboard (hide keyboard, switch app); needs Full Access; long sessions untested; Hex's dictation is bounded at about a minute. |
| B. In the containing app, kept alive like the warm session | The app holds the audio session and the realtime connection; the keyboard shows state and inserts text over the App Group. | Survives keyboard hide; heavier models and reconnect logic live in the app. | The app must be started once and kept alive; background audio rules (App Review 2.5.4: audio is an allowed background use); a Live Activity needed; manual swipe back on 26.4+. |
| C. Server-side | The phone only streams audio to the person's Calternal server (one WebSocket); the server runs the voice model and a chat plugin; it returns text only at the end. | Server stays generic and the model choice stays with the person; same pattern as chat. | Audio crosses the network twice unless the model is local; needs Full Access; latency. |

The principle "app logic lives in plugins; the server stays simple" points to C for the logic
(a `calternal-chat` harness or a voice plugin that owns the session and the "finish" tool) and A or
B only as the audio carrier on the phone. The keyboard asks for an `insert_text` outcome when the
session ends: the same outcome proposed in 4.2.

### 9.4 Swipe decoder: reason confirmed

The owner confirms the main reason for building our own decoder: learning from the person's own
notes and chats. Consequences for the plan (U):
* An off-the-shelf decoder cannot take a per-person vocabulary built on the person's server, so
  the vocabulary file and its ranking must be ours (3.3); the geometry part may start from
  FlorisBoard's Apache-2.0 code.
* Hex shows the general shape of the learning loop: store word-level fixes with counts and
  contradictions, never the text itself (9.2). The same structure fits swipe: store (path summary,
  chosen word) counts, never the raw trace.
* The vocabulary build is a server job over notes and chats, run by a plugin, with private notes
  excluded.

### 9.5 Facts for the #105 spike (also posted on #104)

Riskiest first: (1) microphone capture: copy Hex's hidden-WebView recipe (9.2) and log
`os_proc_available_memory()` before and after; (2) memory limit: log the same value at launch and
after loading a mapped 20 MB file; (3) Full Access required for capture and for any App Group
exchange; (4) warm session and opening the app from the extension through the responder chain.

### 9.6 New question for the owner

**Q8. Which live voice design do we test first: audio in the keyboard, in the app, or on the server?**
*What it is:* a spoken brainstorm with a model, ending with the model typing the result. The
session has to live somewhere: the hidden WebView in the keyboard (A), the app kept alive (B),
or the person's server (C) with the phone only carrying audio. *Why now:* it fixes what the
spike must measure and what the contract needs (an `insert_text` outcome at the end of the
session). *Example:* you talk for five minutes about a plan while the keyboard is open in Notes;
the model asks questions aloud; when you say "write it up" the finished text appears at the
cursor. *Options:* (A) costs: ends when the keyboard hides, memory unknown; (B) costs: an app that
must stay alive and a Live Activity; (C) costs: network delay, Full Access, and a server voice
plugin to build. *Recommendation:* C for logic with A as the carrier, because it keeps the
server generic and avoids a background app; test A's carrier on the iPhone in the spike first.

## 10. Simplification opportunities

* One extension point family (`keyboard.*`) for iOS, macOS and Android avoids three plugin
  interfaces; keep the description contract free of platform names.
* The vocabulary file serves swipe, autocorrect and dictation vocabulary (and the server's
  transcription clean-up); build it once, per person, in one plugin.
* The dictation session, Live Activity and app hand-off could be one reusable module also used
  by the existing "voice note" input, not a keyboard-only feature.
* The theme token set could be the app's own token set with no keyboard-specific layer.
