# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-18 00:35 IST.

## Waiting now

| # | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| 5 | Readability floor for surfaces over a wallpaper (provisional, built so kayg can see it): primary text held at 7:1, lesser text nudged toward it, and a surface may flip its text between light and dark as Craft's header does. Leaves 21 to 49% of the wallpaper visible on sidebars | Keep the provisional choice unless it looks wrong on the preview | Materials |
| 6 | Light renditions of the eight default wallpapers (they suit dark mode): derive a lighter version automatically (provisional), or kayg supplies light ones | Automatic for now | Light theme |
| 7 | Spaces in the shell: where the space switcher lives, what the current space limits, Settings groups, groups holding space roles, an automatic personal space (five questions asked 2026-11-17) | See chat; recommendations stand as provisional | Shell, Settings |
| 8 | One editor engine everywhere, or a native field for short text and the full editor only for notes? The lane built the second: every inline field shares the editor's transaction layer but short fields stay native, which keeps first paint light (2 KB against the full editor's size). kayg said every inline field should be the block editor | Keep the split unless kayg wants identical behaviour in short fields | Editor |
| 9 | Live editing of a note needs the server to accept small text changes, not whole-note replacement. Port the existing app's live rooms (a shared document per open note), or a simpler patch operation? | Port the existing app's rooms | Editor stage 3 |
| 10 | Native apps: Google Sans Flex (shipped with the app) or the system font? The web rule is no system font; the native study measured the system's own | Google Sans Flex, to match the web and your brand | Native apps |
| 11 | Native icons: our own icon set drawn from the web's paths, or SF Symbols? | Our own set, for one look everywhere | Native apps |
| 12 | On a Mac the wordmark is tight beside the window's traffic lights: widen the sidebar, put the wordmark under the lights, or hide that row | Under the lights | Native shell |
| 13 | How long to keep the record that an owner's secret key was erased. It depends on how long backups are kept, which is undecided | Keep while any backup could predate the erasure; needs a backup retention period first | Backups |
| 14 | An agent's sandbox home is shared by all of a person's threads, so a Least-access thread in one space can read the provider's transcripts from another space. Accept (it is the person's own data), a home per space with only the provider login shared, or a provider folder per thread (loses resuming across threads) | A home per space, login shared read-only | Provider merge follow-up |
| 15 | The thread token sits in the agent's environment. The server will redact it from everything it publishes, but a tool could still write it to a file or send it out. Keep it there, or hand it over a per-turn socket only the `calternal` CLI reads | Socket, as a follow-up; redaction now | Agents |
| 16 | The composer asks for a wider permission mode than the thread's saved one: apply it, or require the separate mode change with a fresh passkey check (today it is ignored) | Require the mode change | Agents |
| 17 | Grant lifetimes "once", "this thread" and "always in this space" are not built; every grant lasts the current turn | Build "once" and "this thread" with thread storage; "always" later | Requests UI |
| 18 | Saved thread modes are capped at 1024 per account and nothing deletes them yet, so thread 1025 would fail | Delete with the thread once thread storage exists; raise no cap | Orchestrator step 2 |

## Design rounds

- **In progress: Settings and Appearance**: first eleven answers recorded in `settings.md`; second batch asked. Appearance (wordmark and its font, fonts, wallpaper, accent, themes and presets), agents (retry and snooze, streaming, default permission mode), shortcuts, machines, plugins, skills, automations.
- Onboarding for developers versus everyone else: what changes.
- Skills and automations management screens.
- The app shell on narrow screens, and the app switcher's exact form.

## Queued, no decision needed

- Rewrite git history to drop old images at the next quiet point.
- A documentation site generated from the code (`/docs/user`, `/docs/admin`, `/docs/dev`, `llms.txt`, Markdown for every page).

## Recently decided (2026-11-17), now in the decision records

- The composer and palette: see `composer.md`.

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
