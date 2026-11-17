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

- Header: the current app's name with a simple icon (not an emoji), fixed. No horizontal
  scrolling app strip.
- Attention card: things that need the user's attention, at most 3 items. It is a card
  because it asks for attention. With nothing to show, the card is absent.
- Body: app-specific.
- Footer: account picture on the left; the app switcher in the middle; a search icon on
  the right. Switching always happens at the bottom: apps in the sidebar footer, panels
  as tabs at the bottom of the right card, the tab bar on phones. The header's app name is
  a label, not a control; discoverability wins over removing the switcher.

### Middle card

- Where the app lives. Blurred and dimmed over the background.
- Its top and bottom edges touch the viewport.
- Notes default: a narrow centred column with no inner card, and a thin rail at the card's
  left edge that expands into a small table-of-contents card.

### Right card

- AI chat by default.
- Tabs at the bottom of the card switch between AI chat and the current app's panels.
  The app's plugin decides which panels exist (for Notes: the inspector and style sheet).

## Order of work

No UI lane starts until the foundation is solid and break-tested. The orchestrator is the
first app built, so that development can move onto it.

## Open

- Narrow screens, to be drawn out: the sidebar and right card slide over; the current view
  slides into them; the mockup's bottom tab morph is kept. The middle card must visibly
  float above the sidebar, which it does.
- The backdrop-sampling technique for adaptive blur and its frame cost.
