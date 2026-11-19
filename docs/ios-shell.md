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
the shell sets on `:root`: `data-native-chrome` (attribute) and `--chrome-top` (points the header
covers, including the status bar), `--chrome-bottom` (home indicator). The page then hides its own
sidebar and header and pads itself with those. `chrome(null)` gives everything back.

```jsonc
{ "version": 1, "revision": 7, "scheme": "dark",            // revision rises per publication
  "header":  { "leading": [Item], "title": "Expired invite test", "trailing": [Item] },
  "sidebar": { "open": false,                                // page-controlled; shell follows when it changes
    "title": "Calternal",
    "selectors": [ { "id": "filter", "selected": "all", "options": [ {"id":"inbox","title":"Inbox"}, {"id":"all","title":"All Threads"} ] } ],
    "sections":  [ { "id": "threads", "title": "Threads", "collapsed": false, "menu": [Item],
                     "rows": [ { "id": "t1", "title": "…", "detail": "06:10", "symbol": "pin.fill",
                                 "badge": {"kind":"dot"} /* or count / text + value */, "selected": true,
                                 "leadingActions": [Item], "trailingActions": [Item], "menu": [Item] } ] } ],
    "apps":    [ { "id": "threads", "title": "Threads", "symbol": "bubble.left", "selected": true } ],
    "account": { "id": "account", "initial": "K", "title": "Account" },
    "search":  { "placeholder": "Search threads" } } }
// Item = { "id", "title", "symbol"?: SF Symbol name, "role"?: "destructive", "disabled"?, "menu"?: [Item] }
```

Events arrive as `window` `calternal:chrome` with `detail` `{type, id?, rowId?, option?, text?, open?,
revision, sentAt}` (`sentAt` is milliseconds since 1970 for latency measurement):
`row` (id), `action` (id of the swipe action, rowId), `menu` (id of the context-menu entry, rowId or
section id), `selector` (id, option), `app` (id), `header` (id of a header item, `title` for the pill;
`rowId` is the button whose menu it came from), `account`, `search` (text, every change),
`section` (id: toggle collapse), `sidebar` (open: the person opened or closed it). The page updates
its stores and republishes; the shell never changes the description on its own, except that the
sidebar opens and closes immediately when dragged or tapped (and says so with `sidebar`).

Reference publisher: `Calternal/Resources/chrome-demo.html` (start address `chrome-demo`).

### What the format cannot express (found while building it)

* **Wallpaper behind the sidebar.** The page's wallpaper lives in the web view; the native sidebar has
  a plain colour. The format needs a `background` (image URL plus blur) or the shell must snapshot the page.
* **Icons.** Only SF Symbols by name; the page's own SVG icon set has no native form.
* **Typography and theme.** No fonts (the "Calternal" wordmark is approximated), no accent colour
  (the system accent is used), only a light/dark `scheme`.
* **Rich rows.** Title, detail, symbol and one badge only: no avatars, status chips, progress, several
  lines of metadata, inline rename, drag to reorder, multi-select.
* **Large lists.** Each revision sends every row; there is no windowing, paging or patch format.
  Fine for tens of rows, wrong for thousands.
* **Sheets and dialogs** (the contract reserves them; not built). **Search** has no scopes, tokens or
  suggestions; results are whatever rows the page republishes. **Header** has no subtitle or status
  dot, and no per-page layout hints (a permanent split view on iPad).
* **Card corner radius** is a fixed 55 pt (the display's true radius is private API).
