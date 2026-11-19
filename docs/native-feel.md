# Native feel: the rules for our web components (draft for the owner, 2026-11-19)

The owner's question: where should the buttons be, how should they be painted, where should fields be and how should they look, is the accent consistent, is the spacing right, should it be opaque or transparent, why does this not animate. This is the rule set that answers each, for a web app that will run in a thin native Swift shell on iPhone and must *feel* native.

**Sources.** A = the SwiftUI patterns skill (`Dimillian/Skills`, `swiftui-ui-patterns`: SKILL.md and all 31 reference files, read in full; it states conventions and very few numbers). B = Apple's Human Interface Guidelines for iOS 26, read from developer.apple.com (Buttons, Text fields, Lists and tables, Sheets, Alerts, Action sheets, Toolbars, Tab bars, Menus, Materials, Color, Layout, Typography, Motion, Haptics, Loading, Onboarding, Accessibility, Segmented controls, Searching). C = what this app has already settled (`docs/ui-design-system.md`, `docs/ui-shell.md`, the tokens). **Apple gives few numbers**: where a number is not from Apple's pages it is marked *(convention)*, and where Apple says nothing it is said. Our numbers are our tokens (spacing `--s-1..10` = 2, 4, 6, 8, 12, 16, 20, 24, 32, 48 px; radii `--r-select` 8, `--r-menu` 14, `--r-card` 20, `--r-pane` 28, `--r-pill`; `--pill-h` 44; `--row-h` 48; `--row-h-settings` 52; type on touch: title 34, heading 20, UI 17, label 13, section 11; motion `--dur-fast` 150, `--dur-base` 220, `--dur-slow` 300, `--dur-leave` 200, `--dur-morph` 350, `--dur-sheet` 340, `--ease-out`, `--ease-sheet`, `--ease-morph`, `--spring`, `--press-scale` .97).

Each rule: the rule; the numbers; what implements it; where we break it today. The audit table with a screenshot per finding is `docs/native-audit.md` (frames on issue #15).

## 1. Where the actions go

| Rule | Numbers | Implemented by | Breaks today |
|---|---|---|---|
| **Sheet (phone):** Cancel or Close at the top **leading** edge, the one primary (Done, Save, Send) at the top **trailing** edge, disabled until valid. Multi-step: Back replaces Cancel on later steps; never Cancel, Done and Back together. (B Sheets; A form, sheets) | bar height `--bar-h` 52; both controls 44 pt hit | `Sheet.svelte` (grabber, detents) has no header buttons: each sheet draws its own | Sheets put their actions at the bottom in a row; no common header |
| **Card or full screen, phone:** the primary is **one full-width button at the bottom of the content, in the thumb zone**; a secondary sits **under** it; Cancel or Skip is a quiet text row under that. Back is the **chevron at the top leading corner**, never a text button at the bottom. Apple has no rule for in-content buttons *(convention: alerts stack the default on top and Cancel at the bottom)* | button 44 pt high, full width inside `--s-7` (20) padding, 12 px between stacked buttons | Onboarding card (chevron and one primary built 2026-11-19) | Sign-in card: the button is natural width and left-aligned; Settings rows put buttons beside text and squeeze them on a phone |
| **Card, desktop:** the primary is **trailing** in a row, Cancel before it (leading), Back is the same chevron. (B Alerts: default trailing, Cancel leading) | buttons natural width, 12 px apart | `.go` rows in onboarding | rows put the primary first (leading) |
| **Alert and action sheet:** default on top (stack) or trailing (row); Cancel at the bottom (stack) or leading (row); destructive in red, never the default. At most 3 buttons in an alert, 4 (with Cancel) in an action sheet. (B) | | `confirm()` and `alert()` of the browser today | we use browser dialogs, which look foreign (see 12) |
| **Toolbar:** Back leading; **one** primary trailing; at most about three groups; extras in a More menu; a title under 15 characters; no solid bar fill, a scroll edge instead. (B Toolbars) | | `HeaderPill`, the page header | pages have two or three pill controls with equal weight |
| **Destructive:** its own section or role, in the danger colour, with a confirmation; never in a title menu and never the primary. (A list, form, title-menus; B Buttons, Alerts) | | `Button variant="destructive"` | Sign out is a secondary button in the middle of a long page (fixed in the account menu) |

## 2. How buttons are painted, their shape and hit area

Rank by **style, not size** (B Buttons): same-height buttons; at most one or two prominent per view.

| Rank | Paint | Where |
|---|---|---|
| Primary | accent fill, white label, `--r-pill`, 44 high; one per view | `Button variant="primary"` |
| Secondary | tint (`ink` 7 %), ink label, same size | `variant="secondary"` |
| Cancel, Skip, Back | plain: no fill, ink-2 label (back is a chevron `IconButton`) | `variant="quiet"`, `IconButton icon="back"` |
| Destructive | danger tint (13 %) with danger label; red fill only inside a confirmation; never the primary | `variant="destructive"` |
| Toolbar and glass controls | monochrome symbol, tint only the one primary | `IconButton`, `Pill` |

Numbers: minimum hit area 44 x 44 pt (B Accessibility: 44 default, 28 absolute minimum), about 12 px of space between bezelled buttons and about 24 px around unbordered ones. Every button has a pressed state (`--press-scale` .97, `--dur-fast`) and a busy state (a ring grows in before the label, the label changes to the verb in progress: "Create passkey" to "Creating…"; B Buttons). Labels start with a verb and use title case; a trailing ellipsis when it asks for more input. Breaks today: several quiet buttons are under 44 pt tall in Settings rows on a phone; busy states exist only where added on 2026-11-19 (onboarding).

## 3. Fields and forms

Apple does not prescribe grouped rows or free-standing boxes; its own Settings uses **grouped inset rows**, and the SwiftUI skill says the same (A form, list: "grouped inset sections with titles on a secondary background, rows on primary, whole row tappable"). Rule for us:

- **Entry on a card (onboarding, sign-in): free-standing fields**, label above, one per row, full width, height 44, radius `--r-pill` (our fields), fill `ink` 7 %, 8 px between label and field, 12 px between fields (B Text fields: "evenly space", consistent widths, a label that clearly belongs to its field, placeholder **and** label).
- **Settings and any list of values: grouped inset rows**: a rounded section card (`--r-card`) on the page background, rows 52 high (`--row-h-settings`) with the label leading and the value or control trailing, a hairline between rows, the whole row the tap target, section titles above in 13 px, destructive rows in their own last section.
- **Focus:** the field grows an accent underline from its leading edge (`--dur-base`), the first field of a screen takes focus when it arrives (not on touch, where it would raise the keyboard before the screen is read; A focus); Return moves to the next field (`enterkeyhint="next"`, `"done"` on the last); the keyboard dismisses on scroll; the primary stays disabled until the form is valid (A form).
- **Validation:** check usernames and email **when the field is left**, not on each key (B Text fields); the sentence appears in the **info panel above the primary action** (`Callout`, built 2026-11-19), the field is marked invalid (`aria-invalid`), nothing is announced twice.
- **Attributes:** `inputmode`, `autocomplete`, `autocapitalize="none"`, `spellcheck="false"` on technical fields; a clear button at the trailing end of search fields; secure entry for secrets.
Breaks today: Settings panels use free-standing pill fields inside rows; the onboarding fields validate on every key; no focus underline; no `enterkeyhint`.

## 4. One accent

**One accent colour**, used for the single primary action's fill, the selected item, the focus underline and a progress fill; never for body text, never for two different meanings (B Color: "apply color sparingly", "do not use one color for different meanings"); semantic colours (danger red, warn amber) are separate and only for status. Contrast: white on the accent and the accent as text hold 4.5:1 (our `accent.ts` already searches lightness for this).

**Why the owner saw amber and then purple.** The accent is *derived from the wallpaper* (`accent: "wallpaper"` in the appearance preferences; `accent.ts` takes the picture's dominant hue). The onboarding's fourth step, **Choose a look**, applies a preset at once behind the card ("it applies at once behind this card, so what is chosen is seen"): the picture changes, so the accent changes, and the screens after it are purple. It is by design, not a bug, and it is the one place the accent may change: **as the direct result of the person choosing a look or a wallpaper, and animated** (`--accent-base` registered as a `<color>` property and transitioned over `--dur-slow`, so it is a crossfade, not a flash). Nowhere else may it change underfoot. Breaks today: the change is instant; the Look step does not say the accent follows the picture.

## 5. Spacing (the 8-pt grid on our tokens)

Apple gives no spacing numbers for these gaps (B Layout) beyond about 12 px around bezelled controls and 24 around unbordered ones; the scale below is our proposal on the 8-pt grid *(convention)*, in our tokens:

| Between | Gap | Token |
|---|---|---|
| card edge and content (left, right, bottom) | 20 | `--s-7` |
| card top to the title (no back chevron) / under the chevron row | 24 / 8 | `--s-8` / `--s-4` |
| title and its paragraph | 8 | `--s-4` |
| paragraph and the first field or block | 20 | `--s-7` |
| label and its field | 8 | `--s-4` |
| field and field | 12 | `--s-5` |
| a block and the next block | 20 | `--s-7` |
| last block and the action group | 20 | `--s-7` |
| button and button (stacked) | 12 | `--s-5` |
| a quiet row (Skip) and the note under it | 2 to 4 | `--s-1` to `--s-2` |
| rows in a list | 0 with a hairline (feed rows: 12 top, 6 bottom, 16 sides, no separators; A list) | `--s-5`, `--s-3`, `--s-6` |
| layout margin on a phone | 16 (20 on the widest phones) *(convention)* | `--s-6` |

Breaks today: onboarding cards use one 12 px gap between everything (title to paragraph is the same as button to button); `.go` rows use 8 px between buttons (under Apple's 12).

## 6. Opaque or translucent

**Chrome is glass, content is opaque or a standard material** (B Materials: "do not use Liquid Glass in the content layer"; regular variant when there is text, clear only over rich media; no solid or semi-opaque colour behind bars; honour Reduce Transparency and Increase Contrast). In our terms:

- **Glass (translucent, blurred, our `Glass`/`Panel`):** the sidebar, the header pill, popovers and menus, toasts, the keyboard-adjacent composer bar, tooltips. Regular strength, because it holds text.
- **Opaque (`variant="solid"` or the page's paper):** cards that hold reading or entry (onboarding, sign-in, settings pages, dialogs, sheets' bodies), rows, fields.
- **Never both stacked on text** (glass behind running text).
- **Clear glass** only over the wallpaper where no text is read.
Breaks today: the first-thread composer and empty-state sit on the wallpaper veil with mixed legibility in light mode; the sign-in card is a solid card on a bare wallpaper (fine); the onboarding Look step shows the wallpaper change through a solid card (fine).

## 7. What must animate

Every state change **a person causes** animates, from a token, and can be interrupted; nothing animates by default on a frequent interaction (typing, scrolling); reduced motion replaces movement with a fade and loops stop (B Motion, Accessibility). The list and the tokens:

| Change | Animation | Token |
|---|---|---|
| appearing and disappearing (panel, row, toast, menu) | opacity and a small translate from where it came from; leaving goes back the same way | `--dur-base` in, `--dur-leave` out, `--ease-out` |
| selection (tab, segment, row, chip) | the highlight glides to the new item; the old one fades | `--dur-base`, `--spring` for the glide |
| validation (a field turns invalid or valid) | the info panel opens by growing; a valid tick draws on | `--dur-base` |
| progress (stepper, upload, wait) | the fill grows; a done step ticks with a draw-on | `--dur-slow` |
| navigation (step, page, push) | old content leaves toward where it came from, new content arrives from the move's side; direction follows forward and back | `--dur-fast` out, `--dur-base` in |
| sheets | slide with the detent; the grabber drags | `--dur-sheet`, `--ease-sheet` |
| press | scale to `--press-scale` | `--dur-fast` |
| loading | skeleton rows in the real layout (see 10); the loaded content fades in over them | `--dur-base` |
Direction must match origin (a thing that came in from the top goes out to the top; B Motion). **Why things do not animate today** (code-checked): Settings page switches on a phone swap the card at once; a disclosure (`<details>`, "Or sign in on the machine") opens at once; list rows that arrive or leave jump; picking a look applies at once; the accent changes at once; the sign-in screen has no entrance. Onboarding got a first pass on 2026-11-19 (directional step slide, arrival order, the stepper line filling, the info panel, the busy ring).

## 8. Sheet, full screen or inline

A sheet is for a **scoped task tied to the current context** (compose, edit, share, pick), one at a time, brief; it has Cancel/Close leading and Done trailing, supports swipe-down to dismiss (with a confirmation if there are unsaved changes), and a **medium** detent on a phone when progressive disclosure helps, **large** otherwise (B Sheets). A **push** (full screen on the stack) is for drill-down and for anything long or with several screens (onboarding, Settings pages). **Inline** (panel, toast) is for feedback about what the person just did; toasts come from the top edge with a fade, replace rather than stack, and dismiss after about 2 s (A overlay). One modal at a time, routed through one store (A sheets, app-wiring). Alerts only for things that need a decision (B Alerts). Breaks today: pickers that are popovers on desktop become sheets on a phone through `Popover` (good), but sheets have no shared header; confirmation uses `confirm()`.

## 9. Lists and rows

Plain rows for feeds (16 / 12 / 6 / 16, no separators), inset-grouped for settings; the whole row is the tap target (44 min, `--row-h` 48); a disclosure chevron means "goes deeper", an info button means "details only"; **swipe actions** reveal the one or two actions people use most, destructive last and in red; **context menus** (long press) are a shortcut, never the only way (A menu-bar, B Context menus: about three groups, one level); selection persists as a highlight when navigating a hierarchy and flashes with a check when choosing an option (B Lists); re-tapping the active tab scrolls to the top (A tabview). Implemented by `Row`, `SwipeRow`, `pressRow` (the Threads lane's long-press). Breaks today: not every list has swipe actions; Settings rows mix buttons and text.

## 10. Loading placeholders

Show something at once; keep the real layout and render **three to six skeleton rows of the real row component** at fixed height so nothing shifts; no infinite spinners except one per section for short work; then a dedicated empty state (icon and title) or an error state with its next step; a spinner inside a button for a slow action with the label changed (A loading-placeholders, B Loading). Search and typeahead debounce **250 ms** and cancel stale requests silently (A async-state). Breaks today: lists say "Loading…" text; the Waiting count-up is a spinner without a skeleton.

## 11. Type, contrast, size

Body 17 (our `--text-ui`), callout 13 (`--text-label`) never below 11 (B Typography); title 34 only for the page title; text contrast 4.5:1 (3:1 for 18 pt or bold); layouts reflow at larger text (Dynamic Type: stack, do not truncate), our `--ui-scale` and root font factor are the web equivalent and must reach every component (A theming). Hit 44.

## 12. Identity: who am I, and leaving

A person must see **which account they are in** and **sign out within two taps from the account picture** (the owner could not find it: it was a secondary button at the bottom of a long Settings, Account page, and the picture said "Kay" for everyone because it was a fixed pretend name). Built: the account menu opens with the signed-in person (name and username or email, opens Settings, Account), then the spaces, Settings, and **Sign out** last in the danger colour (confirmation first). The account picture and its accessible name read the real name. Sign out stays in Settings, Account too (same function).

## 13. What needs the native shell (requests, not faked)

1. **Haptics** through a bridge: `selection` (tab, segment, picker tick), `impact` (press, pull to refresh), `success`/`warning`/`error` (outcomes); subtle, never in bursts, a user setting; the page calls `bridge.haptic(kind)` and does nothing without it (A haptics, B Playing haptics).
2. **Passkeys inside the web view:** the app's web view refused the request at once because the site association is missing: request `webcredentials:` associated domains for the server's host (and the matching `apple-app-site-association` file served by the server), or a native `ASAuthorizationController` bridge. Until then the page says "not available in this app yet".
3. **Real sheets with detents and a grabber** (`UISheetPresentationController`) for pickers and pickers-with-search; the web side keeps its own sheet for the browser.
4. **Glass bars over the web view**: a native top bar and tab bar with the scroll edge effect, with the page reporting its title, its one primary action and its scroll offset; the page's own header pill hides when the native bar is present.
5. **Keyboard:** no form accessory bar (the previous, next, done bar covers our action button); interactive dismissal on scroll; the keyboard frame to the page (`visualViewport` is not exact).
6. **Edge-swipe back:** the page owns it (the shell's is turned off); the page reports when it cannot go back (first step).
7. **Share, open in Safari, copy** through the system share sheet; external links to `SFSafariViewController`.
8. **Pull to refresh** and **context-menu previews** natively if the web versions measure worse.
9. **Status bar and home indicator colours** follow the page background (the page reports it; Safari ignores `theme-color`, learned on the Simulator).
10. **Dynamic Type:** pass the system text size so `--ui-scale` follows it.

## 14. The top edge: pills float, there is no band (owner, 2026-11-19)

**Rule.** Header controls are glass pills that **float over the content**; the content scrolls up *under* them and is faded progressively from the top edge of the screen (a smoothstep, zero slope at both ends) over the status area plus the pills' row plus 16 px; no dark strip, no hard edge, nothing cut off under the status bar (bb, ChatGPT and Claude all do this). Implemented by `.edge-fade` (`packages/ui/src/styles/base.css`), set on every scroller under a header with `--fade-t` (status area plus pill row) and `--fade-b`. **What painted the band:** the mask used to hide content completely under the whole pill row and only then ramp over 16 px, so a 110 px band of nothing sat under the status bar (the theme tiles in Settings, Colour were sliced there). Fixed 2026-11-19: the ramp now starts at the screen's top edge and spans the whole inset. A page's first line still rests below the ramp. Breaks today: the sidebars' `.scroll` use the same mask (fixed with it); the right sidebar and canvas windows on phone have their own insets (to check in the audit).

## 15. One corner radius for the card (owner, 2026-11-19)

**Rule.** On a phone the middle card, wherever it is shown (thread, notes, settings, a canvas window), has the radius of the **display's corners**, so beside an open sidebar its corners are the same shape as the screen's (bb, ChatGPT, Claude). Token `--card-r` = `var(--device-r, 55px)` on a narrow shell; 55 px is the iPhone Pro display radius, used until the native shell passes the real value (`--device-r` on the root; **request to the shell lane**: send the device's display corner radius at launch and on rotation). The card is square (0) when the sidebar is closed and it fills the screen, and grows to `--card-r` over `--dur-sheet` as the sidebar opens or drags (as built). Desktop keeps `--r-pane` (28). Breaks today: the card was `--r-pane` (28) on a phone with the sidebar open; canvas windows on a phone were 28 too. Fixed 2026-11-19.

## 16. The wallpaper is the ground (owner, 2026-11-19)

**Rule.** The blurred wallpaper (and its veil) is one layer fixed to the **layout viewport**, the whole screen, under the status bar, the home indicator and the keyboard; content follows the **visual viewport**. Surfaces over it are glass (translucent with a blur) so the picture shows through the sidebar and the bars, as in bb. Implemented by `Backdrop.svelte` (`position: fixed; inset: 0`), and, while a keyboard is up (`html[data-kb]`), the same picture and veil are painted on the root too, because the keyboard's glass shows the root's own background where the page is not drawn. Breaks today: the keyboard's glass showed a flat colour. **Transparency:** bb's sidebar lets the picture through almost undimmed; ours solves a veil for contrast and sits at a heavier default (`Clear to tinted` at the middle), so the picture is faint. That default is a product choice, not a defect: the measured difference and a proposal (a clearer default on the sidebar, the card keeping its veil) go to the owner with this document. **Question for the iPhone shell lane:** the web view's background (`underPageBackgroundColor`) should be the page's bar colour (`--bars`), and which of the safe-area insets it reports to the page.

## 17. Accent consistency across a session

**Rule.** One accent colour, derived from the wallpaper by default (Settings, Appearance, Accent: From wallpaper or Custom). It is the same on every screen for the same wallpaper: onboarding, sign-in, the first thread, Settings. The owner saw amber, then purple, then green: **amber** (the default wallpaper), **purple** after the Look step (he chose a preset with a different picture), **green** in the app (he chose the moon-in-leaves wallpaper in Appearance): each follows the picture, by design. What is wrong is not the rule but the *jump*: it changes instantly and without a word. So: the accent changes only when the person changes the wallpaper or look, as a crossfade over `--dur-slow`; the Look step and the Wallpaper section say "the accent follows the picture"; and Custom is one tap away in Appearance for people who want it fixed.

## 18. Sidebar rows and renaming

**Rule.** Every row of a list is the same row (`Row`): the icon column, the 8 px indent, 17 px text at the row weight; an action row ("Add to this list", "New thread") is a row, not a smaller, bolder, differently indented control. An in-place rename shows a **field**: a filled rounded rectangle (`ink` 10 %, `--r-select`) with the caret inside it, so a text caret never appears bare in a row. Breaks today: "Add to this list" was a 13 px bold button at a different indent between the shortcuts and the tab selector (fixed: it is a `Row` now); the rename field in the thread list shows only a caret on some states (audit: reproduce, then fix).
