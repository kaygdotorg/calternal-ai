# Native feel for the web client

Rules for the page (`apps/web`) so it feels like an app on iPhone, inside the shell
([ios-shell.md](ios-shell.md)) or in Safari. Drawn from the eight mobile-app-design skills the owner
named (reviewed 2026-11-19, third-party guidance: applied where it fits, `AGENTS.md` and
`docs/open-questions.md` win; the review table is in ios-shell.md, "Design skills review"). Each
rule says where it came from and what is measured.

## Safe areas and the keyboard (apple-web-app)

* `viewport-fit=cover` (kept). Mirror `env(safe-area-inset-*)` into `:root` variables once and read
  those everywhere; express bar heights as base + inset. **A zero inset is valid** (SE, landscape):
  never hide or collapse the app waiting for a positive one.
* Never `user-scalable=no` or `maximum-scale=1` (pinch zoom is the low-vision path, WCAG 1.4.4). Stop
  double-tap zoom with `touch-action: manipulation` on tappable elements. The shell no longer
  disables pinch either.
* **Keyboard:** the skill says `interactive-widget` is unimplemented in WebKit. **Measured here it is
  honoured** (iOS 27 Simulator, shell web view): `resizes-content` shrinks the layout viewport to the
  area above the keyboard (`innerHeight` 609 of 956), which is why the keyboard's glass showed flat
  colour ([before](https://git.kayg.org/attachments/e9dd2282-d244-4995-a44b-76e4aef0309a)). Use `resizes-visual`, keep the
  wallpaper on the layout viewport (`position: fixed; inset: 0`), and size only the chrome (composer,
  scroller padding) by `visualViewport` ([after](https://git.kayg.org/attachments/07b64736-b8f7-4602-b00f-0b26a3ca3069)). Feature-detect;
  do not sniff versions (iOS 26.x behaviour has changed between point releases).
* `overscroll-behavior` must be in a stylesheet parsed before the first touch; setting it from script
  mid-gesture does nothing.
* `theme-color` is ignored by iOS 26 Safari, which samples the page's own edge colour: the root's
  background must be the colour that should show behind the bars. In the shell call
  `calternalNative.appearance({scheme, background})` instead.
* Installed or shell-hosted: keep a visible way back or out on every screen (the shell has no browser
  chrome and the edge swipe belongs to the page).

## Touch, focus, names (better-accessibility)

* Touch targets at least 44 x 44 px (24 x 24 is the WCAG 2.2 AA floor); extend the hit area with
  padding or a pseudo-element, never let extended areas overlap.
* Every icon-only control has an accessible name that starts with its visible text; a state shown by
  colour (unread dot, warning) has a second cue (text, icon, or an accessible value).
* Every drag has a single-pointer alternative (2.5.7): reorder needs "move up / move down" actions.
* Toasts that carry an action or an error stay until dismissed; announce non-urgent updates with a
  `role="status"` region rendered before its text changes.
* Text containers use `min-height`, not `height`; the layout survives 200 % text and Dynamic Type.

## Motion (accessible-animation, interaction-design)

* Tier every animation. **Remove** under `prefers-reduced-motion: reduce`: parallax, large slides
  across the viewport (the drawer slide), scale of large surfaces, scroll hijacking. **Soften** to an
  opacity fade of at most 200 ms: card and drawer transitions. **Keep** opacity and colour fades, focus
  rings, progress indicators. Opt elaborate motion in with `@media (prefers-reduced-motion: no-preference)`;
  use `0.01ms`, not `0s`, in a global safety net so `transitionend` still fires.
* Every transition states its order, duration and curve **from the motion tokens** and is
  interruptible (`AGENTS.md`: this wins over the skill's generic 100 to 500 ms table, used only as a
  sanity check). Feedback for a tap within the same frame; a loading state keeps the layout (skeleton)
  instead of collapsing.

## Lists (react-native-best-practices, skimmed)

Not our stack; the transferable parts: stable identity for rows, window long lists (thousands of
notes: render and publish a window, not the whole list), measure before optimising, and avoid
cascading re-renders (publish the native chrome description once per store change, coalesced per frame).
