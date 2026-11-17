# Settings and Appearance

Decided with kayg, 2026-11-17. Terms: [glossary](glossary.md).

## Shape

- Settings is a plugin and an app in the app switcher, with the same layout as every
  other app: sections in the sidebar, the page in the middle card.
- Two groups: **User** (yours, follows you) and **Admin** (the space, and the server for
  its operator). The model is a modern Apple settings window such as Xcode's, not Notes'
  or Reminders'.
- No search box inside Settings; the palette finds every setting.
- Every setting is an operation. An agent changing one follows the same rules as Approve
  for me: reversible changes such as appearance and shortcuts just happen; anything about
  access asks.

## Appearance

- Personal by default; a space can set defaults that members may override.
- Wallpaper is per person, the same in every space.
- A preset sets everything at once; each part stays adjustable afterwards.
- First run shows a short gallery of presets, then goes straight in.
- Fonts: interface, chat and code. One **UI size** slider scales the whole interface.
- Materials are three checkboxes: **Blur**, **Dim**, **Tint**. No sliders. The values are
  chosen automatically from the contrast of the selected background so everything stays
  readable in light and dark.
- Motion follows the system's reduce-motion setting, plus an off switch.
- The accent is inferred from the wallpaper; the wordmark and its font are set here.

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

Each is at `https://unsplash.com/photos/<Photo ID>`.

## Being tried

- The app switcher as a three-item capsule at the bottom of the middle card, always
  visible by default, which frees the sidebar's footer for an app's own tabs.
