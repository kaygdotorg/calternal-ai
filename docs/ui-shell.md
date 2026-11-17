# App shell

Decided by kayg on 2026-11-17. This is the target for all UI work and replaces the layout
described in `docs/ui-preview.md`. Visual language comes from the mockup in
`/home/kayg/Developer/calternal` on branch `review/switcher-a-pills` (`apps/mockup`,
`packages/ui`), except where this document says otherwise. UI is implemented by Claude only.

## Principles

- Simplify wherever possible.
- Apple-native feel throughout. Icon sizes, spacing, and transparency/blur are tuned
  separately for desktop and for mobile, not shared values scaled down.
- Built from reusable surfaces: pill, capsule, toolbar, card. No one-off chrome.
- Transparency and blur are adaptive: a surface responds to what is behind it instead of
  using one fixed tint.
- One seamless tinted background runs behind the whole app.
- The shell must be extremely performant; measured on real devices, not assumed.

## Three regions

All three change with the current app and are exposed to the plugin API, with good defaults.

### Sidebar (left)

Not a card: it sits directly on the tinted background.

- Header: the app's name as a wordmark in a cursive font, with no icon. The name and its
  font are set in Settings → Appearance. No horizontal scrolling app strip.
- Attention card: things that need the user's attention, at most 3 items. It is a card
  because it asks for attention. With nothing to show, the card is absent.
- Body: app-specific. In Threads the person makes their own sections and drags threads
  freely between them; nothing is grouped under an automatic project label. Double-click
  renames a thread; right-click opens a context menu.
- The sidebar's content fades out at its top and bottom edges; no hard cut-off.
- Footer: account picture on the left; the app switcher in the middle; a search icon on
  the right. Switching always happens at the bottom: apps in the sidebar footer, panels
  as tabs at the bottom of the right card, the tab bar on phones. The header's app name is
  a label, not a control; discoverability wins over removing the switcher.

### Middle card

- Where the app lives. Blurred and dimmed over the background.
- Its top and bottom edges touch the viewport.
- Notes default: a narrow centred column with no inner card, and a thin rail at the card's
  left edge that expands into a small table-of-contents card.

### Right sidebar

- Flat, directly on the tinted background like the left sidebar. It is not a card; only
  the middle is a card.
- AI chat by default.
- Tabs at the bottom switch between AI chat and the current app's panels. The app's
  plugin decides which panels exist (for Notes: the inspector and style sheet).

## Backgrounds and fonts

- The background behind both sidebars is a wallpaper: the person's own picture, one from
  a good built-in set, or one from Unsplash. It is blurred once when chosen, never per
  frame, so scrolling stays smooth.
- Fonts are a personal choice with a polished default (Google Sans Flex, Maple Mono):
  from Google Fonts, Fontsource, Fontshare, the person's own files, or the system font.
  Chosen fonts and wallpapers are stored on the server; pages never load from those
  services while in use.
- The reference for both, and for the overall polish, is kayg's bb plugin `calternal-ui`
  (`/home/kayg/Developer/bb-plugins/calternal-ui`). Calternal must be at least that good.

## Interaction rules

Decided by kayg, 2026-11-17, from the first live preview.

- Focus rings appear only for keyboard focus, never from a mouse or touch.
- Controls animate when used by pointer or touch, never when invoked from the keyboard.
- Motion is non-linear (springs or eased curves) and never blocks input; a surface never
  appears before its material is ready (no transparent popover that then blurs).
- Blur is cheap: wallpapers are blurred once when chosen; live blur is limited to small
  surfaces. Performance decides every material choice.
- The top of the middle card fades content out; no solid band.
- A caret sits exactly on the text baseline of its placeholder and text.
- Icons come from one set, at one optical size and stroke per tier, aligned to the text.
- State indicators say something or are absent: no dot for idle.
- A thread shown in the attention card is not also listed below it.
- Selecting text in a chat offers two actions: **Comment** (puts the selection in the
  composer as a quote, a blank line, then the caret) and **Ask in Side Chat** (opens the
  right sidebar's chat with the selection quoted).
- Interaction craft follows Emil Kowalski's and Jakub Krehel's published guidance and the
  Hallmark skill; visual presets follow the Craft study in the existing app's repo.

- Nothing draws out of bounds: rings, menus and popovers stay inside the window and their
  container.
- The accent colour is inferred from the wallpaper by default, never a fixed brown; it can
  be overridden.
- One design system defines every surface (pill, capsule, toolbar, card, menu) with its
  transparency and blur, and those adapt to the contrast of what is behind them, as the
  existing app's primitives do.
- Sidebars fade at top and bottom; no solid bands anywhere.
- Pickers (model, machine) show an icon per provider and have an inline search field.
- Message text is not streamed by default: a turn shows that it is working and its steps,
  and the finished message appears at once. Streaming is a setting.
- Keyboard shortcuts are discoverable with Cmd-/ or ? and configurable, as in the
  existing app.
- The composer doubles as a command and search palette that understands plain language,
  after the existing app's unified composer.

- No lone standard-sized icon: an icon that is a control sits in a pill. Deliberately small
  inline icons (a message's actions) are the exception and stay bare.
- Each message has small actions beside its time (copy, reply in side chat, more) that
  fade in on hover or focus, with tooltips.
- Threads outside any section sit in a "Threads" section below the attention card.
- The app switcher capsule shows labels only, three apps by default, on the pill surface.

## Preview

The web UI is deployed for review at `https://preview.calternal.cloud/ai/` (served from
`~cua/preview/ai` on `calternal-dev-02-std`, NetBird only).

## Order of work

No UI lane starts until the foundation is solid and break-tested. The orchestrator is the
first app built, so that development can move onto it.

## Open

- Narrow screens, to be drawn out: the sidebar and right card slide over; the current view
  slides into them; the mockup's bottom tab morph is kept. The middle card must visibly
  float above the sidebar, which it does.
- The backdrop-sampling technique for adaptive blur and its frame cost.
