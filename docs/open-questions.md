# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-18 23:30 IST.

## Waiting now

| # | Question | Recommendation | Area |
| --- | --- | --- | --- |
| 31 | Thread rows: hover hides the time and state mark for the three actions; swipe right shows only Copy link; touch has no long-press menu, "More" sits in the swipe tray | Keep all three; try on a phone | Threads |
| 35 | People: can a group hold a server role; what happens when a space's only Owner is disabled | Port the existing app's answer | People |
| 36 | Notes: purging the whole Trash at once, reusing a file name after delete, renames that touch many links | To grill | Notes |
| 40 | Performance: the slow start at 1,000 spaces (320 to 860 ms against 300 ms), the cold save tail and the draft-save tail. No profile yet says where the time goes | Profile first, then ask | Performance |
| 10 | Native font: system or Google Sans Flex | Decide after seeing both on a Mac and an iPhone | Native apps |

## Automatic thread title

Decided 2026-11-18: a person never signs in twice and no API key is used. The title turn runs on the person's existing provider login; the per-turn lane is finding how to run it with no tools, hooks or saved session, and proves it before any code.

## Design rounds

- **Settings and Appearance**: second batch answered 2026-11-18 (see Recently decided). Left: the readability floor and light mode on the preview (#24).
- **Machines without a terminal**, partly decided 2026-11-18 (below); still to grill: pre-configuring machines at deploy time, the `curl | sh` installer at calternal.com/install for both a server and a machine, the macOS app that bundles the CLI, how a machine behind NAT stays reachable for people who know nothing about networking, and how a group's allowance of pool compute is configured and shown.
- **Pricing**: plans are listed at calternal.com/pricing; storage is bought separately; what the sizes, hours and storage steps are.
- **Upgrades with real data**: decided 2026-11-18 that updates are automatic and a failed one rolls back without the person or admin noticing; the index rebuild is made faster first (search is unavailable for 67 to 80 s at 100,000 notes today), then migrations are written for the small core (accounts, spaces, roles, secrets), which cannot be rebuilt from files.
- **MicroVMs**: one per person with a container per thread; sizes decided; the lane posts its build plan on issue #51 for review.
- **Next to write up** (kayg, 2026-11-18): sharing (#52), then Settings and Appearance.
- **The canvas experiment** (kayg, 2026-11-18; still an experiment behind `?canvas=1`): no outer card; the area between the sidebars is the canvas and each window is a middle card, so one window looks exactly like the app today and a second animates as a split; structured columns as in niri; windows touch the top; one continuous veiled background behind them, as under the sidebars; windows opaque or transparent with no blur of their own; both sidebars follow the focused window's app; on a phone one window at a time. The app switcher is no longer part of the canvas (it moved to the left sidebar's footer). Feedback 2026-11-18 17:00, being fixed by the Threads lane: one window must match the app without the canvas exactly (fonts, spacing, animations); no fade on a half-scrolled window; windows are neutral dark or light, never tinted by the wallpaper's accent; no accent border on the selected window (inactive windows dim instead, to be judged); the fullscreen icon is two diagonal chevrons and the link icon becomes a real share menu.

## Queued, no decision needed

- A long thread: a thread file stops at 2 MiB today (`thread_full`), so a thread cannot hold 100,000 messages; how a thread continues past the limit needs a design.

- Rewrite git history to drop old images at the next quiet point.
- Try phone navigation (swipe back, dragging the card) on a real phone: it is merged but not counted as done until checked on a device.

## Recently decided (2026-11-17), now in the decision records

- Interface details (kayg, 2026-11-18 night): every segmented choice uses the existing app's Selector, ported (an outer pill with the selected item as a pill inside it), including Inbox and All Threads; no search button beside those tabs; the full-screen control is hidden when a window is already full; window width and look leave the three-dot menu and their defaults live in Settings, Appearance; Settings shows nested sections as a split inside the card when there is width (the same primitive a mail plugin would use); no setting appears twice.
- Production is not deployed again until data survives upgrades (kayg, 2026-11-18): migrations for everything that cannot be rebuilt, a version on every format, and a tested rollback come first; kayg signs up after that. The scaling measurement for message search (10 million, 100 million, 1 billion if the disk allows; many threads and few long ones) is accepted as enough.
- Materials (#24, 2026-11-18): the readability floor is 7:1 with text flipping light or dark; light mode uses the lighter copy of the photo; the temporary Compare section is removed.
- Draft saves are relaxed (2026-11-18): they no longer wait for the disk to confirm, so a power cut may lose the latest unsent draft edits; notes and sent messages keep the full guarantee.
- Thread rows are one line in the Inbox as in All Threads; the waiting reason is a mark at the row's end (2026-11-18).
- Canvas (2026-11-18): the right-sidebar toggle is an item inside the three-dot menu; windows are resized with the same handle as the sidebars.
- Settings round (2026-11-18): Settings is an app like the others, so a window in the canvas, with panes modelled on Xcode's settings; a theme is one readable file that can be exported, imported and shared, plugins can ship themes, and themes are tokens, never CSS; an Inbox thread can be snoozed until a chosen time, and a thread waiting for a subscription limit moves to a Snoozed section until the reset; a space's Owner installs plugins for their own space, from a gallery, an address, or built in the app; a machine can be renamed, paused, switched between sandboxed and direct, inspected and removed. Retry and shortcuts were already decided (`orchestrator-proposal.md`, `composer.md`).
- Editing a sent message (2026-11-18): the interaction on issue #33 is approved; the version on screen is per thread, the same for everyone.
- First-owner setup code (2026-11-18): a server command prints one when no owner exists yet; production is deployed fresh with that build and earlier test data and rollbacks are deleted (never the old app's data).
- Sharing (#52, 2026-11-18): the existing app's sharing decisions are ported (people, groups, links with optional expiry and no password, guests, the share sheet, Shared with me). New here: a single item can be shared by itself as well as through a space; notes, collections and threads can be shared, threads view-only; Editors share with people and groups, a public link needs a Manager or the Owner unless the space allows Editors; a public link has its own address (`/s/<secret>`) and may be served on a custom domain, which a later publish or blog plugin will use; Share opens the sheet with people, links, Copy link and the system share sheet, export stays under the three dots; things shared with you show as a section in each app's sidebar; Share on a canvas window shares the item in it.
- Fonts (2026-11-18): Google Sans Flex is the default for the interface and for chat, each separately configurable; Cascadia Code for monospace and a future terminal (a terminal would come from a plugin), replacing Maple Mono; fonts are configurable per role as in the calternal-ui plugin.
- Shell footer (2026-11-18 evening, replaces the space dots and the capsule under the card): spaces sit behind the account picture on desktop and phone; the app switcher is an icon-only capsule of three apps at the bottom centre of the left sidebar, scrolling sideways. This also settles where the switcher sits in the canvas.
- MicroVM sizes (2026-11-18): the smallest is 512 MiB with at most two active threads, then 1,024 MiB; measured on real turns (issue #51).
- Plugin network access is refused for private addresses by default, with an explicit grant approved by a server admin or the machine's owner; only cloud metadata addresses are always refused. Design to be written up.
- Plugin workers (#33, 2026-11-18): a shared space's workers run only on the operator's pool machines; with no machine able to run containers the plugin says it is unavailable and how to add a machine; revoking a machine frees its capacity, nothing is freed on a timeout; network access is declared in the manifest, approved by a Manager and fetched by the server on the plugin's behalf, with the design for broad needs (mail, money) still to be written up.
- Requests (#34, 2026-11-18): an unused single-use approval belongs to the turn; limits are 32 open requests per person per thread and 256 per account, standing grants counted separately; any member may read `AI/AGENTS.md`, Managers and the Owner edit it; a replaced token reaches the running agent through a protected local channel only the `calternal` command reads.
- Performance (2026-11-18): the 10 ms ceiling measures from the durable commit to the client receiving it, in release builds, as a hard gate; the full round trip gets its own limit.
- The look chooser is only a step in onboarding; the separate first-visit dialog goes (#28, 2026-11-18).
- Automatic titles run with no tools on the person's existing sign-in; kayg's Claude Code sign-in may be reused for tests as a reduced copy, never displayed.
- Long tool output is kept in full for the thread's life, in a hidden plain-text file beside the thread, and counts toward storage (#42, 2026-11-18; measured at about 225 KiB a coding session).
- 2026-11-18 evening: composer chips are always shown, also at their default (#25). The palette and the composer stay two surfaces of one engine (#30 closed; `composer.md`). Editing a sent message forks inside the same thread: the old tail is kept and shown there, never as a second thread in the list (#45). Many spaces: a horizontal strip showing three dots at once, scrolling sideways, as Aside (#39). The page waits for its font before painting (#32). Readiness is checked on demand only (#37). Wallpapers are downloaded from Unsplash on first use and served from our own copy, as the existing app does. The thread file's hidden comment line stays. Canvas: no fade on a half-scrolled window; the share menu holds the system share sheet and Copy link until the sharing round (#52).

- The composer and palette: see `composer.md`.
- Plugin interface (2026-11-18): stateful generated views: a plugin's interface code runs sandboxed and sends the host a tree of host components; sealed frames only for what the catalogue cannot express; official and community plugins are treated the same; the core (sign-in, approvals, secrets, recovery) cannot be replaced or styled. The canvas experiment is judged first, because the layout may become permanent. The largest gaps against bb, bb-lite and pi-durable get fixed in this order: live install and reload, plugin-owned slots, scoped reactive values, readiness and logs, durable waits; then another audit. Notes and Threads become ordinary plugins on public service calls. The acceptance test: a small model, given only the plugin guide, writes plugins and loads them live with no restart.
- Plans (2026-11-18): three sandbox sizes; an hour is sandbox running time; an idle sandbox stops after ten minutes and resumes on the next message; when hours run out new work queues and running turns finish; an invite carries the group; usage shows as a small meter in Settings and a line when close. Work on your own machine is unlimited and makes the product free, with a 1 GiB storage cap; more storage is bought separately.
- App switcher (2026-11-18): the capsule under the middle card, three apps as text only, horizontally scrollable, no list; the same bar is the phone's bottom bar for apps, with spaces behind the account picture.
- Onboarding (2026-11-18): one path; an "I write code" switch adds repositories, the CLI and direct mode.
- Skills and automations screens (2026-11-18): a list per space; creating either is hand-held; a skill follows the official Agent Skills standard; an automation's schedule is typed in natural language; each run is a thread.
- Plans (2026-11-18): a group's compute is a sandbox size (CPUs and RAM) plus hours per month.
- The re-authentication line reads "<Provider> needs you to sign in again" (2026-11-18).
- Threads sidebar (2026-11-18): a top section the person can edit (New Thread, Automations, Skills by default); no Needs-you card; two tabs, Inbox (threads waiting on the person) and All Threads.
- Onboarding (2026-11-18): stepped, in the style of the infraternal onboarding; includes the machine and the provider sign-in.
- Thread titles are generated automatically after about five messages by a cheap model on the person's own subscription (GPT-6 Luna medium fast with Codex; Haiku 5.5 low with Claude only).
- Thread files, revised 2026-11-18: one self-contained file (no hidden companion; IDs stay in the file in the least noisy form), frontmatter keys as the existing app names them, heading times in the writer's timezone with the UTC offset.
- Thread files (2026-11-18, first form): very readable, like the existing app's files: a `format:` tag in the frontmatter as the existing app does (`format: calternal-money`), a heading per message naming who or what wrote it (for an agent: harness, provider, model, reasoning level, time, duration), and machine state (IDs, provider session, counters) in a hidden companion file.
- Wallpapers reuse the Unsplash key already in the production secrets file (2026-11-18).
- Installer (2026-11-18): a hand-held, interactive, plain-text wizard (not a TUI) at calternal.com/install for both a server and a machine; it may install Podman and the microVM runtime with consent; the CLI comes first, the macOS app after; the server installer enrolls its own host as a machine by default.
- Machines and sandboxes (2026-11-18): the first owner's onboarding walks through adding a machine unless one was pre-configured at deploy; someone using only the hosted instance never sees machines, their group decides their compute; adding a machine works by an agent over SSH and also by a printed script to run on any Linux or macOS machine; a machine always connects out to the server; one standard sandbox for everybody, its image changeable only by server admins; own machines are sandboxed by default with Direct as an explicit choice.
- Provider sign-in (2026-11-18): done in the app, as items in the bar above the composer and an in-chat flow (a message with the link that turns into "Authentication done"); re-authentication is started automatically on expiry and, if it lapses, offered again when the person next starts typing ("these providers need signing in again", expandable); none of the flow is written to the thread file except one line that sign-in was attempted or done for a provider, with a time.
- calternal.cloud is replaced by this product (kayg's instruction, 2026-11-18); the existing app is stopped and its data kept.
- Composer chips (2026-11-18): Attach, Model, Permission, Runs on; the reasoning level lives in the model chip, which reads provider icon, model name, level; Runs on shows the machine name and a shield when sandboxed; no dots inside a chip. Chips drop to icon-only by the composer's width in the order Runs on, Permission, Model; on a phone all are icon-only except a non-default choice. Attachments sit in a sticky row above the text. Everywhere, rows of labelled controls go from icon and text to icon-only when width is short, through one shared primitive.
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
