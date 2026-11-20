# Threads as several plugins · proposal · 2026-11-20

Lane issue: [#102](https://git.kayg.org/kayg/calternal-ai/issues/102).
Owner instruction (2026-11-20): build Threads from parts that a plugin can
replace one at a time (for example `calternal-threads-people`,
`calternal-threads-composer`, `calternal-threads-ai`), the way bb builds its
thread app from several plugins.

This is a study and a proposal. No code changes. The contract read is
revision 14.44 (`origin/lane/v2-host-base`, `3e62ca07`). The Threads code read
is `plugins/calternal-threads` and `plugins/calternal-threads-claude-code` on
this branch.

## Summary

- bb's core owns the data (threads, messages, queue, providers' runs) and the
  window. Everything a person sees is a **slot**. A few slots are
  **exclusive** (one plugin draws it: the composer, the thread list, the
  sidebar navigation, the code and diff renderers). The rest are **additive**
  (many plugins add rows, actions, tabs, mentions or banners, placed by group
  and order). bb's own composer, thread list and navigation are ordinary
  bundled plugins that use the same slots.
- Threads can be split the same way into eight parts, one of them
  optional. One part, the
  **core** (`calternal-threads`), must stay whole. It owns the thread file
  format and is the only writer of thread files. The other parts read through
  the host and change a thread only by calling operations that the core
  publishes.
- The cost is processes. In Calternal every plugin with a worker is its own
  sandboxed process. A process costs about 25 MiB in an awake space (derived
  from the wall measurements below, not measured per process). bb pays almost
  nothing per plugin because its plugins run inside its own server. Hence the
  main recommendation: parts that only describe interface (list, composer,
  people panel) should need **no process at all**. 14.44 cannot express that
  yet.
- 14.44 already has the right foundation: extension points declared by an
  app, contributions from other packages, five contribution kinds, extension
  choices, and host-enforced consent and people lookup. Twelve gaps are listed
  below. They go to #78 only after the owner agrees to the split.

## 1. How bb does it

Sources: the bundled plugin manifests under bb's install directory, the
`bb-plugin-authoring` skill references, the `plugin-api-docs` plugin, and
`bb guide plugins`. I read none of the files under `~/.bb` that hold
secrets or sign-ins.

### What bb's core owns

| Core owns | Examples |
| --- | --- |
| Data and its lifecycle | projects, threads, messages, the queue, turns, environments, machines |
| The single message checkpoint | every message passes through the `message.dispatch` hook once per attempt |
| The window frame | layout, panels, the tab strip, persistence of tab order and choices, crash containment per slot |
| Choice and order of contributions | exclusive-slot picks under Settings → Appearance, deterministic ordering of additive slots |
| Plugin lifecycle | install, enable, disable, reload, safe mode, per-plugin storage |

bb's core does **not** own the composer, the thread list, or the sidebar
navigation. Each of these is a bundled plugin (`composer`, `thread-list`,
`navigation`) built only on the public slot, so each one is also a working
example to fork.

### Kinds of contribution

| Kind | What it is | bb examples |
| --- | --- | --- |
| Exclusive surface | one plugin draws the whole thing | composer box, thread list, sidebar navigation, source and diff renderers |
| Additive slot | many plugins add parts, placed by group and order | thread row metadata, row status, thread actions, Info rows, context card, message actions, composer actions and + menu, banners, mentions, highlighting, side-panel tabs, home sections, palette actions |
| Matched renderer | claims a pattern inside content | message directives, code-fence languages, file openers by extension |
| Provider | a backend that core drives | agent providers (`provider-claude-code`, `provider-codex`, `provider-pi`, `provider-acp`), environment providers (`environment-git-worktree`, …), machine providers |
| Hook | a question core asks and obeys | `message.dispatch`: proceed, wait (queue with a reason) or reject; `concurrency-limit` and `scheduled-send` use it |
| Event | an announcement after the fact; cannot veto | `thread.idle`, `message.queued`; `push-notifications` and `provider-retry` listen |
| Agent tools, skills, instructions | capabilities given to agent runs | `ask-user-question`, `workflows`, `honcho` |
| CLI command | one top-level `bb` subcommand per plugin | `bb git`, `bb automation` |
| Cross-plugin call | one plugin calls another's RPC; the callee sees the caller's plugin ID | separate plugins that share a feature, such as a bots plugin with a separate sidebar plugin |
| Per-thread namespace | each plugin keeps up to 256 KiB of JSON on a thread | the thread stays core's; plugin data sits beside it |

`drafts`, `scheduled-send` and `side-chat` are good examples of small parts.
Each adds one thing to the composer or the thread without replacing it.

### How a part is chosen, ordered, replaced, disabled, or missing

| Situation | bb's rule |
| --- | --- |
| Several plugins fill an exclusive slot | a plugin the user installed wins over a bundled one, then the first in a fixed slot order; the person can pin one under Settings → Appearance, per client |
| Order in an additive slot | by `group`, then `order`, then registration order; bb's own entries come first on a tie |
| Two plugins claim the same renderer pattern | bb's own renderer is used and a warning is logged |
| A plugin is disabled | its contributions disappear; for an exclusive slot, the next eligible one (or bb's default) takes over |
| A part crashes | only that slot is replaced: a crash notice, a "stopped working" placeholder with Reload, or bb's own renderer |
| An exclusive part is missing | it depends on the slot: renderers fall back to bb's own; the thread list shows a placeholder that links to Plugins; with no composer plugin enabled, **no composer is shown** |
| A replacement wants the default for some cases | the slot passes `Original` (the default bound to this call), so a replacement can delegate without replacing everything |

### What carries over, and what does not

Carries over:

1. The app is a small core plus slots. The default parts are ordinary
   plugins on the same slots.
2. Every slot states whether it is exclusive or additive, and the host owns
   choosing, ordering, and falling back.
3. The core keeps the data and one checkpoint. Parts add to the data
   beside it, in their own namespace, and never rewrite the core's records.
4. A replacement can delegate to the default (`Original`).

Does not carry over:

- bb plugins are full-trust code in bb's server and window. A part costs
  almost nothing at run time, so bb can have fifty of them. In Calternal a
  plugin with a worker is a sandboxed process with its own permissions and
  accounting (`plugin-api-v2.md` §2). Parts cost memory and wake time.
- bb parts draw with React. Calternal parts describe interface, and the
  renderer draws it natively. A Calternal composer replacement is therefore
  a description (fields, prompts, actions), not code. This makes the
  composer cheaper to replace than in bb.
- A missing composer in bb means no composer. Calternal should never do
  that (see question 3).

## 2. The proposed split

```
                      ┌──────────────────────────────────────────────┐
                      │ calternal-threads  (core, one writer)        │
                      │ thread kind + calternal-thread/v2 format     │
                      │ create · append · edit · delete · quote      │
                      │ send / queue / run job · alarms · snapshots  │
                      │ app shell with the extension points below    │
                      └──────────────┬───────────────────────────────┘
       points declared by the core   │   operations the core publishes
  ┌───────────┬───────────┬──────────┼──────────┬───────────┬──────────┐
  list        composer    people     ai         harnesses   notify     search
  (one)       (one)       (one)      (one)      (many)      (one)      (one)
  sidebar,    form,       panel,     call-in,   Claude      what       ranking
  rows        actions     invites    planner    Code, …     rings      and index
```

"(one)" means exclusive: one contribution is active and the person or
space picks it. "(many)" means additive: all enabled contributions appear,
in order. Message actions and row fields are additive points of their own,
listed under the parts that use them.

### Parts

| Part (package) | Owns | Point it fills | Needs from others | A third party writes |
| --- | --- | --- | --- | --- |
| **Core** `calternal-threads` | the `threads` kind, the `calternal-thread/v2` format, every write to a thread file, the conversation view snapshots, the send/run job, alarms, per-person state (read position, mute, pin) | declares all the points | the host only | not replaceable as a part; someone who wants a different file format writes a whole app (owner decision: one Threads list) |
| **List** `calternal-threads-list` | the sidebar: sections (Groups, People, Cals, Harnesses, Archived, folders), pinned strip, row template | `list` (one); reads row fields from the `row-fields` point (many) | host Binds over the `threads` kind and its indexed fields | a sidebar description: sections, Binds, row template; no worker |
| **Composer** `calternal-threads-composer` | the composer form: text field, draft slot, attachments, mention trigger, agent and machine choices, Send / Queue / Schedule | `composer` (one); hosts `composer-actions` (many) | the core's `threads.send` and `threads.arm`; host mentions, drafts and uploads | a form description that submits the core's send operation; a worker only if it transforms text (for example a rewrite button) |
| **People** `calternal-threads-people` | the participants panel, adding and removing people, invite links, how a group is shown, "read-only because someone left" | `people` panel (one); `message-actions` (many) for "reply privately" and the like | host group-scoped lookup (`group_people_scope`); the core's participant operation | a panel description plus actions that call the core's participant operation; usually no worker |
| **AI** `calternal-threads-ai` | calling an agent in: which agent, what it may read (one message, a range, the whole thread), the prompt sent to the harness, consultations between agents, queued follow-ups policy | `agent-planner` (one), called by the core's run job | harness contributions (through the core's job); host consent (`attempt_disclosure`) | a planner operation: input = selected messages and request, output = harness input or a consultation plan; needs a worker |
| **Harnesses** `calternal-threads-claude-code`, `-codex`, community | running one agent program; its own working log | `harnesses` (many, picked per send) — **exists in 14.44** | the core's run job calls it; the core writes the reply | exactly what `calternal-threads-claude-code` writes today |
| **Notify** `calternal-threads-notify` | which events ring, badge or stay silent (muted chats never ring; mentions show a count bubble) | `notify-policy` (one) | host push delivery (does not exist yet) | a policy operation; a worker. **Later**: the host has no notification contract |
| **Search** (optional, none by default) | a different ranking or index for threads | `search` (one) | the host's default ranking always works without it (owner decision) | a search profile with its own index job |

Consent is **not** a part. 14.44 makes consent a host rule
(`attempt_disclosure`): the host omits material from authors without a
live grant, whatever a plugin says. The AI part chooses what to ask for.
The host decides what is disclosed. A replacement AI part cannot weaken
consent. The same holds for finding people (`group_people_scope`).

The conversation view and the entries stay in the core. The view is a
snapshot that only a write-authorized run of the file's owner may publish.
A second plugin drawing messages would need read access to the file and
publish rights, which is the same as being the core. What others can add to
the view is **entry actions** (react, forward, pin a message) and, later,
**entry parts** (a contributed block inside one message, like bb's message
directives). Both are additive points.

### What must stay in one place, and why

| Stays in the core | Why |
| --- | --- |
| The file format and its version | one format means every reader, editor and export agrees; Notes can open thread files through the existing `editor` point |
| Every write to a thread file | the host checks one writer per kind; consent, quote redaction, delete-for-everyone and restart-safe appends are checked once, in one place |
| The run job that calls harnesses | 14.44 lets only the item's owner write the reply under the exact attempt fence; a part outside the core cannot append a reply |
| Per-person state (read position, draft, mute, pin) | host viewer state, declared by the kind's owner |
| Snapshots for a sleeping view | only the writer may publish them |

A part keeps its own data in its own hidden items, linked by message
anchor, as a harness keeps its working log. It never stores data inside the
thread file. Reactions, for example, would live in the reactions plugin's
own file and be shown through an entry part.

## 3. Cost

### How many plugins a fresh space installs

| Arrangement | Packages | Processes for Threads |
| --- | --- | --- |
| Today | 2 (core, Claude Code harness) | 2 |
| Every part its own package, every package with a worker | 7 (core, list, composer, people, AI, harness, notify) | 7 |
| Every part its own package, description-only parts with no worker (recommended; needs gap G5) | 7 | 4 (core, AI, harness, notify) |
| Default parts inside the core as its own contributions | 2 | 2 |

The harness's agent runs in the agent VM, not the plugin VM. The table
counts its plugin-side process, if it has one.

### Memory and wake

From [`docs/measurements/space-runtime-walls.md`](https://git.kayg.org/kayg/calternal-ai/src/branch/dev/docs/measurements/space-runtime-walls.md)
on `dev` (perf VMs, under the perf lock):

- A sandboxed process tree with five Node workers measured **127 MiB** PSS
  awake (arm B). That is about **25 MiB per worker**. This is derived by
  division, not measured per process.
- A resident space was about **130 MiB**. A fresh-worker wake from disk took
  **1,607 / 2,124 ms** p50 / p95 on perf-02. A stopped process tree started
  in about **350 ms** cold (arm B).
- Plugin VMs have two vCPUs. More processes starting at once compete for
  them. How wake time grows with the number of workers is **not measured**.

| Arrangement | Extra awake memory per space, Threads only | Wake effect |
| --- | --- | --- |
| 7 workers | about +125 MiB over today | more processes to start or restore at each wake; unmeasured |
| 4 workers (recommended) | about +50 MiB | AI and notify can start only when called, if the host starts processes lazily (gap G11) |
| 2 workers | none | none |

At 100 resident spaces, +125 MiB per space is about 12 GiB, more than a
whole perf VM. +50 MiB is about 5 GiB. These are rough estimates.

### Can parts share one runtime?

Not across packages. §2 gives each enabled plugin its own process, with its
own OS permissions, resource accounting and crash containment. If several
packages shared one process, one could read another's memory and spend its
quota. If only first-party packages could share, that would be a privilege
for our own plugins. There are two fair ways to save the cost instead:

1. **Description-only contributions** (gap G5). The list, composer and
   people panel are interface descriptions that the host checks at install.
   They call the core's operations. They need no process. Any publisher gets
   the same.
2. **One package, several parts.** A package may fill its own points. The
   core could carry default parts as its own contributions, and a third party
   replaces one by contributing to the same point. This costs nothing, but
   the default parts are no longer packages a person can remove or update
   on their own.

## 4. Mapping onto 14.44

### What 14.44 already expresses

| Need | 14.44 |
| --- | --- |
| An app declares typed, versioned points; others contribute | `extension_points` in the app manifest; `contributions` in the contributor's manifest; exact version match; paired schemas compared as parsed JSON |
| Harnesses | `harness` kind, `HarnessRef`, `extension.choices`, `contribution_call` / `contribution_read`, owner writes the reply under the attempt fence |
| Panels | `panel` kind; `Panel.extension` in the description |
| Row fields | `row_field` kind; `RowTemplate.extensionFields`; basic row without a wake, plugin fields for visible rows only, at most 8 per row |
| Actions | `action` kind; `Invoke.extension` |
| Opening thread files in another app | `editor` kind with formats; `format_open`; person's explicit pick without changing the default |
| A person picks one of several | choice prompt with `options: {extension: <point>}` |
| No direct plugin-to-plugin connection, no implicit grants | `extension_points.authority` |
| Consent and people lookup enforced by the host | `attempt_disclosure`, `group_people_scope` |
| Mentions, drafts, uploads in the composer | `entry_composer.*`, `attachment.*` routes |
| Search ranking by a plugin | `search.plugin`, but only profiles declared by the kind's owner |

### Gaps (one line each, for #78 after the owner agrees)

- **G1 Cardinality.** A point declares whether it takes one active contribution or many.
- **G2 Selection.** For a one-of point: who picks (space default, a person's override), where the pick is stored, and disabling per space.
- **G3 Order.** Additive contributions carry a group and an order; the host sorts them deterministically, the owning app first on a tie.
- **G4 Fallback.** The owning app's own description fills a point when no eligible contribution is installed, enabled or healthy; a crash falls back to it.
- **G5 Description-only contribution.** A contribution that is a checked description fragment (sidebar sections, a composer form, a panel), with no operation and no process.
- **G6 Calling the owner.** A contribution's description or operation may invoke operations the owning app publishes on the point (the composer submits `threads.send`; the people panel calls the participant operation), with the caller's rights intersected.
- **G7 New point kinds.** `composer`, `list` (sidebar sections), `entry` (a block inside one message), and a generic `service` kind for calls without interface (AI planner, notification policy).
- **G8 Several calls per attempt.** A run may call a planner and then a harness, and a consultation calls two harnesses; 14.44 allows one contribution call per attempt.
- **G9 Contributed search.** Another package may contribute a search profile for the owner's kind.
- **G10 Notifications.** No host notification delivery and no point for an app's notification policy.
- **G11 Lazy start.** Wake starts only the processes a call needs, and the wall measurements state the cost per extra worker.
- **G12 Contributor data beside owner items.** A contributor's own hidden items keyed to the owner's message anchors, shown through an entry part (reactions, bookmarks).

## 5. Questions for the owner

### Q1. Should Threads' own default parts be separate packages?

- **What:** the list, composer, people, AI and notification parts could each
  be a separate package (`calternal-threads-people`, …), or they could live
  inside `calternal-threads` as contributions it makes to its own points.
- **Why now:** it decides how the next Threads code is cut and how many
  packages a fresh space installs.
- **Example:** someone writes a Telegram-style composer. In both options they
  install it and pick it for the `composer` point. The difference: with
  separate packages, a person can also remove or update our composer on its
  own.
- **Options and cost:**
  - (a) Every part its own package with a worker: 7 packages, about +125 MiB
    per awake space, slower wake (unmeasured). Works with 14.44 plus gaps
    G1–G4, G6 and G7.
  - (b) Separate packages, but the list, composer and people parts are
    description-only: 7 packages, about +50 MiB. Needs G5 as well.
  - (c) Default parts inside the core: 2 packages, no extra cost. The parts
    are still replaceable. Our defaults are not removable on their own.
- **Recommendation: (b).** It matches your instruction and the rule that
  first-party plugins are not special, at a cost the walls can carry. Until
  G5 exists, build the parts as separate modules inside one package with the
  same boundaries (see Q4).

### Q2. Who picks the replacement: the space or each person?

- **What:** when two composers (or lists, or AI parts) are installed, one
  must be active.
- **Why now:** it decides where the pick is stored and how gap G2 is worded.
- **Example:** Alice installs a third-party composer in a space she shares
  with Bob. Does Bob get it?
- **Options and cost:**
  - (a) The space Owner picks for everyone. Simple; no personal taste.
  - (b) Each person picks among installed parts (bb does this, per device).
    The pick follows the person across devices.
  - (c) Split by kind: parts that only change what the person sees (list,
    composer) are per person; parts that change what others receive (AI,
    notifications) are per space.
- **Recommendation: (c).** A person's composer affects only them. An AI part
  decides what an agent reads and says in everyone's thread, so it belongs to
  the space. The Owner still decides what is installed.

### Q3. What happens when a part is missing or crashes?

- **What:** a picked part can be disabled, removed, failing, or asleep.
- **Why now:** it decides whether the core ships its own plain version of
  each exclusive part (gap G4).
- **Example:** the third-party composer fails its install check after an
  update. In bb, with no composer plugin there is no composer.
- **Options and cost:**
  - (a) The core carries a plain fallback description for each one-of point
    (a simple list, a multiline composer, a plain people panel). A few dozen
    lines of description in the core, no server code.
  - (b) Show a placeholder that links to Plugins. Less in the core; Threads
    can become unusable.
- **Recommendation: (a).** Threads must never lose Send. The fallback lives in
  the app, not the server, so the server stays generic.

### Q4. Do we split the code now or after the contract gaps are closed?

- **What:** today's `calternal-threads` has people, composer, format and run
  logic in one worker.
- **Why now:** the Threads lane is writing that code this week.
- **Example:** moving people handling (`people.ts`) to its own package now
  needs G6 (the people part calling the core's participant operation), which
  14.44 lacks.
- **Options and cost:**
  - (a) Split into packages now, using fixtures for the missing host parts.
    More fixture code, which is thrown away later.
  - (b) Keep one package now, with internal modules on the proposed
    boundaries (core, list, composer, people, AI) and interfaces written as
    point schemas. Move each module out when its gaps are in the contract.
    No throwaway code; the split happens later.
- **Recommendation: (b).** It follows the existing rule that missing host
  support keeps a feature disabled, and the boundaries are tested before the
  packages exist.

## Simplification opportunities

- Consent and people lookup need no plugin part. They are host rules
  already. Keeping them out of the split removes two candidate packages.
- The conversation view should not be a part. It removes a cross-plugin
  publish path, and the only reason to replace it (a different look) is
  covered by the renderer and entry parts.
- `plugins/` has two names per agent program: the machine-side adapters
  `calternal-claude-code` and `calternal-codex` (used by today's app and the
  v1 `calternal-orchestrator` manifest) and the new harness package
  `calternal-threads-claude-code`. At cutover, one package per agent program
  should carry both the harness contribution and its adapter, and the v1
  orchestrator manifest goes (the word "orchestrator" is retired).
- `contribution_calls_per_view: 2` and "one contribution per attempt" are two
  separate bounds on the same idea. A single per-point call budget, declared
  by the point, would cover the harness, the planner and consultations.
