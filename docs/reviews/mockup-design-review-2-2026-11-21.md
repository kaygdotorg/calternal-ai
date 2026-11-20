Second independent design review of the mockup (Sonnet 5.5, medium reasoning), commissioned by the coordinator on 2026-11-20 night after the first review, whose findings it was given. Reviewed: the live mockup of `lane/threads-ui` at commit 00fff43d.

# Second design review of the Calternal mockup (2026-11-20)

I measured everything in the live page on dev-02: Playwright, device scale factor 2, 1440x900 and 390x844 with touch emulation, light and dark. Phone checks also ran at 320. Screenshots and scripts are in `/tmp/design-review-2/` here and on dev-02. I edited nothing in the repository.

## 1. D1 to D23 verification

| # | Verdict | Measured evidence |
|---|---|---|
| D1 | fixed | The pill names the open item in chat ("Trip to Lisbon: dates"), mail ("Your booking is confirmed"), calendar, contacts ("Anna Okafor"), files, tasks ("Renew passport") and notes (it equals the page h1). |
| D2 | fixed | Every pill is 38 px tall and hugs its text: widths 64, 76, 85, 112, 121, 122, 159 and 196. |
| D3 | fixed | Collection list rows are one line with a mark, and titles share one left edge. |
| D4 | mostly fixed | Gallery tiles are all the same size (n=15, 184 wide) and captions are left-aligned. Board cards are all 237.3 px wide. Thumbnails exist for PDF, spreadsheet, note and photo. |
| D5 | fixed | Every sidebar row title is at x=48 in all 8 apps, light and dark, desktop and phone (about 45 rows each). |
| D6 | partly | Sizes are on the scale except 11.81 px in 26 places (code) and 13.455 px on the phone (known). Weights are 440, 620 and 600 (wordmark). Weight 700 and sizes such as 18.8, 13.6, 10.1, 9.7, 7.6 and 4.8 px appear only inside generated thumbnails. |
| D7 | fixed | Rendered stroke is 1.40 to 1.42 px on all 58 glyphs, including "more". |
| D8 | fixed | The mail Reply glyph is a curved return arrow and Forward is a forward arrow (desktop). On the phone those buttons have no glyph, and Forward wraps to a second line at 320. |
| D9 | fixed | Deadline uses a clock and Priority uses a flag. |
| D10 | fixed | Mail rows show either a dot or a count, never both. |
| D11 | fixed | The panel header is 22 px with a close button in all apps. The Details/Assistant bar is at x=1128, 151 px wide ×2 in all 8. The panel is hidden when nothing is open. |
| D12 | partly | Light desktop is nearly clean: 5 low boxes in chat, 1 in tasks. Light phone chat strip badges read 1.9 to 2.1. See section 2. |
| D13 | partly | Dark has 14 low boxes on desktop and 10 on phone. See section 2. |
| D14 | fixed | Window 0 went 398, 374, 362, 350 … 278 for 12 px steps, so it follows the pointer after a 12 px start threshold. Nothing jumps on release. |
| D15 | mostly fixed | Only the Files "Keep downloaded" switch track still uses `background-color 0.22s ease`. Everything else is on the token curve. |
| D16 | partly | On desktop the @ button no longer covers the last message. On the phone its "1" badge overlaps the corner of the "Merging the fix" bubble. Code blocks still cut hard at the edge ("eslint-p") with no cue. |
| D17 | fixed | Properties read "Conversation", "Codex" and "Claude". Agent rows show a provider mark. |
| D18 | partly | Section headings no longer truncate at a 200 px sidebar. The foot still reads "Lind…" and "Ada L…", and the chat Recent rows read "You…" and "Where to…". |
| D19 | fixed | The fade is a full row tall ("Sent >" fades under the foot). |
| D20 | partly | The header name is no longer sliced. Bubbles still run straight up under the pill with a cut rather than a fade. |
| D21 | not fixed | Under 44 on pushed phone pages: mail message rows 358x32, sender 95x22, "to me, Chloe Rossi" 130x18, "Show the quoted text" 34x20, calendar guests 138x32 and 124x32, contacts "Show all" 60x23, Files breadcrumb 126x32, chat composer 205x38. At 320, the calendar week cells are 39 wide. The sidebar screens are clean. |
| D22 | fixed | Phone Files: breadcrumb and actions on one row, title below. |
| D23 | partly | Table view shows "Nov 18" while every list shows "18 Nov". The same fixture says Hike in Sintra is "Today" in the list, "Fri" on the board and "Nov 20" in the table. Train tickets (8 Nov) shows "Sun" on the board. Lease shows "Sun" in the list and "Nov 22" in the table. A note is titled "Friday, 20 November" (comma) while pages use "Friday 20 November". |

## 2. Verification of the lane's claims

- **Sidebar row titles.** All sit at x=48 from the viewport edge, which is 42 from the 6 px inset list. The claim holds, in 32 of 32 app/scheme/viewport combinations.
- **Phone row heights.** These are not uniform: 44 in most apps, 48 in Notes, Tasks task rows and the strips.
- **No phone target under 44.** False for item pages (D21).
- **Radii.** Only 8, 14, 20 and pill appear. The exceptions are 1 to 3.6 px inside Notes page thumbnails, which are illustration. The claim holds.
- **Font sizes.** The claim is mostly true, but there is code at 11.81 px (×26) and the 13.455 px selector labels. Spacing I did not measure.
- **Gallery frame "172 by 129".** I did not reproduce it. At 1440 with the panel open, each tile is 184 wide and 176.3 tall including its caption, and the frame from the screenshot is about 184x137. All tiles are equal, so "one fixed frame" holds. The numbers are width-dependent.
- **Board cards.** All 237.3 px wide, so equal.

**Contrast.** My method hides all text and takes the worst background pixel inside each text box. It found 37 boxes under 4.5:1 at 17 px and below, against the lane's 31. A few are avatar or badge artefacts.

| Where | Text | Ratio |
|---|---|---|
| Chat, light desktop (window) | "Sending…" 2.47, "09:24" 2.43, "Merging the fix now." 3.53 | 2.4 to 3.5 |
| Chat, light desktop (sidebar) | "3" and "4" count badges (12 px) | 4.2 |
| Chat, light phone (sidebar) | strip badges "3", "4", "1" | 2.0, 2.1, 1.9 (and one at 3.31) |
| Chat, light phone (sidebar) | "T", "O" initials | 3.27 |
| Chat, dark desktop (window) | "Sending…" 3.05, "09:24" 3.16, "09:20" 3.8, "edited" 4.12, "09:16" 4.37 | 3.05 to 4.37 |
| Chat, dark desktop, suspect | "Hana Sato" 1.86, "3 unread messages" 1.22, "@Ada" 1.45 | readable by eye, so likely artefacts of my method |
| Chat, dark phone (sidebar) | "T", "O" 3.81, "Jun Park", "J", "P" | 4.25 |
| Notes, Tasks (dark desktop, window) | active "All" filter chip | 1.54 |
| Tasks | "Overdue" | 4.09 light, 1.21 dark (suspect) |
| Mail, dark desktop | "2 attached" | 3.98 |
| Dark phone | calendar "21" 3.84, "Dentist" 4.25, "Chloe Rossi" 4.25, "Renew passport" 4.25, "Museums and what is closed when" 4.25, photos "Lisbon" 3.35 | 3.35 to 4.25 |

Most of the real failures are timestamps and "Sending…" in chat bubbles, the dark selected-row text at 4.25, and the dark "All" chip.

## 3. New findings, most visible first

1. **Defect: reduced motion is ignored.** With `prefers-reduced-motion: reduce`, 120 elements still carry transitions, the same as without it. The press scale (0.97) and the one animation remain. Fix: a global `@media (prefers-reduced-motion: reduce)` that zeroes durations and swaps the morphs for fades. I did not check the JS-driven animations (the wordmark morph).
2. **Defect: narrow windows clip the date column.** A 278 px collection list cuts "Sun", "27 Nov" and "Today" mid-glyph and the toolbar's "⋯", while the title keeps its full width. Rule: truncate the title with an ellipsis and keep trailing columns whole. Fix: the date and count get `flex: none`, the title gets `min-width: 0` with an ellipsis, and the toolbar wraps or overflows into a menu.
3. **Defect: the phone chat pill clips text.** "Paperwork for the accountar" is cut mid-letter against the (i) button, with no ellipsis. At 320 the (i) button is also cut and the composer's agent button is cut at the right edge. Fix: ellipsis plus a reserved button column.
4. **Defect: phone text collision and clipping.**
   - Contacts "Recent mail" runs "Photos from the walkTen is perfect" together with no gap.
   - The Photos "Favourites" title collides with the Days/Months selector at 320.
   - Item pages give "Work", "7 items" and similar no room under the pill.
5. **Defect: internal copy leaks.** Notes and Tasks phone collections read "1 items · a sidebar section's collection" and "7 items · a sidebar section's collection". There is a plural bug ("1 items") and design jargon.
6. **Defect: arrow keys do nothing in sidebar lists.** Tab works, with a visible 2 px outline everywhere. ArrowDown on a row leaves focus on the same row. There are 21 Tab stops before any sidebar content, because each of the 8 app rows adds a "Pin" stop. Fix: a roving tabindex with arrows in lists and the card, and Pin reachable by a key on the row.
7. **Defect: the Table view date format.** "Nov 18" against "18 Nov" elsewhere, in the same collection. "Kind" truncates to "transacti…" and "Owner" to "Ha…" while the Name column has room to spare. The header row is a solid white band on glass. Related to D23.
8. **Defect: the Mail empty-state cards clip text mid-line.** "Pick up where you left off" cards show a half-line fragment under the second preview line. The Mac frame shows two identical sidebar-toggle buttons (one by the traffic lights, one in the foot).
9. **Defect: the tweak panel is too transparent.** The right panel's numbers show through it, and "Assistant" is visible behind its last button. It has no title and starts at a toggle.
10. **Defect: layout shift between views.** The "14 items" label sits 4 px lower in List (69.6 vs 65.6 CSS px) than in Table, Board and Gallery.
11. **Defect: profile popover edges.** Space names start at x=42 while the Settings, tweaks and sign-out rows start at 78.
12. **Defect: four windows, two shown.** With 4 windows, 2 lie under or beyond the right panel and I saw no overflow cue.
13. **Taste:** the dark send arrow on the olive button is low-contrast grey (likely the disabled state). The empty band under the card's last row is still there at 3, 5 and 7 rows (T2). The phone account pill still picks up bright wallpaper (T8). The pinned strip's right-edge avatar and album clip is still present. Tab, panel-tab and card-row hovers change colour but have no tint (hover is on the parent row). The press scale applies to `.ib` buttons but not to `.sc-card .r` rows. Collection list dates alternate bold and regular with no visible reason. Gallery thumbnails are inconsistent: some are text mini-cards, some are glyph on tint, some are full images.

## 4. What the owner will notice first

1. Every window title is now the item name, in a 38 px pill that hugs its text. Good.
2. The sidebar is cleanly aligned at 48 px across all eight apps, and the right panel is the same everywhere. Good.
3. Collection views: one-line list, equal-size gallery tiles and board cards. The Table's "Nov 18" format and the inconsistent weekday/date fixtures stand out. Mixed.
4. Narrow or shared windows clip dates mid-glyph, and phone item pages have collisions and tap targets under 44. Bad.
5. Keyboard and reduced-motion support are thin: arrow keys do nothing, there are many Tab stops, and reduced motion is ignored. Bad.

## 5. What I could not check

- Real-GPU blur and material.
- Frame timing on a real device.
- Touch long-press drags, dragging a row off the pinned strip, and moving a window by its pill.
- The Settings app. I only opened the profile menu and the tweak panel.
- Hover and pressed states of chips, tiles, cards and the board.
- Spacing on the scale: not measured.
- The 1024-wide layouts: I looked at the montage only, with no detailed measurement.
- JS-driven animations under reduced motion.
- Empty states beyond Mail's.
- Long-name truncation: measured only in mail, chat and sidebars.
