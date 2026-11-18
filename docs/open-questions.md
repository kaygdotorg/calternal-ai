# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-18 11:15 IST.

## Waiting now

| # | Question | Recommendation | Area |
| --- | --- | --- | --- |
| 25 | Composer chips: what each chip shows at each composer width (icon only, icon and text), and which chips exist | In the grill now | Composer |
| 24 | Materials: how far the middle card separates from the sidebars, the readability floor for text over a wallpaper, and how much photo shows in light mode. Rebuilt from the calternal-ui plugin's defaults with a five-step Clear-to-Tinted slider | Review on the preview | Appearance |
| 30 | Should the composer double as the command palette, or stay separate from Cmd-K? | Stay separate | Composer |
| 31 | Thread rows: hover hides the time and state mark for the three actions; swipe right shows only Copy link; touch has no long-press menu, "More" sits in the swipe tray | Keep all three; try on a phone | Threads |
| 42 | Long tool output: the thread file keeps only its size. Where the output lives, how the thread links to it, how long it is kept | To grill | Threads |
| 29 | Phone: switching apps takes three taps and the app name does not look tappable | See both forms on a phone | Phone shell |
| 39 | Many spaces: the dots overflow the sidebar footer (about 7 of 50 show; the rest only through the palette) | To grill | Shell |
| 28 | The first-visit "Choose a look" chooser is a centred dialog; the record only says "a short gallery, then straight in" | Keep the dialog | Onboarding |
| 32 | The page waits for its font before painting; painting first is earlier but the text visibly swaps | Keep waiting | First paint |
| 33 | Plugin workers: where a space's workers run, sharing capacity (eight per machine today), a server with no container runtime, brokered network fetch, a lost machine | To grill | Plugins |
| 34 | Requests: how a rotated token reaches the agent after a single-use approval; whether an unused approval belongs to the turn or the thread; dividing the limits (32 per thread, 256 per account); who may read `AI/AGENTS.md` | To grill | Requests |
| 37 | Readiness: probe every pool machine on a refresh? Re-check provider sign-in while a machine idles? (Today: cached five minutes, refreshed on demand) | On demand only | Agents |
| 35 | People: can a group hold a server role; what happens when a space's only Owner is disabled | Port the existing app's answer | People |
| 36 | Notes: purging the whole Trash at once, reusing a file name after delete, renames that touch many links | To grill | Notes |
| 40 | Performance: shortening the security-log replay at start (1,000 spaces: 320 to 860 ms against 300 ms), the cold save tail, the draft-save tail, and what the 10 ms "write reaches a subscribed client" ceiling measures | Decide the measure first | Performance |
| 10 | Native font: system or Google Sans Flex | Decide after seeing both on a Mac and an iPhone | Native apps |

## Design rounds

- **In progress: Settings and Appearance**: first eleven answers recorded in `settings.md`; second batch asked. Appearance (wordmark and its font, fonts, wallpaper, accent, themes and presets), agents (retry and snooze, streaming, default permission mode), shortcuts, machines, plugins, skills, automations.
- **Machines and sandboxes from the UI** (kayg, 2026-11-18): the whole flow without a command line: enrolling a machine, making its sandbox, signing in to a provider inside it; every instance must have machines attached.
- **Plugin-delivered interface**: how a plugin ships real UI, so the orchestrator can be split (shell, composer, a thread rail as `calternal-orchestrator-rail`).
- **Upgrades with real data**: there is no migration code by rule; what an upgrade across a schema change does once calternal.cloud holds data.
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

- Thread deletion removes its saved permission mode; the 1,024-per-account bound remains.
- `AI/AGENTS.md` is written by one narrow operation, Managers and the Owner only.
