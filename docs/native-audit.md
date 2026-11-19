# Native-feel audit: onboarding, sign-in, the first thread, Settings (2026-11-19)

## The ten highest-impact changes first (improve-ui procedure: confidence, then impact, then reach, then cost)

Each row: problem, evidence (the before frame on issue #15), one change, scope, confidence.

| # | Problem | Evidence (before frame) | One change | Scope | Confidence |
|---|---|---|---|---|---|
| 1 | A person could not see which account they were in or sign out (a secondary button at the bottom of a long page; the picture said "Kay" for everyone) | menu (account page, before: audit-light-thread-settings) | Account menu: the signed-in person, then Sign out, two taps from the picture | every screen on a phone | High, fixed |
| 2 | A solid band under the status bar and content cut off under the header pills | band-sidebar-before-after (left) | Fade from the screen's top edge across the status area and the pill row | every scrolling page | High, fixed |
| 3 | The card beside the sidebar had 28 px corners where the display has about 55 (not like bb, ChatGPT, Claude) | band-sidebar-before-after (third frame) | `--card-r` = `--device-r` | every card on a phone | High, fixed |
| 4 | The wallpaper ended where the page was drawn: flat colour behind the glass keyboard | keyboard frames (before) | `interactive-widget=resizes-visual` and the root mirror | every screen with a keyboard | High, fixed |
| 5 | Onboarding errors were loose text under the buttons, the back control a text button below the action, the passkey refusal worded as a cancel | onboarding-error-callout (after); before in the owner's screenshots | `Callout` above the action, chevron back, wording by error name and timing | onboarding, sign-in | High, fixed |
| 6 | The first thread offers a provider that is not signed in, without a word | audit-light-thread-settings (thread) | Warn at the end of onboarding (done); the composer needs a stored check from the server | first run | Medium, web side fixed, server state open |
| 7 | The accent changes underfoot (amber, purple, green) with no explanation | audit-light-onboarding-signin | Crossfade over `--dur-slow` and one sentence in the Look step | onboarding, Appearance | Medium, open |
| 8 | Settings rows are flat with free-standing pill fields and buttons squeezed beside text | audit-light-thread-settings (settings) | Grouped inset rows, field on its own line on a phone | Settings | Medium, open |
| 9 | Sign-in is a natural-width left-aligned button while onboarding's is full width, and it has no entrance | audit-light-onboarding-signin | Full-width primary, the arrival order of onboarding | sign-in | Medium, open |
| 10 | Nothing a person does is felt: no haptics, browser `confirm()` dialogs, no native sheets | n/a (shell) | `haptic()` behind the bridge (selection, success, warning, error: built), our own confirmation sheet | whole app on iPhone | Medium, haptics built; the sheet open |


Against `docs/native-feel.md`. Simulator, Safari, iPhone Pro Max at 440 pt, light and dark; frames are attached to issue #15 (names in the last column). "Fixed" means fixed on `lane/ui-shell` the same day; "Open" needs the owner's decision or the native shell.

| # | Screen | Finding | Rule | Status | Frame |
|---|---|---|---|---|---|
| 1 | Onboarding, every step | Back was a text button under the primary action, and the chevron did not exist | 1 | Fixed: chevron at the card's top left (44 pt), none on the first screen | ob-who, ob-email |
| 2 | Onboarding, every step | Errors were loose coloured text below the buttons, announced as a bare `role=alert`, and the card jumped | 3, 7 | Fixed: `Callout` info panel above the primary action, one announcement, the height animates | ob-error |
| 3 | Onboarding, passkey | "The passkey prompt was closed or timed out" was shown when the browser refused the request at once | 3, 13 | Fixed: wording by error name and timing; the refused case says passkeys are not available in this app or browser yet and to open Safari or Chrome | ob-error |
| 4 | Onboarding, wordmark | A stray comma-like mark after "Calternal" while the page re-centred | 16 | Fixed: the script face's last "l" swings past its advance width; Safari painted the overhang as a separate piece while the word moved; the wordmark's box is padded so the whole stroke is inside it (frames before and after on the issue) | ob-comma-before, ob-comma-after |
| 5 | Onboarding, machine | "connected.. Threads can run here" had a doubled full stop; Skip and its warning were shown with a machine connected; Continue, Add another machine, Back and Skip sat on three ragged rows | 1, 2 | Fixed: one sentence; Skip hidden when the step is satisfied; Skip is one quiet row with its cost under it; Back is the chevron | ob-machine |
| 6 | Onboarding, steps | The stepper showed step 3 (second passkey) as a bare number after it was skipped, so it read as missed | 7 | Fixed: a skipped step shows a dash in a ring, and its label says "skipped" | ob-provider |
| 7 | Onboarding, provider | With a machine connected but no provider enabled it said "No machine is connected yet" and offered nothing; the first thread then offered a composer for a provider that was not signed in | 3, 12 | Fixed on the web side: the step says a machine is connected and no provider is on, and where to turn one on; the last screen warns before the first thread that nothing is signed in. Missing server state: a provider enabled in the space and a stored check (`agent.check`) | ob-provider, ob-done |
| 8 | Onboarding, accent | Amber on step 1, purple after Look, green in the app | 4, 17 | By design (accent follows the picture); the jump is abrupt and unexplained. Open: crossfade and a sentence in the Look step | ob-look |
| 9 | Onboarding, spacing | One 12 px gap between title, paragraph, fields and buttons; 8 px between buttons | 5 | Open: apply the spacing scale of rule 5 when the motion work is approved | ob-who |
| 10 | Onboarding, fields | Username checked on every key; no focus underline; no `enterkeyhint` | 3 | Open | ob-who |
| 11 | Sign-in | The one button is natural width and left aligned (onboarding's is full width); no entrance | 1, 2, 7 | Open | signin |
| 12 | Account picture and menu | The picture said "Kay" for everybody; no account shown; Sign out was at the bottom of a long Settings page | 12 | Fixed: the picture and menu show the signed-in person; Sign out is two taps from the picture | menu |
| 13 | First thread | "Write a message" button above a composer that says "Message Claude Code…": two ways to do one thing | 1, 2 | Proposal (Threads lane's file): remove the button, focus the composer when the thread opens (not on touch), and keep the sentence "Say what you want done in the box below" | thread |
| 14 | First thread | The composer offers a provider that is not signed in, without a word | 12 | Open: the Threads lane's readiness notice needs a stored check; the web side now warns at the end of onboarding | thread |
| 15 | Settings, Colour (top) | A dark band under the status bar: content hidden under the whole pill row and cut off at an edge | 14 | Fixed: the fade now starts at the screen's top edge and spans the status area plus the pills' row | band-before, band-after |
| 16 | Card corners, sidebar open | 28 px radius; the display's corners are about 55 px; bb, ChatGPT and Claude match the display | 15 | Fixed: `--card-r` = `--device-r` (55 px default; the native shell passes the real value), also for canvas windows on a phone | sidebar-before, sidebar-after |
| 17 | Keyboard up | The glass showed a flat colour; the picture ended where the page is drawn | 16 | Fixed: the picture and veil are also the root's background while a keyboard is up | keyboard-before, keyboard-after |
| 18 | Sidebar, Add to this list | 13 px bold button at a different indent between the shortcuts and the tab selector | 18 | Fixed: it is a `Row` | sidebar-before, sidebar-after |
| 19 | Sidebar, rename | A bare caret in a row with no field | 18 | Fixed: `input.inline-editor.rename` has a field background (specificity: `InlineEditor`'s `background: none` won) | rename |
| 20 | Settings, fields and buttons | Free-standing pill fields inside flat rows; row buttons squeezed to natural width beside text; page changes on a phone swap at once | 2, 3, 7, 9 | Open | account |
| 21 | Whole app | Browser `confirm()` and `alert()` for confirmation | 1, 8 | Open: our own alert and action sheet components | menu |
| 22 | Transparency | bb's sidebar shows the picture almost undimmed; ours has a heavier default veil | 16 | Open: owner's decision (a clearer default on the sidebar) | sidebar-after |
| 23 | No haptics, no real sheets, no glass bars | Web cannot do them | 13 | Requests to the shell lane (list in rule 13) | n/a |
