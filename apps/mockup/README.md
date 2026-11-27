# calternal mockup

A standalone, fixture-only Svelte app for judging interface ideas. It has no server and no plugin behind it, and depends on nothing of the workspace except `packages/ui` (tokens, styles, components), by path. Visual reference: the old app's mockup, `docs/ui-shell.md` and `docs/ui-design-system.md`.

- `npm run build` builds `dist/`; `./run.sh` serves it on port 4313 (`serve.mjs`) and prints the address.
- The gear button (bottom right) opens the **tweak panel**: mockup-only switches for the open points of the design.
- The address can force a state for captures: `?app=notes&tab=collections&coll=lisbon&steps=5&scheme=dark&wm=caveat&phone=1&tweaks=1&sheet=search`.
- First screen built: the shell sidebar (wordmark, switcher card with Apps and Collections that resizes to 3, 5 or 7 rows by pulling its grabber, the selected app's or collection's sidebar, the foot with profile and search).

## Second round (owner answers, 2026-11-20)

- The top line is the wordmark when nothing is open and morphs (torph, MIT, see `THIRD-PARTY-NOTICES.md`) into the name of the open app or collection; choosing the open row again, or tapping the name, morphs it back. Nothing open shows a blank middle page headed by the wordmark.
- The card: rows scroll inside it; the resize handle is hover or focus only on a desktop (always on touch); tab switch, reorder and resize animate with the motion tokens. Apps are in frecency order (fixture history) until one is pinned or dragged, then in the person's own order (grip, drag, Alt+Arrow, pin); the tweak panel resets it.
- Settings is reached only from the profile popover. A collection opens in the middle card as list, table, board or gallery; the sidebar does not change.
- Tweaks: card at the top of the sidebar or the bottom of the middle card; Mac window (traffic lights, sidebar button, wordmark below them); the app-name face (interface or cursive).
- Phone: the foot is a floating tab bar (profile item and a detached round search button).

## Conventions every screen follows (checked by measurement, see the review report)

- **One title edge.** A sidebar row is: padding 12, a fixed-width lead slot (`--lead`, 22 px, in `src/shell/rows.css`), a gap (`--lead-gap`, 8), the title. So every title starts 42 px from the sidebar's edge, whatever the mark is (an icon, an avatar, a check, a thumbnail, a day number).
- **One right panel.** `src/shell/PanelFrame.svelte`: a 22 px title with a close button, the app's details in the body, and the same Details / Assistant bar at the bottom. An app supplies only its details (`panel`). The panel is closed when nothing is open.
- **One date formatter.** `src/lib/when.ts`: a clock time is `09:24`; a day in a list is `Today`, `Yesterday`, `Tomorrow`, a three-letter weekday inside a week, else `14 Nov`; a day on a page is `Friday 20 November`, with the time after a comma. Today is Friday 20 November 2026. An app writes a moment only through these.
- **Counts and chips.** A filled accent badge means "needs you" (unread); a plain grey number is a total. A chip is one pill: 20 px high, 12 px, an ink tint (`.chip`-style rules in the shared components); state shows in the glyph, never in coloured words (text stays in the ink so it passes contrast).
- **Type, spacing, radii, motion** come from the tokens only: sizes 11, 12, 13, 15, 22 (the phone's touch scale for the same tokens), weights 440 and 620, spacing `--s-*`, radii `--r-select|menu|card|pill`, durations and curves `--dur-*` and `--ease-*`. The wordmark (32 px, 600, a cursive face) is the one written exception.
- **Touch.** Every control is at least 44 pt on a phone (`rows.css`, under `hover: none`).
- **Contrast.** The sidebar, the card and the right panel each use the secondary inks solved for their own veil (`App.svelte`); the veil is solved for the worst pixel, with headroom (`vendor/appearance/veil.ts`).
- **Taste options** (the owner's call) are switches in the tweak panel with today's behaviour as the default: item name once, the grabber over the last row, a neutral selected tint, centred gallery captions, collection icons, a denser searchable profile popover.
- **Gallery tiles, one visual language** (my choice, stated so you can change it): every tile is one frame (the same size and radius) with a title line and one muted line under it. What is in the frame depends on the kind, in three tiers. PICTURE kinds (photo, file, note) show their picture, filling the frame (a note shows its page, cropped). CARD kinds (task, mail, event) show their card, centred. MARK kinds (person, chat, transaction, workout, label, setting) show a mark or a big figure centred on a quiet tint.
- **Keyboard.** Each list (the sidebar, the card's rows) is one Tab stop with the arrow keys, Home and End inside it, Enter to open, `m` or the menu key on a section heading for its menu, `p` on a card row to pin it, Space on a task to tick it, Escape to leave. The sidebar is about nine Tab stops in all.
- **Reduced motion.** A global rule makes transitions and animations 1 ms; script-driven motion (the wordmark morph, the window split and close, the board's rise, the list reorder) asks `reduced()` in `src/motion.ts`.
