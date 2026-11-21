Independent design review of the mockup (Opus 5.5, medium reasoning), commissioned by the coordinator on 2026-11-20 night. Reviewed: the live mockup of `lane/threads-ui` at about 19:00 UTC (before commit 1ea7041f, which already addresses D1 and part of D5). The capture numbers refer to the reviewer's own screenshots, which are not kept in the repository.

I captured 31 states plus about 40 interaction frames with Playwright on dev-02, and measured type, edges, icons, radii, transitions, contrast and tap targets in the page. PNGs are in `/tmp/design-review/` on this machine; my scripts are in `/tmp/design-review/local/` here and `/tmp/design-review/` on dev-02. I changed no repository files and touched no services.

## 1. Findings, most visible first

Unless a row says otherwise, measurements are CSS px at 1440×900.

**Defects (wrong by the project's own rules)**

| # | Where | What is wrong | Evidence | Rule broken | Fix |
|---|---|---|---|---|---|
| D1 | Notes, Mail, Tasks, Contacts with an item open; desktop and phone | The title pill names the app ("Notes", "Mail", "Tasks", "Contacts"), not the open item. Chat, Files and Photos are correct. | `05`, `06`, `08`, `12`, `28` | Title pill names the window's content | Have each of these apps supply a title for the item open in its window (`titles[w.ref]` in `Canvas.svelte`). |
| D2 | Every window | The pill does not hug its text, and it is a different height from the toolbar beside it. | `.cv-tp` has `min-width: 6rem`, so "Notes" sits in a 96 px box. Pill is 28 px tall; the toolbar capsule next to it is 38. | "Pill hugs its text"; `--pill-h` 38 is the one control height | Drop the min-width and use `--pill-h`. |
| D3 | Collections, list view | File and photo rows are two lines with no mark (e.g. "Lease 2026.pdf / 412 KB"). Title edges drift between 505, 508, 511 and 514. | Two-line rows are about 46 px tall against 32 for the rest (`13`) | Rows are one line | One line with the size on the right, and a file or photo mark like every other kind. |
| D4 | Collections, gallery and board | Photo, PDF and spreadsheet items draw empty grey tiles or no tile at all. Captions are centred for people and left-aligned for everything else. Tile rows have uneven heights. | `15`, `16` | Craft-style thumbnails; plain craft | Use the existing `FileThumb` and photo scene in `Item.svelte`'s tile and card views; left-align all captions. |
| D5 | Sidebar rows, all apps | Row titles start at three different x positions: 44 (Notes documents, task circles, the action rows "New note", "Write"), 48 (app rows, avatars, events, mail) and 50 (Files and Photos thumbnails). | Measured title left edges. The coordinator's 44 vs 42 is this same drift; I measure 50 vs 48. | One edge | One fixed-width lead slot so every title starts at 48. |
| D6 | Text sizes and weights, all apps | More than six sizes and three weights are drawn. | Avatar initials at 0.38 × diameter: 6.84, 9.12, 10.64, 14.44, 16.72, 33.44. Code at 11.96. Selector labels at weight 600 (chosen) and 500 (others) in packages/ui `Selector`. Wordmark at 600. | "Six sizes"; weights 440/460/620 only | Initials at `text-label` or `text-section`; Selector to 440/620. |
| D7 | Icons | The three-dot "more" glyph uses `stroke-width: calc(2.6px)`. | It renders at 1.52 px (14 px glyph) and 1.95 px (18 px "Window menu"), against 1.40 and 1.42 for every other glyph. Confirms the coordinator's "heavier stroke" icons. | One stroke per tier | Use the tier stroke tokens. |
| D8 | Mail item | Wrong glyphs on the actions. | Reply and Reply all use `icon="back"` (`mail/Thread.svelte:82–83`); Forward uses the share glyph. | Icons must mean the action | Add reply and forward glyphs. |
| D9 | Task page | Deadline and Priority both use `flag`, so two identical flag buttons sit side by side. | `tasks/TaskPage.svelte:17–18` | Same | Give one of them a different glyph. |
| D10 | Mail rows | Unread rows show a dot and a count badge together ("08:41 • 10"). | `06` | "One state signal" | Badge or dot, not both. |
| D11 | Right panel, across apps | Header is 22 px in Chat, Notes, Files, Photos, Tasks and collections, 15 px in Mail, Calendar and Contacts. A close button appears only in Chat, collections and the sidebar view. The bottom tabs are full width in most apps but end 48 px short in Tasks; Notes has four icon tabs and no Assistant tab. On the home page the panel shows a "Properties" heading with nothing under it. | `01`, `05`–`12` | One surface, one pattern | One panel header component with a close button; the same tab bar everywhere; hide the panel when nothing is open. |
| D12 | Light scheme, sidebar and right panel | Secondary text is under 4.5:1 almost everywhere. | `ink-3` rgb(85,85,85) on a backdrop of about rgb(183,178,175) is 3.52–3.65:1 (20 of 59 sidebar texts; 15 of 31 right-panel texts). Timestamps in chat bubbles (opacity 0.6) are 2.26:1. | Design system: ink-3 at 4.5:1 | Run the veil solver in the mockup, or darken ink-3 to about rgb(70,70,70). |
| D13 | Dark scheme | Several texts are well under 4.5:1. | ink-3 rgb(166,166,166) over sidebar mid-tones is 3.2–3.8:1; "New section" is 2.15:1; the calendar SAT/SUN chips are 1.84–1.97:1. | Same | Same fix. |
| D14 | Window divider drag | The window lags the pointer. | The 350 ms `flex-basis` transition stays on during the drag. On the last move the window was 339 px wide; it should have been 238. On release it jumps about 100 px. | "Following the finger, one to one" | Turn the transition off while dragging; keep it for the snap. |
| D15 | Off-token motion | Some transitions use plain CSS curves and off-scale durations. | Selector: `120ms ease-out` opacity, `250ms ease-out` colour, `150ms ease-out` scale (the CSS keyword, not the `--ease-out` token). Toasts: `160ms ease-out`. Files: `220ms ease` background. | Motion tokens | Use `--ease-out` and the `--dur-*` tokens. Also, `ui-design-system.md` says `--dur-fast` is 120; the token is 150. Make them agree. |
| D16 | Chat, desktop and phone | The floating @ (jump to mention) button covers the last message ("Merging the fix n…"). Code blocks are clipped with no scroll cue. | `03`, `i01`, `j05` | Nothing draws over content | Reserve room for the button above the composer, and fade the code block's edge. |
| D17 | Chat fixtures | The thread "Release notes" has provider claude, yet Codex answers in it. The two agent rows in "Cals" show a play and a search glyph instead of the provider mark. Properties show raw values: "Kind chat", "Agent claude", "Default view list". | `03`, `13` | Provider marks on agent chats; plain language | Fix the fixture text and the values shown. |
| D18 | Narrow sidebar (dragged to 200) | Section headings truncate to "Gr…", "Pe…", "Ha…" while row titles still fit. The foot shows "Lind…" and "Ada L…". | `i04` | Plain craft | Lay the hover actions over the heading instead of reserving their width. |
| D19 | Sidebar bottom, all apps | Not what the coordinator thought: rows never go under the foot (the scroller ends at y=840, the foot starts at 846). But the 16 px fade is half a row, so a row or heading always shows cut through its middle ("Work", "Favourites"). | `05`, `j09` | "Fades, no hard cut-off" | Make the fade at least one row tall and add bottom padding equal to it. |
| D20 | Phone chat, dark | Messages are cut hard under the header (the "Hana Sato" name is sliced). | `j05` | "The top of the card fades" | Apply the top fade under the floating header. |
| D21 | Phone, small tap targets | Several controls are under 44 pt. | Section menu 26×26; "Aa" 29×24; card grabber 26 tall; mail message rows 32 tall; "to me, Chloe Rossi" 18 tall. | 44 pt minimum | Make them pill height. |
| D22 | Phone Files page | Controls come before the heading: breadcrumb, then List/Gallery and Upload, then the title. Upload's label is visibly heavier than List's. | `30` | Plain craft | Title first; controls in the header or toolbar. |
| D23 | Dates | Several formats for the same thing, and one wrong weekday. | "9:24" vs "09:30" vs "08:41"; "yesterday" vs "Yesterday"; "Wed" vs "Wednesday"; "Sun, 15 November 2026, 12:03" vs "Sunday 15 November · 12:03". The note "Thursday, 20 November": 20 Nov 2026 is a Friday. The Lisbon list row "Museum visit" shows a TUE chip, "Sat, 11:00" and "15 Nov" (a Sunday) at once. | Plain craft | One date formatter used everywhere. |

**Taste (the owner's call)**

| # | Where | Observation |
|---|---|---|
| T1 | Item open, all apps | The item's name appears three or four times: pill, page heading, breadcrumb and right-panel header. Files shows "Lisbon trip" four times. |
| T2 | Switcher card, desktop | About 30 px of empty band sits under the last row, reserved for a grabber that only shows on hover. It reads as uneven padding. |
| T3 | Mail item | The page has its own toolbar with a ⋯ menu, about 70 px below the window's ⋯ menu. |
| T4 | Profile popover | The glass is very clear over busy chat content (the space checkmark collides with an attachment icon behind it). It is wider than the sidebar, and the space names start at a different edge from the rows with icons. It has no space search, which `ui-shell.md` asks for. |
| T5 | Selected row, dark | The selected-row tint is a saturated amber (152,111,23). Grey secondary text on it falls to about 1.9:1. |
| T6 | Collections tab | Collections show colour dots; Apps show icons. |
| T7 | Contacts page | The contact is left-aligned in an 800 px column with an empty right third; Notes centres its column. |
| T8 | Phone foot | The account pill picks up bright wallpaper (it turns yellow) and stands out from the rest of the glass. |

## 2. Measurements

- **Left edge.** The sidebar starts at x=0. Section headings and every row mark start at 18; the wordmark's ink starts at 17. Inside the card that is 12 px from the card's own edge (the card is at x=6), so the lane's "12 px" claim holds measured from the card. The 1 px wordmark offset is optical and fine.
- **Row heights.** 32 for desktop sidebar rows, card rows and section headings. 48 for strips and for every phone row. About 46 for two-line collection rows (D3) and for the sidebar-file rows.
- **Type actually drawn.** 13/440 (349 uses), 12/440 (331), 15/620 (67, section headings), 11/440 (52), 22/620 (12), 32/600 Dancing Script (the wordmark), plus everything in D6. Page thumbnails draw text at 3.9–7.6 px, which is acceptable as illustration. Phone text uses 17 and 20 (the touch scale).
- **Spacing.** Most values are on the 2–48 scale (6, 12, 8, 2, 4, 16, 20 dominate). Off-scale values: 10.67 (68 uses), 8.67 (26), 2.16, 1.44, 4.8, 5.76, 6.48, 3, 5, 10 and 11. These are em-relative or hand-written.
- **Radii.** Token values 8, 14, 20, 999 and 50% are used. Off-scale: 6 (48 uses; file thumbnails, calendar, Look), 11 (42; calendar day cells), 5, 12 (`App.svelte`, `Item.svelte`, `press.ts`), 10, 9 (chat), 4 and 2.
- **Icons.** Two sizes, 14 (stroke 2.4, renders 1.40 px) and 18 (stroke 1.9, renders 1.42 px), except the "more" glyph (D7), 10 px checks at stroke 3 (1.25 px), and the file-type icons at 24 px with stroke 1.
- **Contrast.** For each text box I took the worst pixel of the background with all text hidden. Desktop light and dark are in D12 and D13. Phone light: secondary text 3.3–3.6:1. Phone dark: 4.4–4.7:1, except about 1.9:1 over the selected tint. Primary ink passes everywhere.

## 3. Motion and interaction

- **Transitions in use.** 150, 220, 300 and 350 ms on the token curve and the morph curve, plus the strays in D15.
- **Right panel close.** The middle card goes from 908 to 1228 px wide in about 200 ms and is smooth (190 ms sheet token).
- **Card resize.** Dragging 70 px takes the card from 3 to 5 rows (96 → 160 px) and it settles about 150 ms after release. The grabber has opacity 0 until the card is hovered, can be reached with Tab, and is always shown on touch.
- **Wordmark morph.** It morphs to the app name. I captured frames at 60 to 700 ms but could not judge its smoothness from stills.
- **Phone push.** About 88% of the way across at 60 ms, settled by 150 ms; the list underneath dims. I could not reproduce "the page beneath shows through the glass": the pushed Notes, Files and Chat pages are near-opaque in light and dark.
- **Row hover.** The tint is ink at 5% alpha (rgba(38,38,38,0.05)), so it is very faint. As far as I could see it has no background transition; only the press scale has one.
- **Drag and drop.** All of these worked with a mouse:
  - a row from Groups into People (a ghost reading "Move here" plus an accent insertion line);
  - a section dragged by its heading to the end;
  - "Recent mail" from the card's Collections tab into the Notes sidebar (inserted where dropped);
  - a board card from To do to Done (tilted ghost, column outlined, counts 4/5/5 → 3/5/6);
  - the sidebar resize handle.

  The window divider works but lags (D14).
- **Keyboard and touch.** Section actions, Pin buttons and the grabber can all be reached with Tab, but every app row adds a "Pin" stop, so there are about 20 Tab stops before the sidebar content. I found no hover-only action without a keyboard route. Touch targets are in D21.

## 4. Consistency across the eight apps

- **Page headers.** Six patterns: Notes (centred 22 px title), Files (breadcrumb plus H1 plus controls on the right), Mail (22 px two-line subject plus its own toolbar), Calendar (month title plus a selector), Photos (back link plus a 13 px title), Contacts (88 px avatar).
- **Chips.**
  - Mail: plain "Personal" next to grey chips for "Travel" and "Receipts".
  - Tasks: date chips with an icon; overdue in red.
  - Calendar: uppercase day squares.
  - Notes: a "#journal" chip.
- **Counts.** Olive filled badges in Chat and Mail; plain grey numbers in Files, Photos and Tasks.
- **Sidebar strips.** Avatars (Chat, Contacts) and album squares (Photos) are clipped at the right edge mid-glyph ("C", "Autun") in every app.
- **Right panels and dates.** See D11 and D23.

## 5. Phone (390 wide)

The tap targets in D21, the hard top cut in D20 and the Files header order in D22. Also:
- the pinned strip cuts "DP" and its badge at the edge;
- the @ button covers the last message;
- the title pill shows the app name on Notes pages.

I found nothing unreachable.

## 6. Ten changes for the first minute, ranked

1. The title pill names the item, hugs its text and is 38 px tall (D1, D2).
2. Real thumbnails and one-line rows in the collection views (D3, D4).
3. One right-panel pattern, hidden when nothing is open (D11).
4. One title edge at 48 in every sidebar (D5).
5. Secondary text at 4.5:1 in both schemes (D12, D13).
6. Correct glyphs: Reply, Forward, the two flags, the heavy "more" (D7–D9).
7. A fade one row tall at the sidebar foot and at the phone header (D19, D20).
8. One date and time formatter (D23).
9. Clean chat fixtures: the agent's mark matches its provider, the @ button clears the last message, Properties use plain words (D16, D17).
10. The window divider follows the pointer one to one; section headings stop truncating at narrow widths (D14, D18).

## 7. Questions only the owner can answer

- **Where should the item's name live** (T1)? Today it appears in the pill, the page heading, the breadcrumb and the right panel. I recommend the pill plus the page heading, with the right panel showing kind and metadata only.
- **The empty band under the switcher card on desktop** (T2). Show the grabber faintly all the time, or remove the band and show the grabber over the last row on hover? I recommend over the last row: no dead space.
- **Should the right panel exist with nothing open?** I recommend it stays closed.
- **Selected-row tint** (T5). Keep the wallpaper accent, or use a neutral 8–10% ink tint like macOS? I recommend neutral, with the accent only for the mark.
- **Gallery captions** (D4). One alignment; I recommend left.
- **Collections tab marks** (T6). Colour dots or icons? I recommend each collection's own icon, consistent with Apps.
- **Wordmark weight.** Dancing Script at 600 sits outside the weight tokens. Accept it as a brand exception, or tokenise it? I recommend writing it down as the one exception.
- **Profile popover** (T4). Denser glass and a space search, as `ui-shell.md` specifies? I recommend yes.

## What I could not check

- Backdrop blur and material on a real GPU: headless Chromium does not draw live blur above about 24 px.
- Frame timing and fluidity on real devices; touch long-press drags; dragging a row off the pinned strip; moving a window by dragging its pill.
- Reduced motion, focus-ring visuals and tooltips.
- The tweak panel and the Settings app.
- 1024-wide layouts for most apps (I captured Notes, Calendar and Mail, and judged only Notes closely).
- The card at 5 rows: captured (`19-steps5.png`) but not viewed.
- The wordmark morph's curve, beyond stills.

My contrast method takes the worst pixel inside each text box, so a few results are inflated where a box overlaps an avatar or badge. I discarded those (for example the 1:1 readings on the mini-calendar).

## Simplification opportunities

- One `PanelHeader` component and one tab bar for all right panels.
- One shared lead slot in `Item.svelte` that the apps' own sidebars reuse, instead of each app drawing its own rows.
- One date formatter.
- Selector and Toasts in packages/ui moved onto the motion tokens instead of literal curves.
- The title-pill source made mandatory in app registration, so no app can fall back to its own name.
