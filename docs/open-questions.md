# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-18 02:25 IST.

## Waiting now

| # | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| 5 | Readability floor for surfaces over a wallpaper (provisional, built so kayg can see it): primary text held at 7:1, lesser text nudged toward it, and a surface may flip its text between light and dark as Craft's header does. Leaves 21 to 49% of the wallpaper visible on sidebars | kayg wants to see it: a comparison is being built on the preview | Materials |
| 18 | Saved thread modes are capped at 1024 per account and nothing deletes them yet, so thread 1025 would fail | Delete with the thread; raise no cap (the threads lane, #28, is building it this way) | Orchestrator step 2 |
| 21 | Light mode over a photo: derive a lighter photo and keep showing a lot of it (built), or do as Craft does and put a nearly opaque light surface over any photo (92% white, about 8% of the photo shows), which works with every photo but shows little of it | See both on the preview, then choose | Light theme |
| 22 | Members screen: can a group hold a role in a space ("Family: Editor"), or only people one by one at first | Groups too | Members |
| 23 | The record that a deleted owner's secret key was erased: backups are external (PBS), so the app cannot know when the last old backup is gone. Keep the record permanently (32 bytes per deleted owner)? Needs the owner's sign-off because it is kept forever | Keep permanently | Secrets |
| 10 | Native font: decide after seeing both on a Mac and an iPhone (deferred by kayg) | Build a switch in the native preview | Native apps |

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
