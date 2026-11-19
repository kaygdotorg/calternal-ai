# iPhone shell

The iPhone app is a thin native shell around the web app: one full-screen `WKWebView` opening
`https://calternal.cloud/`. Plugins and agents change interfaces in the web client; the shell adds
only what a web view cannot do. Source: `apps/ios` (Swift, UIKit, iOS 26 or later, no dependencies).

## Layout of `apps/ios`

| Path | Purpose |
| --- | --- |
| `Calternal/*.swift` | The shell: `ShellViewController` (web view, colours, restore), `NativeBridge` (the page's door to native), `NativeBar` (glass bar experiment), `MenuPresenter`, `ShellConfig` (start address, switches), `Probe` (evidence tool, off by default). |
| `Calternal/Resources/bridge.js` | Injected into every page: defines `window.calternalNative`. |
| `Calternal/Resources/bridge-demo.html` | Test page for the bridge and for painting under the keyboard (start address `demo`). |
| `Calternal/Settings.bundle` | iOS Settings > Calternal: start address and the glass bar switch. |
| `Info.plist`, `Calternal/Calternal.entitlements` | URL scheme `calternal://`, launch colour, passkey domain (`webcredentials:calternal.cloud`). |
| `Shell.xcconfig`, `Local.xcconfig.example` | Local signing values live in the untracked `Local.xcconfig` (team, phone UDID). Never commit it. |
| `sim.sh`, `cycle.sh` | Build and run in the lane's Simulator (on the Mac Studio). |
| `install-on-phone.sh`, `share-build.sh`, `common.sh` | Signed builds without the login keychain; install on the phone, or upload to the share page. |
| `sync-to-mac.sh` | Copies the tracked files to `kayg@std:~/Developer/Calternal-AI/apps/ios`, a plain rsync target (not a git checkout) that keeps its own untracked files. |

## Start address

First match wins: launch argument `-CalternalStartURL <url>`, environment `CALTERNAL_START_URL`
(Xcode scheme setting), Settings > Calternal > Start address, then production. `demo` opens the
bundled bridge page (`demo#resizes-content` opens it with the other keyboard mode).

```sh
xcrun simctl launch <device> cloud.calternal.app -CalternalStartURL http://10.69.69.222:4310/
./sim.sh run https://preview.calternal.cloud/ai/        # on the Mac Studio, in apps/ios
```

`calternal://` opens the app; `calternal://threads/abc` opens `<start origin>/threads/abc`.

## The bridge (`window.calternalNative`)

Present only inside the shell, so feature-detect: `if (window.calternalNative) …`. Every call returns
a Promise. Bridge version 1.

| Call | What it does | A web view alone cannot |
| --- | --- | --- |
| `haptic(style)` | `selection`, `light`, `medium`, `heavy`, `soft`, `rigid`, `success`, `warning`, `error` | vibrate or tap at all (iOS has no `navigator.vibrate`) |
| `appearance({scheme, background})` | `scheme` `light`/`dark` sets the status bar text and the keyboard look; `background` (`#rrggbb`) is the colour behind the page. Call on every theme change. Without it the shell uses the colour WebKit reports. | set the keyboard's look separately from the system's |
| `ready()` | First real frame painted: the shell fades its launch cover. Fallback: 0.6 s after load. | |
| `share({text, url, anchor})` | System share sheet; resolves `{completed}` | open the full share sheet from script outside a user gesture on every page (`navigator.share` is limited) |
| `menu({anchor, title, items})` | Native menu next to `anchor` (`{x,y,width,height}`, page pixels); items `{id, title, symbol?, destructive?, disabled?}` (SF Symbol names); resolves `{id}` or `{id: null}` | show a real system menu |
| `saveState(obj)` | At most 64 KB of unsaved page state kept in memory; after iOS ends the page's process the shell reloads the page where it was and fires `calternal:restore` with `detail` = the object | |
| `bar({items, selected})` / `bar(null)` | Experimental native glass bar; resolves `{native}` (false when the switch is off: draw your own). Taps arrive as `calternal:bar` `{id}`; `--native-bar-inset` is set on `:root` while shown. | draw real Liquid Glass |

Events from the shell to the page: `calternal:restore`, `calternal:bar`.

## What the shell does so the page can look native

* The web view is full height under the status bar, the home indicator and the keyboard, with
  nothing opaque of its own: what shows under the keyboard's glass is what the page paints there.
* `env(safe-area-inset-*)` is correct (62 / 0 / 34 / 0 points on a 440-point-wide iPhone).
* iOS 26's scroll edge effect is off (it faded the page into the shell's colour at the top: the
  "top band").
* The back swipe belongs to the page (the web view's own gesture is off).
* No rubber-band on the page's root scroller (`bounces = false`); inner scrollers keep their
  momentum. Use `overscroll-behavior: none` on inner scrollers that must not bounce.
* The keyboard follows the page's scheme through `appearance()`, not the device's.

## What the page must do (found with the Simulator and the probe)

1. **Paint under the keyboard.** The page declares `interactive-widget=resizes-content`
   (`apps/web/index.html`). In the shell that shrinks the layout viewport to the area above the
   keyboard (`innerHeight` 609 with a 347-point keyboard on a 956-point screen), so the fixed
   wallpaper (`.backdrop { position: fixed; inset: 0 }`) stops at the keyboard and the glass shows
   flat colour. Use `interactive-widget=resizes-visual`: the layout viewport and the fixed
   wallpaper stay full height under the keyboard, and the page sizes only its chrome (the
   composer, the scroller's bottom padding) by `visualViewport`. The bundled demo shows both modes.
2. **Top fade:** draw it in the page, over the page's own wallpaper (the shell no longer adds one).
3. Call `calternalNative.appearance()` when the theme changes and `ready()` after first paint.

## Probe

`-CalternalProbe YES` prints one `PROBE {json}` line to stdout after each load and keyboard change
(web view frame, safe areas, scroll offset, `visualViewport`, the page's element and colour at the
top and bottom edges). `-CalternalProbeMagenta YES` paints every shell-owned layer magenta so any
pixel the shell (rather than the page) draws is visible. `-CalternalProbeViewport <mode>` is an
experiment switch; `-CalternalProbeEdgeEffects YES` turns the scroll edge effect back on.

## Push (design note, not built)

APNs needs the server's web push lane. Design: the shell requests permission only when the page
asks (`calternalNative.push.register()`), receives the device token in
`application(_:didRegisterForRemoteNotificationsWithDeviceToken:)`, and hands it to the page as a
`calternal:push-token` event; the page registers it with the server over its normal authenticated
operation. A notification tap arrives as `calternal://<path>`. Needs the Push Notifications
entitlement and an APNs key (`.p8`, handled like the App Store Connect key: never in git).

## Signing and sharing

`install-on-phone.sh` installs on the owner's phone when reachable; `share-build.sh "title" "notes"`
uploads to the share page. Both archive unsigned, ad hoc sign with the entitlements, and export
with Apple cloud signing through the key in `~/.config/asc` (path passed to `xcodebuild`, never
printed). `Local.xcconfig` supplies the team ID and phone UDID.

## Page loading and cache

The shell loads the top-level page with `reloadRevalidatingCacheData`, on launch, for `calternal://`
links and when the app returns after more than 30 minutes away. Subresources use normal caching, so
hashed assets stay cached. Why: a server that sends `index.html` with no `Cache-Control` and an old
`Last-Modified` lets WebKit reuse an old document by heuristic, so a new bundle is never fetched.
Observed with a test server in the Simulator: three launches, three requests for `/`.

`Settings > Calternal > Clear page cache` (applied at the next start) and `calternal://clear-cache`
drop cached files, service workers and offline copies but keep cookies, local storage and IndexedDB, so
the person stays signed in.

## Native chrome (experiment, draft contract)

Behind `Settings > Calternal > Native chrome (experiment)` (or `-CalternalNativeChrome YES`). Off by
default: the page then draws its own sidebar and header and nothing below applies. When on, the page
**publishes a description** of its navigation chrome with `calternalNative.chrome(description)`
(resolves `{native: boolean}`; false means keep drawing your own) and republishes whenever it
changes. The shell draws it in SwiftUI with Liquid Glass and sends back **events**. The shell knows
nothing about threads or notes: it draws rows, selectors, apps and buttons.

When `native` is true the web view becomes the content card, full height under a native header, and
the shell sets on `:root`: `data-native-chrome` (attribute), `--device-r` (card corner radius, 55px fixed) and `--chrome-top` (points the header
covers, including the status bar), `--chrome-bottom` (home indicator). The page then hides its own
sidebar and header and pads itself with those. `chrome(null)` gives everything back.

```jsonc
{ "version": 1, "revision": 7, "scheme": "dark", "accent": "#d9822b",   // revision rises per publication
  "header":  { "leading": [Item], "title": "Expired invite test", "trailing": [Item] },
  "sidebar": { "open": false,                                // page-controlled; shell follows when it changes
    "title": "Calternal", "refresh": true,                   // refresh: pull down sends a "refresh" event
    "selectors": [ { "id": "filter", "selected": "all", "options": [ {"id":"inbox","title":"Inbox","badge":"5"}, {"id":"all","title":"All Threads"} ] } ],
    "sections":  [ { "id": "threads", "title": "Threads", "collapsed": false, "menu": [Item], "actions": [Item],
                     "rows": [ { "id": "t1", "title": "…", "detail": "06:10", "label": "spoken", "hint": "spoken",
                                 "icon": { "symbol": "pin.fill" /* or "image": "data:image/png;base64,…" */ },
                                 "trailing": [ {"kind":"dot"}, {"kind":"symbol","value":"exclamationmark.triangle","tone":"warn"} ],
                                 "selected": true, "leadingActions": [Item], "trailingActions": [Item], "menu": [Item] } ] } ],
    "apps":    [ { "id": "threads", "title": "Threads", "symbol": "bubble.left", "selected": true } ],
    "account": { "id": "account", "initial": "K", "title": "Account", "menu": [Item] },
    "search":  { "placeholder": "Search threads" } } }
// Item = { "id", "title", "symbol"?: SF Symbol name, "role"?: "destructive", "disabled"?, "menu"?: [Item],
//          "checked"?: bool, "sub"?: second line, "input"?: {"placeholder","value"} /* asks for text, returned as event.text */ }
// Badge kinds: dot | count | text | symbol (value = SF Symbol); tone: info (accent) | warn | bad.
```

Events arrive as `window` `calternal:chrome` with `detail` `{type, id?, rowId?, option?, text?, open?,
revision, sentAt}` (`sentAt` is milliseconds since 1970 for latency measurement):
`row` (id), `action` (id of the swipe action, rowId), `menu` (id of the context-menu entry, rowId or
section id), `selector` (id, option), `app` (id), `header` (id of a header item, `title` for the pill;
`rowId` is the button whose menu it came from), `account`, `search` (text, every change),
`section` (id: toggle collapse), `refresh`, `sidebar` (open: the person opened or closed it). An `action` from a section heading button has `rowId` = the section id. The page updates
its stores and republishes; the shell never changes the description on its own, except that the
sidebar opens and closes immediately when dragged or tapped (and says so with `sidebar`).

Reference publisher: `Calternal/Resources/chrome-demo.html` (start address `chrome-demo`).

### What the format cannot express (found while building it)

* **Wallpaper behind the sidebar.** The page's wallpaper lives in the web view; the native sidebar has
  a plain colour. The format needs a `background` (image URL plus blur) or the shell must snapshot the page.
* **Icons.** SF Symbols by name or PNG data URLs; SVG has no native renderer (rasterise on the page).
* **Typography.** No fonts (the "Calternal" wordmark is approximated with a system script face; a `titleImage` is wanted), no Dynamic Type hand-off to the page.
* **Rich rows.** Title, detail, icon and trailing marks only: no avatars, progress, several lines of
  metadata, drag to reorder (wanted: a `moved` event), multi-select, attention cards with buttons.
* **Large lists.** Each revision sends every row; there is no windowing, paging or patch format.
  Fine for tens of rows, wrong for thousands.
* **Sheets and dialogs** (the contract reserves them; not built). **Search** has no scopes, tokens or
  suggestions; results are whatever rows the page republishes. **Header** has no subtitle or status
  dot, and no per-page layout hints (a permanent split view on iPad).
* **Card corner radius** is a fixed 55 pt (the display's true radius is private API).

## Design skills review (2026-11-19)

The owner asked for the eight skills in the "mobile app design" collection of ui-skills.com. They are
third-party guidance: read in full, applied where they fit; `AGENTS.md` and `docs/open-questions.md`
win where they disagree (said below); no install command from them was run. Page-side rules are in
[native-feel.md](native-feel.md).

| Skill | Adopted | Rejected, and why | Concrete change in what exists |
| --- | --- | --- | --- |
| dimillian/swiftui-ui-patterns | `@Observable` state owned at the root and passed explicitly; stable `ForEach` identity; `.task(id:)` for debounced work; item-driven presentation; previews with fixtures; single `progress` value as the source of truth for the drawer | `RouterPath`/tab scaffolding (no tabs or stacks: the card drawer is gesture-driven UIKit); central sheet enum (no sheets built yet) | `ChromeModel` `ObservableObject` to `@Observable`; trailing badges `Badge: Identifiable` (was `id: \.offset`, an anti-pattern); search debounced with `.task(id:)` instead of an `onChange` event per key; rename prompt is a `PromptRequest: Identifiable` (was a tuple behind a flag); `ChromePreview.swift` fixtures |
| figma/figma-swiftui | Use the system control, never rebuild it: glass buttons via `.buttonStyle(.glass)`, `Picker(.segmented)`, `scrollEdgeEffectStyle`, no hard-coded black/white backgrounds, SF Symbols by name | Figma/MCP tooling (no Figma file); `.searchable` bottom toolbar (needs a NavigationStack; the sidebar is a drawer) | circle buttons were hand-built `glassEffect` stacks: now `.buttonStyle(.glass)` + `.buttonBorderShape(.circle)`; the selector was a custom capsule ([before](https://git.kayg.org/attachments/d6ef77aa-c921-4f34-842f-40fc4cefe509)), now the system segmented picker ([after](https://git.kayg.org/attachments/05edb104-4030-4517-8924-f3964e4ac65f)); soft scroll edge on the list |
| joe-bell/apple-web-app | Zero safe-area inset is valid; keep pinch zoom; `touch-action`; keyboard by `visualViewport`; fixed layers follow the layout viewport; `overscroll-behavior` parsed early; theme colour is sampled, not declared | "`interactive-widget` unimplemented in WebKit": contradicted by measurement (it shrinks the layout viewport, evidence in native-feel.md); standalone-PWA items (splash images, manifest) do not apply inside the shell | shell no longer disables pinch zoom; page to-do list in native-feel.md |
| callstackincubator/react-native-best-practices | Measure first; window long lists; stable identity; avoid re-render cascades | Everything React Native specific (bundle size, Hermes, Reanimated) | list windowing recorded as a chrome-format gap (thousands of notes) |
| jakubkrehel/better-accessibility | 44 pt targets; second cue besides colour; accessible name/hint/value; single-pointer alternative to drag; focus/screen-reader order; no timers on needed content | web-only parts (ARIA, tabindex, landmarks) apply to the page, listed in native-feel.md | section "+" and "…" were ~20 pt, now 44 ([before](https://git.kayg.org/attachments/a8f79990-5947-4c03-9549-40c2dcd0f7be)); dots and warnings speak "unread" / "warning"; selected row has the selected trait; card hidden from VoiceOver while the sidebar is open and the sidebar while closed; two-finger Z closes the sidebar; the sidebar button is the non-drag path |
| wshobson/interaction-design | Motion communicates (feedback, orientation, continuity); feedback in the same frame; loading states; interruptible spring | its generic timing table and CSS easings (our motion tokens win, `AGENTS.md`); framer-motion recipes | selection haptic on row, selector and app taps fires with the tap; the drawer states order (card slides and rounds together), 0.42 s spring, bounce 0.12, carries the finger's velocity, interruptible |
| iart-ai/accessible-animation | Tier motion (remove, soften, keep); fade not teleport; live setting | CSS/GSAP/Lenis layers (page-side, in native-feel.md) | Reduce Motion: the drawer uses a 0.18 s linear move with no spring (Tier 2). **Not verified in the Simulator** (no switch driven) |
| ibelick/improve-ui | The method: a finding needs a contract, a runtime proof and one correction; try to falsify each before reporting; plan, do not guess | its limits (three findings, never modify source, plans only): the owner asked for ten ranked changes and lanes implement | the ten below, each with evidence |

### The ten highest-impact changes (ranked by what the owner sees)

1. **Keyboard glass shows flat colour** (page): `interactive-widget=resizes-content` shrinks the layout viewport. [before](https://git.kayg.org/attachments/e9dd2282-d244-4995-a44b-76e4aef0309a), [after](https://git.kayg.org/attachments/07b64736-b8f7-4602-b00f-0b26a3ca3069). Page lane's change.
2. **Solid band at the top** (shell, done): the iOS 26 scroll edge effect faded the page into the shell's colour. [before](https://git.kayg.org/attachments/4427f0a8-8c0a-45e8-acfd-31a71be1aa87), [after](https://git.kayg.org/attachments/3699863f-609d-4157-b54c-d262f329b921).
3. **Stale `index.html` kept for ever** (shell, done): the main document is revalidated on launch and after 30 minutes away. No frame; evidence is a request log: three launches made three requests for `/` (the "before" behaviour was not measured in the Simulator).
4. **Native search under the keyboard overlapped the list** (shell, done): [before](https://git.kayg.org/attachments/0dbcaf85-4d02-4fdb-87f3-7ba0c8c54653), [after](https://git.kayg.org/attachments/44db382e-f97b-4047-a35d-c0b0fdfaa53a) (material behind the bar, 44 pt cancel).
5. **Sidebar has no wallpaper** (open): the native sidebar is a flat colour where the page's sidebar shows the blurred wallpaper ([before](https://git.kayg.org/attachments/1b7a84fa-15bc-4fc0-891f-bbf6c2bcc086)). Needs the coordinator's choice between a snapshot, a `background` field, or a floating glass sidebar over a full-screen web view.
6. **Tiny section buttons** (shell, done): 20 pt to 44 pt. [before](https://git.kayg.org/attachments/a8f79990-5947-4c03-9549-40c2dcd0f7be).
7. **Selector was a hand-built capsule** (shell, done): now the system segmented picker. [before](https://git.kayg.org/attachments/d6ef77aa-c921-4f34-842f-40fc4cefe509), [after](https://git.kayg.org/attachments/05edb104-4030-4517-8924-f3964e4ac65f).
8. **Pinch zoom disabled** (shell, done): re-enabled (WCAG 1.4.4). No frame.
9. **Colour-only marks** (shell, done): unread and warning marks now speak; selected row announced. Not verified with VoiceOver (not driven).
10. **Drawer under Reduce Motion** (shell, done, unverified): 0.18 s linear instead of a spring.

Anti-patterns of the first skill found in what was built, and changed: `ObservableObject` where `@Observable`
fits; index identity in `ForEach`; a tuple behind a boolean for the prompt; per-keystroke event with no
debounce. Not found: giant views, `AnyView`, live calls in `body`, multiple booleans for exclusive sheets.

## One codebase for iPhone and Mac (2026-11-19)

The native chrome is drawn by **CalternalUI** (`apps/apple/CalternalUI`, the native design system that already existed): `Chrome/ChromeShell.swift` is the
phone and desktop layout, `Chrome/ChromeSidebar.swift` the sidebar, both built from CalternalUI's surfaces (`Row`, `Pill`, `SegmentedCapsule`,
`BottomTabCapsule`, `ActionButton`, `Card`, glass) and its generated tokens, wallpaper veils and motion. The description types and the store are in the same
folder. The apps are thin adapters (`apps/ios`):

| Part | Shared | iPhone | Mac |
| --- | --- | --- | --- |
| Chrome renderer, tokens, wallpaper, motion | CalternalUI | | |
| Description, store, events | CalternalUI `Chrome/` | | |
| Bridge protocol and dispatch, `bridge.js`, demo pages, start address, measurement | `apps/ios/Shared` | | |
| Web card, drag, haptics, share, menus, status bar | | `ChromeHost`, `ShellViewController+Platform`, `MenuPresenter` | `MacShell` |
| Entry, windows, menu bar, Settings | | `AppDelegate`, `SceneDelegate` | `CalternalMacApp`, `MacCommands`, `MacSettings` |

On a Mac the description draws as columns (sidebar, card with the header over it); below 760 pt wide it is the phone layout (the card slides). The Mac app
opens one window per `MacShell`; a window shows the plain web view until the page publishes a description.

### Tokens and consistency

There is one tokens source, `packages/ui/tokens/tokens.json`; the web emits CSS from it and CalternalUI generates `Tokens.generated.swift` from it
(`apps/apple/CalternalUI/scripts/tokens.sh`, `--check` fails when stale). Nothing visual is hard-coded in the chrome code: sizes, spacing, radii, controls,
durations, curves and colours are `@Design` metrics and theme values (one spot: a 20 pt box for page-supplied PNG icons, to be replaced by a glyph token).
**Cannot be shared:** the icon strokes (the web draws its own set; native draws SF Symbols behind the same names, `IconName`), the typeface rendering and
Dynamic Type (native follows the system), glass and blur (the web's `backdrop-filter` imitation against the system's Liquid Glass), the veil solver (a Swift
port of `veil.ts`, kept in step by tests; the page can also send its solved veils), hover and press physics, and window chrome.

### The component catalogue definition (proposal)

The chrome is a catalogue of components (header, sidebar, selector, section, row, mark, card, app, account, search, item, icon, menu) with their events.
Proposed format: **one JSON Schema document, `contract/chrome-v1.schema.json`** (written; the draft is in the repository), as the single definition. From
it generate (a) TypeScript types for the page and the plugin SDK and (b) Swift `Codable` types for CalternalUI (today hand-written to match), and keep a set
of shared fixtures (`contract/chrome-fixtures/`) that both sides' tests load, like the existing contract fixtures. The Rust host models follow when plugins
publish descriptions through the server (`crates/core/src/models.rs` stays the source for wire types and exports the schema, as `contract/README.md`
describes; chrome would join that pipeline). Rules: the format is versioned (`version`), additive within a version, readers ignore unknown fields, no
Calternal-specific fields (icons and apps are names, rows are rows), and every field that a platform may not honour says so.

### Performance budgets and measurements

Budgets are a hard gate. Method: `-CalternalMeasure YES` (Settings > Calternal > "Measure performance at start" on the phone) runs the scripted drag, a fling
of the 5,000-row list at 6,000 pt/s and the memory read, and prints `MEASURE {json}`; the display-link `FrameMeter` counts a hitch as a frame over 1.5 refresh
periods; signposts (`cloud.calternal.app` / `measure`) are in place for Instruments. Release builds. On the phone the output is read with
`xcrun devicectl device process launch --console ... -- -CalternalMeasure YES ...`.

| Metric | Budget | iPhone 16 Pro Max, iOS 27.2.1 | Mac Studio (M5 Ultra, 5K at 120 Hz) |
| --- | --- | --- | --- |
| Sidebar drag: frames over 1.5 periods | at most 1 % and none over 3 periods | 72 frames, 0 hitches, p99 16.7 ms (60 Hz run) | not applicable (columns) |
| 5,000-row list fling: hitches | at most 1 %, worst under 3 periods | 298 frames, 1 hitch (0.34 %), worst 47.8 ms | 587 frames, 5 hitches (0.85 %), worst 77 ms (over budget) |
| Description published to first painted frame | at most 50 ms warm | 2.6 ms (second publish) | unchanged republish 3 to 7 ms; a real change (a row's unread mark) 25 to 30 ms; first publish 53 ms to 310 ms (wallpaper preparation); the earlier 81 to 122 ms were publishes during the scripted list fling (window events) |
| Cold launch to the page's first paint | at most 1,000 ms (bundled page; network excluded) | 265 ms | 557 ms |
| Native process memory with one web card | at most 150 MB phone, 300 MB Mac | 36 MB | 284 MB (the 5K wallpaper copies dominate) |
| Bridge round trip | at most 2 ms | 0.39 ms (Simulator) | not measured |

What failed or was not measured: the first Mac measurement of warm publish (81 to 122 ms, over the 50 ms budget) was taken while the scripted fling was
republishing windows. An Instruments Time Profiler trace of the Mac app (22 s, 2,163 samples, 1,990 on the main thread) shows the main thread 90 % idle in the
run loop and the rest dominated by SwiftUI hosting-view layout (`NSHostingView.layout` 391 samples inclusive, 20 % of all samples; app-level frames do not
show because SwiftUI body evaluation is attributed to the graph update), so the cost is a SwiftUI relayout of a 5K-sized window per description, not our decode
(0.5 ms) or the bridge. Fix applied: a republished description that changes nothing the chrome draws (only `revision`) no longer invalidates any view
(`ChromeStore.update`); republished-unchanged now costs 3 to 7 ms and a real change 25 to 30 ms, inside the budget. Still open: the 77 ms stall during the fling and
the first-publish 53 to 310 ms (wallpaper preparation on a 5K window). The phone runs the display link at 60 Hz in the measured build; `CADisableMinimumFrameDurationOnPhone`
is set in later builds. Phone numbers come from
the demo page (14 rows plus a 5,000-note windowed list), not real threads. Web content process memory is separate and was not measured. A first version drew the
windowed list as a `ForEach` over all 5,000 indexes: 75 % of frames dropped on the Mac; it now holds only the window and two spacer rows, so cost follows the window.

### The Mac app

Open `~/Applications/Calternal.app` on the Mac Studio (built by `apps/ios` scheme `CalternalMac`; copy of `build-mac/Build/Products/Release`). It starts on the
bundled `chrome-demo` page; Settings (Cmd-,) sets the start address (for example the fixtures build `http://10.69.69.222:4310/` or `https://calternal.cloud/`).
Menu: View > Toggle Sidebar (Ctrl-Cmd-S), Reload (Cmd-R), Clear Page Cache and Reload (Shift-Cmd-R), File > New Window (Cmd-N, one shell per window).

**Signing and passkeys.** This copy is ad hoc signed (linker), with the sandbox and network only. Passkey sign-in in the web card needs the
`webcredentials:calternal.cloud` associated-domains entitlement (the iPhone has it; Apple's guide "Connecting to a service with passkeys" requires the
Associated Domains capability for the domain), which on macOS is a restricted entitlement that needs a provisioning profile, so this local build cannot
sign in with a passkey (expected from Apple's documentation, not tested; use the fixtures or demo addresses meanwhile). I tried both cloud-signing routes
over SSH (`install-mac.sh`): Developer ID fails with "You haven't been given access to cloud-managed distribution certificates" (the API key is not an Admin
key) and development fails at codesign (the development certificate is in the locked login keychain). **What the owner does once:** open
`apps/ios/Calternal.xcodeproj` in Xcode on the Mac Studio, choose the scheme `CalternalMacPasskeys` (Release, carries the entitlement) and "My Mac", press Run
once so automatic signing creates the Mac profile and registers the Mac; afterwards builds can be started the same way. The bundle id is the iPhone's
(`cloud.calternal.app`), so the server's `apple-app-site-association` entry for the iPhone app also covers the Mac app.

### Version negotiation (chrome description)

The shell states which description versions it draws: `window.calternalNative.chromeVersions` (today `[1]`, from `ChromeDescription.supportedVersions`, the one place).
The page publishes only a version in that list; with no `calternalNative`, no `chromeVersions` (an older shell) or no common version it draws its own chrome
and never calls `chrome()`. The shell enforces the other direction: a description with an unsupported `version` is refused with the reply
`{native: false, reason: "unsupported-version", supported: [1]}` (a malformed one with `reason: "rejected"`), and if native chrome was on it is removed, so the
page gets its own chrome back on any mismatch. Verified in the Simulator (version 2 refused with `supported: [1]`; a bad sidebar title rejected). Versions are
additive within a number; a new number means the old one stays supported by the shell for as long as pages in the field publish it.

## System primitives audit (2026-11-19)

Rule (owner): use the platform's own primitives; a custom component needs a stated reason. The chrome renderer now draws with these; CalternalUI's `Pill`,
`IconButton`, `ActionButton`, `SegmentedCapsule`, `BottomTabCapsule`, `ToolbarPill`, `Row`, `Badge`, `AttentionCard`, the press and bounce style and
`AppSwitcherMenu` are no longer used by the chrome (they remain for the design-system preview).

| Component | System primitive used | Stays custom, and why |
| --- | --- | --- |
| Mac and iPad window: two columns | none (`containerBackground(for: .window)`, `glassEffect`, `.regularMaterial` for the base layer) | the container: macOS 26's `NavigationSplitView` draws its sidebar as a floating glass panel over the content, the inverse of the owner's structure (base layer under everything, sidebar content directly on it with no panel, the middle card floating with rounded corners and a shadow, as on the web and phone); no configuration found hides that panel while keeping the split view, so the two columns are ours and everything inside them is system |
| Title, centred at the top (all platforms) | `Button` with `.buttonStyle(.glass)` over a `ZStack`, in the header over the card | the arrangement (a window toolbar would centre the title on the window, not on the card) |
| Header buttons and menus | `Button` / `Menu` with `.buttonStyle(.glass)`, `.buttonBorderShape(.circle)`, `.menuIndicator(.hidden)` on icon-only menus, `GlassEffectContainer` | |
| Phone sidebar drawer (card slides aside) | `withAnimation`/springs, system `scrollEdgeEffectStyle` | the slide: `NavigationSplitView` collapses to push navigation on an iPhone, no system container slides a card over a sidebar |
| Sidebar list, selection, arrow keys, Home/End, type-to-select | `List(selection:)`, `.listStyle(.sidebar)` (Mac) / `.plain` (phone), `Label`, `Section(isExpanded:)` | |
| Row swipe actions, context menu, refresh | `.swipeActions`, `.contextMenu`, `.refreshable` | |
| Counts and marks | `Text`, `Image(systemName:)` | `.badge` takes one number or text; rows here carry several marks (dot, warning, count) |
| Selector (Inbox / All) | `Picker` with `.pickerStyle(.segmented)` | |
| Attention card | `GroupBox`, `.buttonStyle(.bordered)` / `.borderedProminent` | |
| App switcher | `Picker` `.segmented` of icon-only `Label`s, in the sidebar footer | |
| Account | `Menu` with `.buttonStyle(.glass)` | |
| Search | `TextField` in the sidebar footer | `searchable` needs a navigation container; neither the phone drawer nor our two-column window is one |
| Rename prompt | `.alert` with a `TextField` | |
| Window base layer (Mac) | `.containerBackground(for: .window)`, `.glassEffect`, `.regularMaterial`, `.toolbarBackgroundVisibility(.hidden, for: .windowToolbar)` | the wallpaper picture and its solved veils: no system material knows the person's wallpaper |
| Wallpaper pane behind the phone card | none | the design's pane (blurred copy under the veil) |
| Windowed long list | `List`, `Color.clear` spacer rows | the system list has no virtual data source: spacer rows stand for the rows the page did not send |
| Icons | `Image(systemName:)` through the catalogue `IconName` | page-supplied PNG icons (a 20 pt box) |
| Press, hover, bounce | the system's | nothing: the custom press style was removed (it made the title pill balloon) |

Not used yet and why: `NavigationStack` and sheets with detents (no screen in the chrome pushes or presents a sheet yet), `confirmationDialog` (destructive
actions currently go to the page), `TabView` (the apps are a sidebar footer by decision, not a tab bar).

## Mac window base layer: wallpaper or translucent with frost

Following the owner's own app (kayg/hermternal-apple, read-only; approach taken, no code copied): the window is `containerBackground(.clear, for: .window)`
over **Liquid Glass** (`glassEffect(.regular, in: .rect)`), with a **frost dial** laid over the glass as a regular material at opacity 0 to 1 plus a faint
light tint (`white` at 8 % of the dial) so full frost does not read as a dark scrim. The glass is the blur, so no material goes under it. Sidebar and card
sit on that one base layer, not on differently coloured panes. Default: **wallpaper**, the person's own background, the same as on the web and phone;
translucent is opt-in. Settings shape proposed to the page lane (Appearance): a Mac-only control, shown when `calternalNative.capabilities` contains
`window-material`: "Window background: Wallpaper | Translucent", and under Translucent a "Frost" slider 0 to 1 (default 0.2, live while dragging, saved when editing
ends). Stored with the account as `appearance.material` (`wallpaper` | `translucent`) and `appearance.frost`, published in the description; ignored on iPhone and iPad.
Until the page sends them, the Mac app's own Settings (Cmd-,) holds the same two values.

## Keyboard navigation (measured on the Mac, real key events)

The sidebar is a `List(selection:)`, so Up and Down, Home and End, Page Up and Down, type-to-select and the focus ring are the system's; the selection paints
natively at once and the page hears `row` after 45 ms of quiet (a held key collapses into its last row), so the web card loads the settled item only.
Measured with injected HID key events (autorepeat flag, the app frontmost), holding Down for 5 s through a real 5,000-row list: only about 10 key events a
second were injected (the injector, not the system's fastest repeat of about 33 a second), 56 selection changes; paint delay after each change median
12.7 ms, 95th percentile 124 ms, worst 187 ms; the first selection after launch 470 to 505 ms (building the list). **The budget (0 hitches, paint within one
frame at the system's fastest repeat) is not met**: the median is about 1.5 frames at 120 Hz and the tail is long; the cause (SwiftUI list update and row
realisation) is not yet traced (an Instruments export failed). A first version that read the selection in the list's body was 22 ms median: the selection is
now read only by a tiny reporter view. Windowed lists cannot be walked past the window with the keys (spacer rows are not selectable): a sentinel row that
requests the next window is the open item. The iPhone with a hardware keyboard uses the same list; not measured.

### Colour rules for chrome controls

The accent colour is for **selection** (the selected row, the selected segment) and **primary actions** (an Approve button) only. Header and footer controls are
neutral glass with primary-colour symbols: the shell sets `.tint(.primary)` on the header and the footer groups, because a system glass button otherwise takes
the tint (the page's accent, or the system blue when none is published) and the pills read as accented for no reason. A control that is not selected and not a
primary action must never be tinted.

### The phone's top band, and its test

The band in the top safe area came from the card pane, not the page: `WallpaperBackdrop` finds its place through the named coordinate space `ShellSpace.window`,
and under the status bar that space starts one safe-area inset below the screen, so the blurred wallpaper copy slid down 62 pt and the unveiled wallpaper showed above it.
The phone and wide layouts now draw the pane at screen size (`PhonePane`) with no named space. `apps/ios/Tests/top-safe-area.sh` (Simulator, on the Mac, after `./sim.sh build`)
launches the demo closed and with the sidebar open, takes a screenshot of the Simulator, and fails when the strip above the safe-area line differs from the strip
below it by more than 14 of 255 at two columns clear of the header buttons: it measured steps of 22 to 60 on the old build and 0 to 1 now.
