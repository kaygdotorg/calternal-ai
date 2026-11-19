# Settings and Appearance

Decided with kayg, 2026-11-17. Terms: [glossary](glossary.md).

## Shape

- Settings is a plugin and an app in the app switcher, with the same layout as every
  other app: sections in the sidebar, the page in the middle card.
- Three sections (2026-11-18): **User** (yours, follows you), **Admin** (the server, for its
  operator) and, after them, **the current space** (members, invites, secrets, instructions
  in `AI/AGENTS.md`, plugins; its name is the section's heading). The model is a modern Apple settings window such as Xcode's, not Notes'
  or Reminders'. On a desktop there is no sheet for settings: the account picture and the
  app switcher open this app, and every page, wallpaper and fonts included, is a page of it.
- No search box inside Settings; the palette finds every setting.
- Every setting is an operation. An agent changing one follows the same rules as Approve
  for me: reversible changes such as appearance and shortcuts just happen; anything about
  access asks.

## Access model

One model everywhere, nothing more complex:

- A **user** is one account. A **group** is a named set of users.
- A **capability** is one permission. A **role** is a named set of capabilities.
- A user or a group is given a role **in a scope**: the server, or one space.
- Space administration and server administration are therefore the same mechanism at two
  scopes (Owner and Manager in a space; Operator on the server), not two systems. Plans
  are groups holding server-scope entitlements.

## Structure decided so far

- The Admin group shows only if you manage something.
- Destructive actions sit in a separate Danger area at the bottom of each Admin page and
  ask for a fresh passkey check.
- Plugins are managed on one page: installed, available, and what each may access; a
  plugin's own settings open from its row.
- Machines are added through a guided flow, with the SSH option.
- The default provider and model are per thread, remembering the last choice.
- Onboarding: pick a look, name your space, sign in to an AI provider. It also asks whether
  you are a developer; what follows from that is its own round.
- Onboarding completion follows the signed-in account. Call `onboarding.state` with
  `{}` to read `{finished: false}` for an unfinished account. Call
  `onboarding.finish` with `{}` once to persist `{finished: true}`. Both use
  `POST /api/v1/operations` and derive the account from the person session.
  Repeating finish returns the same state.
- No separate history or export features: settings are files, so version history is the
  record of changes (with undo) and copying the files is export and import.

## Appearance

- Personal by default; a space can set defaults that members may override.
- Color scheme, UI size, app order, chat width and backdrop clarity are
  local to the device. The shell overlays these on the account's wallpaper,
  materials, palette, accent, preset, fonts, wordmark, motion and streaming
  preferences.
- Wallpaper is per person, the same in every space.
- A preset sets everything at once; each part stays adjustable afterwards.
- First run shows a short gallery of presets, then goes straight in.
- Fonts: interface, chat, code, terminal and wordmark. See the [font and account appearance contract](fonts.md). One **UI size** slider scales the whole interface.
- Materials are three checkboxes: **Blur**, **Dim**, **Tint**. No sliders. The values are
  chosen automatically from the contrast of the selected background so everything stays
  readable in light and dark.
- Motion follows the system's reduce-motion setting, plus an off switch.
- The accent is inferred from the wallpaper; the wordmark and its font are set here.

The [Skills and Automations operations](automations.md) cover portable skill bundles and scheduled turn admission.

## Agents

- A new thread starts in Approve for me; the default is changeable here and per thread.
- Retry and snooze, streaming (off by default) and sandbox idle time live here.
- The decision service is a plugin that supports several providers; off until a key is
  connected.

## Default wallpapers

Chosen by kayg, from Unsplash, credited in a tooltip. The licence allows free commercial
use and redistribution without permission; it forbids only selling unaltered copies and
compiling a competing image service. The image files are release assets, never committed
to git; this list is the source.

| Photographer | Photo ID |
| --- | --- |
| Annie Spratt | `fbV03MdVtJA` |
| Bruce Barrow | `BnOdWOMW7Is` |
| Dave Hoefler | `Nkwrpa_v0WA` |
| Eugenia Pan'kiv | `B8zfbse7mOc` |
| Felix Yu | `_zDcx4dD7L0` |
| Geronimo Giqueaux | `EX_STserBZM` |
| Geronimo Giqueaux | `ngWVYP9NIYM` |
| Nataliya Melnychuk | `I_xBR4NmDhU` |

Each is at `https://unsplash.com/photos/<Photo ID>`. The default is Geronimo Giqueaux's
`EX_STserBZM`. See [the server wallpaper contract](wallpapers.md) for the catalog,
image routes and sign-in fallback.

## Being tried

- The app switcher as a three-item capsule at the bottom of the middle card, always
  visible by default, which frees the sidebar's footer for an app's own tabs.

## Decided 2026-11-18

- Everyone has a personal space from sign-up. The server creates the space and
  its Owner membership with the account. Clients use the `personal: true` row
  from `spaces.list` and its independent space UUID. An additional fixture space
  demonstrates the switcher.
- A space's standing instructions are `AI/AGENTS.md` (only Managers and the
  Owner may edit it).
- The default wallpaper is painted on the first paint from tiny pre-blurred renditions that
  `index.html` carries, from the defaults and with no stored setting; the blurred picture
  replaces it when ready.
- **Being compared before deciding (temporary, Settings > Appearance > Compare, removed after
  the decision):** the readability floor over the wallpaper, 7:1 with text that flips (current)
  against 4.5:1 that shows more wallpaper; and light mode, the derived lighter photo (current)
  against Craft's nearly opaque surface, 0.92 white over blur, on the unchanged photo. Each
  flips live on all eight wallpapers.
- Motion follows the system's reduce-motion setting, plus an **Animations** off switch.
