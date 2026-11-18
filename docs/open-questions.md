# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-18 09:15 IST.

## Waiting now

| # | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| 5 | Readability floor for surfaces over a wallpaper (provisional, built so kayg can see it): primary text held at 7:1, lesser text nudged toward it, and a surface may flip its text between light and dark as Craft's header does. Leaves 21 to 49% of the wallpaper visible on sidebars | kayg wants to see it: a comparison is being built on the preview | Materials |
| 21 | Light mode over a photo: derive a lighter photo and keep showing a lot of it (built), or do as Craft does and put a nearly opaque light surface over any photo (92% white, about 8% of the photo shows), which works with every photo but shows little of it | See both on the preview, then choose | Light theme |
| 10 | Native font: decide after seeing both on a Mac and an iPhone (deferred by kayg) | Build a switch in the native preview | Native apps |
| 24 | Materials: the middle card and the sidebars look too alike, which hurts contrast, and the Clear end of the slider gives a light-tinted sidebar even in dark mode. Being rebuilt from the calternal-ui plugin's defaults (how blurred, tinted and dimmed the card is against the sidebars) without the heavy blur; Clear-to-Tinted becomes a stepped slider | kayg to review on the preview | Appearance |
| 25 | Composer chips: show more icons, or icons with text, when the composer is wide (kayg wants to discuss, 2026-11-18) | To grill | Composer |

| 19 | Which Manager-only operation writes reserved `AI/AGENTS.md`? Generic kinds cannot claim or write it | Narrow space-settings operation, or a general protected space-file writer; costs in morning-decisions.md | Agents |
| 28 | The first-visit "Choose a look" chooser: now a centred dialog. The record only says "a short gallery, then straight in" | Keep the centred dialog | Onboarding |
| 29 | On a phone, switching apps takes three taps and the app name does not look tappable: a capsule under the card as on desktop, or the pill in the sidebar footer | See both on a phone first | Phone shell |
| 30 | Should the composer double as the command palette, or stay separate from Cmd-K | Stay separate | Composer |
| 31 | Thread rows: on hover the time and state mark give way to the three actions; swipe right shows only Copy link; on touch the long-press menu is dropped for "More" in the swipe tray (iOS starts a drag on a long press) | Keep all three; try on a phone | Threads |
| 32 | The page waits for its font before painting. Painting first would be earlier but the text would visibly swap | Keep waiting for the font | First paint |
| 33 | Plugin workers in containers: where a space's workers run and how capacity is shared (at most eight workers per machine today), what happens on a server with no container runtime, brokered network fetch, and proof of teardown when a machine is lost | To grill | Plugins |
| 34 | Requests, left open by the lane: how a rotated token reaches the agent when a single-use approval is spent; whether an unused single-use approval belongs to the turn or the thread; how the limits (32 per thread, 256 per account) are divided; who may read `AI/AGENTS.md` directly | To grill | Requests |
| 35 | People: can a group hold a server role; what happens when a space's only Owner is disabled | Port the existing app's answer where it has one | People |
| 36 | Notes: paging of long lists, purging the whole Trash at once, reusing a file name after delete, renames that touch many links | To grill | Notes |
| 37 | Readiness: should one refresh probe every machine in the pool, and should provider sign-in be re-checked while a machine sits idle (today: cached five minutes, refreshed on demand) | On demand only | Agents |
| 38 | The bottom band and edge kayg saw beside the right sidebar could not be reproduced in a real GPU Chrome at 1500x970: which window size and zoom, and does it still show on the current preview | kayg to look again | Shell |
| 39 | With many spaces the dots overflow the sidebar footer: at 50 spaces about 7 show and the rest are reachable only through the palette. Options and screenshots on issue #15 | To grill with the dots design | Shell |
| 40 | Performance decisions left by the fix lane: how to shorten security-log replay at start (1,000 spaces still start in 320 to 860 ms against 300 ms), whether to keep an inherited root-layout check, how to close the cold save tail (one arm at 49.9 ms against 40 ms), how to cut the draft-save tail; and what the 10 ms "write reaches a subscribed client" ceiling measures, since it includes the durable save itself | See the lane report; decide the measure first | Performance gate |
| 42 | A tool step's output: the saved thread file keeps only its size, so a reloaded thread shows "N bytes of output". Where long output lives (its address, how the thread links to it, how long it is kept) is undecided | To grill with long-output storage | Threads |

## Design rounds

- **In progress: Settings and Appearance**: first eleven answers recorded in `settings.md`; second batch asked. Appearance (wordmark and its font, fonts, wallpaper, accent, themes and presets), agents (retry and snooze, streaming, default permission mode), shortcuts, machines, plugins, skills, automations.
- Onboarding for developers versus everyone else: what changes.
- Skills and automations management screens.
- The app shell on narrow screens, and the app switcher's exact form.

## Queued, no decision needed

- Rewrite git history to drop old images at the next quiet point.
- Try phone navigation (swipe back, dragging the card) on a real phone: it is merged but not counted as done until checked on a device.

## Recently decided (2026-11-17), now in the decision records

- The composer and palette: see `composer.md`.
- The composer's 8 ms budget applies to main-thread work per key, with zero missed frames required (2026-11-18).
- An approval for an action that cannot be undone, or that widens access, is always single-use (2026-11-18).
- Deleting an account that is a shared space's only Owner is refused until ownership is transferred (2026-11-18).
- Erasure reaches machines by a durable erase instruction on next connect; an administrator deleting another account needs a fresh passkey check; a deleted person's invites in shared spaces are revoked and stay listed (2026-11-18).
- Default wallpapers are fetched from Unsplash on demand with an API key kayg will provide; hosting them ourselves is to be grilled later, since photos can disappear (2026-11-18).
- Clear-to-Tinted is a stepped slider (2026-11-18).
- The Members screen can give a role in a space to a group as well as a person (2026-11-18).
- The record that a deleted owner's secret key was erased is kept permanently (owner sign-off, 2026-11-18).
- Sidebars are resizable by a handle at the vertical centre beside the middle card, shown on hover; wide mode is the default on desktop (2026-11-18).
- Agent sandbox home: one per space per person, with the provider sign-in shared; a thread reaches another space's thread only through the `calternal` command and its permissions (2026-11-18).
- Spaces in the shell: the switcher is a row of dots at the bottom centre of the left sidebar, as Aside's profile switcher; the sidebar shows only the current space (the palette still searches all); Settings gets a third section for the current space; everyone gets a personal space at sign-up (2026-11-18).
- Native apps: SF Symbols for icons; the wordmark sits under the traffic lights on a Mac (2026-11-18).
- Backups are external for now (a PBS job for the production VM); the app has no retention setting (2026-11-18).
- A space's standing instructions for agents live in `AI/AGENTS.md` (the familiar name); only Managers and the Owner may edit it (2026-11-18).
- The thread token stays in the agent's environment for now, redacted by the server; a private channel only the `calternal` command reads comes later (2026-11-18).
- Approval lifetimes: build "once" and "this thread" with thread storage; "always in this space" later (2026-11-18).
- Text behaves the same in every field: one implementation, the block editor extending a base text field, never two kept in step (2026-11-18).
- Performance: fix startup, save and the account feed first, then adopt measured values as limits for paths the existing app never limited. Aim to beat the inherited budgets, not just meet them (2026-11-18).
- A permission mode picked in the composer applies directly to the next turn, wider or narrower; no extra passkey check (2026-11-18).
- Live editing of notes: port the existing app's live rooms, rewritten for the plugin architecture (2026-11-18).
- The documentation site is built and on the preview at `/docs/`.
- Provider logins: each person signs in to the official programs inside their own sandbox; the server never stores a provider login.
- State lives on the server; clients subscribe to it and cache what they need, as the existing app does. There is no sync protocol to design. A client whose bookmark is too old reloads.
- Agent questions and approvals share the sticky bar with secret requests and do not expire.
- Warm pool of pre-started sandboxes on each cloud machine: yes.
- Offline: a sent message is queued as a visible pending submission and goes out on reconnect.
- Work never moves between machines by itself; an interrupted turn is resumed on request.
- The orchestrator round is held: see `orchestrator-proposal.md`, "Decided in the grill".
- Drafts sync across a person's devices and stay until sent or erased.
- After sign-out, unsent edits are kept as drafts. After revocation, the device offers an export of what it holds and then deletes it; revocation itself is immediate and never waits for that.
- A plugin's secrets belong to the space that enabled it. Plugins declare the secrets they need in their manifest.
- A secret request shows in a sticky bar above the composer and expires after 15 minutes; it can be asked again at any time.
- Secrets: one key file per owner in appdata; provider logins on a person's own machines stay on the machine.
- Request IDs are unique per account and space. The root guard and recovery log are wired into the server.

- Thread deletion removes all saved account modes; the 1,024-per-account bound remains (threads lane #28).
