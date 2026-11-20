# People, permissions, calls and the threat model (primitives 3 and 5)

Status: design section for the night of 2026-11-20/21, branch `lane/design-authority`.
Nothing here is decided unless it says "owner decided". It tests the thesis
(`docs/proposals/primitives-only-thesis-2026-11-21.md` on `dev`) and the contract owner's
draft (`docs/proposals/primitives-only-host-2026-11-21.md` on `lane/v2-host-base`,
called "the draft" below). Section 3.2 is the threat model for the whole design and is
the backbone for the security review that follows.

## 0. The section in twelve lines

1. Every call carries one **call context**: space, person, the chain of plugins it
   passed through, the run (if any), the trigger, and a request ID.
2. People, groups, spaces and roles stay as the existing app settled them (ADR 0058,
   0059, 0060). Per-file sharing is today's `item_grants`, opened to every file and folder.
3. A plugin's **grant** lists kinds, paths, calls, network, secrets, programs and
   triggers. The host writes the install screen from it; the plugin cannot word it.
4. One **call table** maps every name (host methods and every plugin's published
   operations) to a handler. Every hop through it is checked by one function and
   written to the audit trail.
5. Plugin A calling plugin B: A needs the right to call that operation; the person
   needs the rights the operation uses; B acts under its own grant; anything A names
   in the input must be something A could reach or was handed (the **designation
   rule**). This is the draft's model, made precise; the thesis's strict intersection
   is rejected (section 1.5, question 1).
6. The host does not know "agents". It knows **outbound-capable** principals: a
   program run, or a plugin whose grant includes network, secrets or programs. A call
   chain that contains one is **marked**; every read in a marked chain returns another
   person's records only with that person's grant (section 1.6).
7. Consent is a host-held grant per author and file, either for one run or standing,
   asked by a host-drawn prompt; one minute without an answer withholds that person's
   records (owner decided).
8. **Origin stamps** (person, plugin, run) make "own" a host check: a record is
   author-owned by default, so only its author can edit or tombstone it, whatever plugin asks.
9. Dependencies, slots and picks: a part is a plugin filling a named slot; the slot's
   effect decides whether the person or the space picks it.
10. Quotas for storage, compute, calls, wake-ups, runs, egress, audit and consent asks
    are host-enforced per space and per plugin.
11. Secrets are injected by the host into a declared destination; plugin code never
    holds the value. This does not hold for programs (harness logins, owner decided).
12. Security state (roles, shares, approvals, consent, picks) is host-held, not files:
    a correction to "userdata is strictly files" that needs the owner (question 3).

---

## 1. The primitives

### 1.1 Principals and the call context

| Principal | What it is | Authenticated by |
| --- | --- | --- |
| **Person** | An account (UUID). | Session from passkey or OIDC sign-in (`crates/auth`). |
| **Plugin** | An installed package at a pinned digest, enabled in one space. | The host started its worker; the worker's channel is bound to (space, plugin, digest). |
| **Run** | A work unit running a program (an agent harness or any long job) in a sandbox on the host or an enrolled machine (primitive 4). | A per-run credential bound to the machine enrollment and the work row. |
| **Machine** | A computer a person enrolled. Runs only that person's runs, or the space's runs if the owner bound it to the space. | Enrollment key (`crates/auth/src/machines`). |

A **call context** travels with every call and is never set by plugin input:

```text
ctx = { space, person | space-service, chain: [plugin, ...], run?, trigger, request_id, marked }
```

- `person` is the person who acted. For a time or change trigger it is the person who
  armed or subscribed, with rights re-read when it fires. `space-service` is the only
  person-less context (section 1.9).
- `chain` grows by one entry per plugin-to-plugin hop; at most **8** hops (a fixed
  security limit).
- `marked` is true when any principal in the chain is outbound-capable (section 1.6).

### 1.2 People, groups, spaces, roles, sharing (as they exist, kept)

These are the existing app's accepted decisions and today's `crates/auth`; this design
changes their reach, not their rules.

| Concept | Rule | Source |
| --- | --- | --- |
| Group | Named set of people; a person may be in several. You see yourself and members of your groups; nobody else. No server-wide discovery. | ADR 0060, owner 2026-11-20 |
| Invite | Single-use link, 7 days by default, carries groups and quota; members of a group may invite into it. | ADR 0059, owner 2026-11-20 |
| Space | UUID; owner is the space row; every group gets a shared space by default. | `schema.sql` lines 112-139, owner 2026-11-20 |
| Role | reader, editor, manager, owner, each a capability set; checks ask for a capability, never a role name. | ADR 0058, `crates/types/src/capability.rs` |
| Per-file share | A file or folder granted to a person or group as reader or editor; a share link is reader only. | `crates/auth/src/sharing.sql` (`item_grants`) |
| Group departure | People who no longer share any group: the shared file becomes read-only for them; history stays. | owner 2026-11-20 |

The rights function is one place: `rights(person, file) = max(space role capabilities,
shares on the file or any ancestor folder)`, then lowered to read-only by the
group-departure rule. Today's `item_grants` has `kind IN ('notes','collections')`; that
check goes, and a grant names a file ID or a folder ID.

Host methods in this area:

| Method | Input | Result | Failure | Cost (reads / flushes) |
| --- | --- | --- | --- | --- |
| `host.people.find` | text, max 20 | person refs with name, avatar | none visible: empty list | index lookup over the asker's groups; 0 / 0 |
| `host.people.get` | person ref | name, avatar, shared groups | not visible: same answer as unknown | 1 cached read / 0 |
| `host.share.add` | file or folder, person or group, reader or editor | share ID | no share right; target not visible; audience widening needs the host challenge | 1 / 1 (plus version bump) |
| `host.share.remove` | share ID | ok | not found | 1 / 1 |
| `host.share.list` | file or folder | shares the asker may see | no read | 1 / 0 |
| `host.share.link` | file, expiry | link (shown once, copyable again per ADR 0059 style encryption) | space forbids links | 1 / 1 |
| `host.members.*` | invite, change role, remove | as `crates/auth/src/spaces.rs` today | capability missing | as today |

Plugins call `people.*` only if their grant has `people: true` ("can see the people you
share a group with"). Plugins never call `members.*` or invite methods; those are drawn
by the shell (owner decision: authorization stays in the host).

### 1.3 The grant

A grant is part of the manifest. One declaration, from which the host derives the
install sentences, the call-table rights and the quota buckets (one place per fact).

| Field | Meaning | Install sentence (host-written) | Approved by |
| --- | --- | --- | --- |
| own kinds | Kinds the plugin declares | "Keeps *Reading List items* in this space." | implied |
| `kinds: {k: read \| write}` | Other plugins' kinds | "Can read your *notes*." / "Can change your *notes*." | owner |
| `paths: [...]` | Narrows kinds to folders, or names plain files by folder | "...only in *Projects/*." | owner |
| `calls: [name]` | Other plugins' operations and host methods beyond the basic set | "Can send messages through *Chat*." plus whatever that operation discloses (below) | owner |
| `people: true` | Group-scoped lookup | "Can see the people you share a group with." | owner |
| `network: [host] \| "user" \| "any"` | Destinations (rev 14's three lines, kept) | "Can send data to *api.example.com*." / "...to any website." | owner; "user" also by each person for their own server |
| `secrets: [{name, for, at}]` | A secret it may use for one destination | "Can use a key you provide for *api.example.com*; it never sees the key." | the person or manager who enters the secret |
| `programs: {image, logins, where}` | Runs a program (harness) | "Runs a program on machines you enroll, with network access and your *Claude* login." | owner; harness placement rule (owner 2026-11-20) |
| `triggers: {times, changes: [kind\|query]}` | Wake by alarm or change | "Can wake itself up (at most *N* pending)." | owner |
| `slots: [...]` | Slots it fills | "Can replace *the composer* in Chat." | owner |

**Disclosure through calls.** A published operation declares what its result may
contain: `returns: [kind, ...]`. A plugin granted that call is shown as able to read
those kinds "through Chat". So the install screen states everything a plugin can learn,
not only what it reads itself.

**Approval.** The space owner approves at install and on every update that widens the
grant; until then the old digest stays enabled (or the plugin stays disabled for a
fresh install). Narrowing needs nothing. The approval stores the grant's digest; the
host refuses any call outside the approved grant even if the manifest on disk changed.
The plugin may add one "why" line per field, shown in a labelled box "From the
publisher"; it cannot change the host's sentences. (Today's schema gives managers
`plugins.manage`; the owner said "the space owner decides what is installed": see
question 6.)

**Revocation.** Disabling or uninstalling bumps the space's authority version; every
later call and every commit rechecks (section 1.4) and the plugin's runs get Stop.

### 1.4 The call table and the one check

The table is in memory per space, built at enable time from the host's method list and
each enabled plugin's manifest; constant work, built once.

```text
entry = { name: "calternal-chat/send" | "host.files.append",
          handler: host | plugin(id, digest),
          input: schema, designators: [json paths of refs in the input],
          needs: capability (space.read | content.write | ...),
          effect: read | write | outbound | run,
          callers: person | plugins-granted | dependants,
          returns: [kinds] }
```

Names are `<plugin-id>/<op>` and `host.<method>`; a plugin cannot publish in another
plugin's namespace or under `host.` (today's ownership index,
`crates/plugin-host/src/claims.rs`).

**`authorize(ctx, entry, input)`**, one function, run on every hop:

1. **Entry live.** The entry exists, its plugin is enabled at the pinned digest.
2. **Caller may call.** If the caller is a person (from a client), the entry allows
   `person`. If the caller is plugin A, A's approved grant lists the name, or the name
   is in A's declared dependency's published set and A's grant lists it. Host methods
   in the **basic set** (read and write its own kinds, its own appdata, query its own
   kinds, arm its own alarms) need no listing.
3. **Person may.** The person's capability set in the space contains `entry.needs`,
   and for each designated target, `rights(person, target)` covers the effect.
4. **Designation rule** (section 1.5): every ref in a designator field is either
   reachable by the caller itself (`grant(A) ∩ rights(person)`) or is a **handle** the
   host issued to the caller in this person's context.
5. **Marked chain.** If `ctx.marked`, reads are filtered per author (section 1.6).
6. **Quota.** Reserve the call against the bucket of every plugin in the chain and of
   the person (section 1.10). Depth ≤ 8.
7. **Audit.** Append the record (section 1.12).

The decision is cached for the call with the space's **authority version** (one atomic
counter per space, bumped by any role, share, approval, consent or pick change). At
commit, the write compares the version; if it moved, it re-runs steps 1-5 inside the
commit. That is rev 14's "check inside the commit" without tickets.

Failures (one shape for all): `403 not_allowed` with a plain reason and next step, or
`404` identical for unknown and hidden targets; `429 over_quota` with retry-after;
`409 stale_authority` never reaches the caller (the host retries once).

Cost: steps 1-6 read in-memory per-space state: **0 disk reads warm**; one SQLite read
to load a space's authority snapshot after a cold wake. Flushes: 0 for reads (audit is
batched), 0 extra for writes (the audit row joins the write's commit).

### 1.5 Plugin calling plugin: whose rights (first-rank fork)

Example used throughout: a community composer `acme-composer` calls `calternal-chat/send
{chat, text}`; Chat appends a record to the chat file.

#### Model S (strict intersection; the thesis)

- A must hold: the call right, **and** every right B will use (write on `chats`).
- Person must hold: write on the chat.
- B may do: only what `grant(A) ∩ grant(B) ∩ rights(person)` allows.
- Confused deputy: impossible by construction; B can never do more than A could.
- Attack: A must be granted `chats: write` to send one message. A hostile composer then
  writes the chat file **directly**, skipping Chat's rules (48-hour edit window,
  format, quotes as references). The strict model hands more raw power to the less
  trusted plugin. It also breaks every useful layering: a "Save to Notes" button in
  Mail needs `notes: write` on Mail itself.
- Cost: no deputy analysis; every plugin's grant grows to the union of everything it
  touches through others; install screens say "can change your chats" for a composer.

#### Model D (delegated operation; the draft, made precise; recommended)

- **A must hold:** the call right `calls: ["calternal-chat/send"]` in its approved grant
  (shown as "Can send messages through Chat"). Nothing on `chats`.
- **The person must hold:** every right the operation uses on its targets:
  `content.write` in the space and write on the chat file.
- **B may do:** its effects run under `grant(B) ∩ rights(person)`, with the chain
  `[A, B]` in the context. B's reads are filtered per author if the chain is marked.
- **What B returns to A** is limited by B's declared `returns`, shown on A's install
  screen. If A is outbound-capable, the chain is marked, so B's reads inside this call
  already exclude non-consented records.
- **Preventing the confused deputy** (Hardy, *The Confused Deputy*, 1988,
  <http://cap-lore.com/CapTheory/ConfusedDeputy.html>): the danger is B using its own
  authority on a target A **named**. The designation rule closes it at the host:

  > A ref in a designator field of B's input counts only if A could reach that target
  > itself (`grant(A) ∩ rights(person)`), or A holds a host-issued **handle** for it in
  > this person's context. Otherwise the call fails `404` before B runs.

  A **handle** is an opaque token the host mints: `MAC(space key; target, person,
  holder plugin, effect ceiling, expiry)`. The host hands handles to A when (i) A is
  mounted in a slot about a target (the composer gets a handle to the open chat), (ii)
  A's own query returns a target, (iii) B returns a ref in a typed `ref` field to A.
  A handle lets A *name* the target to an operation; it never lets A read it. Verify
  costs one MAC (about a microsecond), no storage.
- **Attacks on model D and what stops each:**

  | Attack | Stopped by | Holds? |
  | --- | --- | --- |
  | A names Lee's private chat (UUID read from a link) in `chat/send` | Designation: A has no reach and no handle; the person (Ann) has no write either | Yes |
  | A names a chat Ann can write but that A was never shown, to post in her name | Designation: no handle for that chat in A's context | Yes |
  | A calls `chat/export {chat, to: "Shared/"}` so Chat copies a chat into a folder others read | Designation on `to`: A must reach `Shared/` itself or hold a handle Ann gave it through the host's folder picker. Moving the chat there would be challenged; a copy is not (G5) | Yes for naming; a copy Ann directed is her choice |
  | A passes a raw path **string** (not a typed ref) and B builds a path from it | Host cannot see it. The SDK types refs; the manifest validator refuses operation inputs with untyped path-like fields named as targets; a B that parses strings anyway is a buggy deputy | **Partly.** A bug in B, bounded by `grant(B) ∩ rights(person)` |
  | A asks B for data and exfiltrates it | B's `returns` is shown on A's install screen; if A has network the chain is marked and B's reads skip non-consented records | Yes for consent; disclosure is by install wording, as in any OS |
  | A loops A→B→A to accumulate rights | Each hop is checked; rights do not accumulate; depth ≤ 8 | Yes |
  | B is itself hostile and does more than its operation says | B is bounded by its own approved grant ∩ the person's rights; the owner approved B | By trust in installed plugins |
  | B answers A from warm memory of another person's data | Not stopped by the call check (section 3.1, gap G1) | **No** |

- Cost: one handle mint per slot mount or query result row exposed to a plugin; one
  MAC verify per designator. No stored tickets.

#### Recommendation

Model D. It gives the less trusted plugin less raw power, it matches how the owner
described the split of Chat, and its one new rule (designation) is checkable by the host
in one place. The thesis's "must not get weaker" line becomes: *a plugin calling a
plugin never gains rights over targets it could not name, and the callee never acts
beyond its own grant and the person's rights*. Question 1 asks the owner.

### 1.6 The content rule: agents and other people's writing

**How the host knows "agent" without knowing agents.** It knows which principals can
move bytes off the host where no reader check follows them:

> A principal is **outbound-capable** if it is a run (any program in a sandbox), or a
> plugin whose approved grant has `network`, `secrets` or `programs`. A call chain is
> **marked** if any principal in it is outbound-capable.

**The rule.** In a marked chain, a read (file read, record read, query, search, count,
excerpt, change feed) of a protected file returns a record authored by person X only if
X is the acting person, or X holds an active consent grant for this file whose
`harness` is `any` or names an outbound-capable plugin in the chain (and, for a one-run
grant, whose run is `ctx.run`). Other records are replaced by a **withheld marker** (`{withheld: true}` with no
author and no text); counts and search rank skip them. Which files are protected is
question 2; the recommendation: every file in which another person wrote, except files
in a space whose owner turned on *members' agents may read members' writing here*
(shown to everyone when they join). Per-file shares and chats between people are
always protected; no space setting reaches them.

Why this definition: a plugin with network can call a model API itself; if only program
runs counted, any network plugin would be an unlabelled agent. It also catches non-agent
plugins that send others' writing out (a link-preview fetcher sends message URLs to
third parties). That is intended, and question 2 asks it.

Agent replies: a record written by a run acting for Bob counts as Bob's writing.

**Consent grants** are host-held (section 1.13):

```text
consent = { space, file, author, scope: run(run_id) | standing, harness: any | plugin_id, at, by_session }
```

| Method | Who | Input | Result | Failure | Cost |
| --- | --- | --- | --- | --- | --- |
| `host.consent.set` | the author, from a host-drawn prompt only | file, scope, harness | ok | not the author's session; prompt expired | 1 / 1 |
| `host.consent.revoke` | the author | file or grant | ok | — | 1 / 1, version bump |
| `host.consent.list` | the author | — | their grants | — | 1 / 0 |
| (none for plugins) | plugins cannot set, read or forge consent; a plugin learns only "withheld" or "included" counts for its own run | | | | |

**The one-minute flow** (owner decided: ask each time or always, per conversation;
one minute; withheld on no answer):

1. A plugin asks to start a run with selected refs: `host.run.start({..., reads:
   {records: [refs], consent: "ask"}})`. The run's input is **refs**, never copied
   bytes of other authors (see gap G2 for what a plugin could still smuggle).
2. The host resolves the refs, groups them by author, and finds authors without a
   covering grant.
3. For each such author it creates an **ask** and draws the host prompt (a host
   component in the chat where the author is, plus a push "*Ann* wants an agent to read
   your messages in *Chat name*"): Allow once, Always in this chat, Don't allow.
4. The run's work row waits with `due = now + 60 s` (primitive 4's state column; this
   survives a restart because the asks live in the work row).
5. At the last answer or at `due`, the host **freezes the input**: consented authors'
   records included, the rest withheld. The caller sees "Lee's messages were withheld:
   no answer". Then the run starts.
6. A late answer: "Always" becomes a standing grant for later calls; "Allow once" after
   the freeze is recorded and does nothing for this run (the plugin may offer to call
   again).
7. Later reads by the run (tool search, opening a file) use the same rule live.
8. Revocation fences further reads of that author's records at once; bytes already
   delivered to the run (and to its model provider) cannot be recalled. The audit shows
   the author every read of their writing by a marked chain.

Cost: steps 2-3 one in-memory lookup per author; one flush to create the asks with the
work row; one flush per answer. The read check itself is an in-memory set lookup per
record author.

### 1.7 Origin stamps and "own" actions

Primitive 1 stamps every write `{person, plugin, run?, at}` where plugins cannot write
it (the files lane decides where). This section uses it:

- A kind declares its records' edit rule: `records: author` (default) or `records:
  writers`. Under `author`, `host.records.replace` and `host.records.tombstone` succeed
  only if `stamp.person == ctx.person`, or the person holds `space.moderate` (question 5)
  on that file. Any plugin, any chain: the host checks, not Chat.
- A record written by a run for Bob is Bob's to edit or delete.
- Time rules ("edit within 48 hours") are the plugin's: Chat reads `stamp.at`.
- **Hand edits.** A file changed by sync or a text editor is diffed into records by the
  host; changed records of another author under `records: author` are refused, and the
  person's version is kept as a conflict copy beside the file. New text is stamped with
  the syncing person.
- **Carried authors.** The host knows which authors' records it returned within one
  call. A write later in that same call is stamped `carries: [authors]` besides the
  person; marked chains treat a carried author like an author (consent needed). This
  costs one small set per call and closes laundering within a call, not across calls
  (gap G2). A run is one long context: its writes (the agent's reply in the chat)
  carry every other author it read, so a later agent reading that reply needs Lee's
  agreement too. Cost: the set of authors per run, bounded by the file's participants.
- The drawn author of a record always comes from the stamp, never from its bytes
  ("**Lee:** I agree" typed by Ann draws as Ann's).

### 1.8 Dependencies, slots and picks

| Declaration | Meaning |
| --- | --- |
| `depends: [{plugin, version, calls: [...]}]` | Install offers to install the dependency (owner approves each grant separately); enable refuses while a dependency is disabled. A dependency grants nothing by itself. |
| `slots: [{name, effect: viewer \| others, recommended: plugin}]` (in the app plugin) | A replaceable part. `viewer` (what only you see: list, composer, editor) is **picked per person**; `others` (what other people receive: AI, notifications) is **picked per space** by the owner (owner decided). |
| `fills: [{slot, ...}]` (in the part plugin) | The part. It runs with its own grant ∩ the picking person's rights; a per-space part runs per receiving person (section 1.9). |

**Missing or broken part** (owner decided: fall back or error, no plain copy in the core):

- `viewer` slot: the person's pick → the space default → the slot's recommended plugin if
  enabled → an error description drawn by the shell: "*Composer* is missing. Turn on
  *Calternal Composer*" (a button if the person may install, otherwise "Ask *owner*").
  A fallback shows a small notice once.
- `others` slot: the space's pick → the space's chosen fallback → error. Never a silent
  switch to a plugin the space did not choose for that slot, because it would change
  what other people receive.

**Disabling a plugin others rely on** (owner decided: warn for an official plugin with
important functionality). Made generic so first-party is not special: the host warns
whenever the plugin (a) has enabled dependants, (b) fills an `others` slot, or (c)
declares kinds with files in the space; the warning lists what stops and says files stay
and open as plain Markdown. For a plugin from the official publisher it adds "This is
part of Calternal; turn it off only if you know what you are doing." Owner only.

Picks are preferences, but a pick decides which plugin acts with the person's rights,
so a plugin must not be able to change it: picks are host-held (section 1.13), shown
in Settings.

### 1.9 Contexts without a live request

| Trigger | Context person | Rule |
| --- | --- | --- |
| Alarm armed by a call | the arming person | rights re-read at fire; if lost, the alarm is dropped and audited |
| Change subscription (a file or query) | the subscribing person | delivered per subscriber; a per-space part (notifications) subscribes per person who has it on |
| Space service (an indexer that reads the whole space) | `space-service` | rights = the plugin's grant over the space; results to people are **refs only**, filtered by the host for the viewer; marked chains read no protected records |

A plugin's own index or ranking (owner decided it may bring one) answers with refs and
scores; the host drops refs the viewer cannot read and takes titles and excerpts from its
own index. Ranks and counts from a plugin index can still leak existence (gap G3).

### 1.10 Quotas

Defaults are operator settings; the ceilings marked **fixed** (numbers proposed here, not yet measured) are security limits in the
contract (draft question 1, option a).

| Resource | Unit and scope | Checked at | On exceed |
| --- | --- | --- | --- |
| Userdata storage | bytes per space; per account quota as ADR 0059 | write commit | refuse uploads; writing of notes and messages still saves (ADR 0059: quota never loses writing), flagged over quota |
| Appdata | bytes per plugin per space | write | refuse; the plugin may rebuild |
| Compute | CPU time per call (deadline, **fixed max 30 s**); CPU seconds per plugin per space per hour; memory per worker | sandbox | kill the call; throttle the plugin |
| Calls | token bucket per (plugin, space) and per person; input and output size (**fixed max 4 MiB**) | call table step 6 | `429` |
| Chain depth | **fixed 8** | call table | `403` |
| Wake-ups | pending alarms per plugin per space; change deliveries coalesced to at most one per subscription per second | arm, deliver | refuse arm; coalesce |
| Runs | concurrent runs per person and per space; deadline (**fixed max**, operator default); output bytes per run | admission | queue or refuse |
| Egress | requests and bytes per plugin per space per hour | network method | `429` |
| Consent asks | per asker per hour; at most one open ask per author per file | ask | refuse, shown to the asker |
| Shares | per file (default 256 recipients) | share | refuse |
| Audit | reserved capacity per plugin per space | before the effect | refuse the effect (rev 14) |

Charging: compute and calls to each plugin in the chain and to the chain's first plugin
for the whole chain; storage to the space; runs to the person who started them, or the
space for a space-bound machine. No bucket is installation-wide.

### 1.11 Secrets a plugin can use but not read

| Method | Who | Effect |
| --- | --- | --- |
| `host.secrets.request(name)` | plugin | the shell draws a host form for the person (personal secret) or a manager (space secret); the value goes to `crates/secrets` |
| `host.net.fetch({url, secret?: name, ...})` | plugin | the host checks the destination against the grant, injects the value at the declared place (`header Authorization: Bearer {}`), refuses redirects with a secret, strips occurrences of the value (raw, base64, URL-encoded) from the response, audits |

Whose secret: a personal secret is used only in a context whose person owns it (a plugin
acting for Bob cannot use Ann's mail password); a space secret in any context of that
space whose plugin's grant names it. Values stay in `crates/secrets`, released only
through its audited `use_value` into the one outbound request.

What does not hold: a secret given to a **program** (env var, mounted login) is readable
by that program. Harness logins (`.claude`, `.codex`) are readable by the harness by
owner decision. The install screen says so.

### 1.12 Audit trail

One record per call-table hop: time, space, person, chain, run, operation, target IDs,
outcome, quota cost, latency, authority version. Never values, text, URLs with data or
outputs. Written into a per-space audit table: write calls in the same commit; read calls
batched (flush every second or 256 records); crash gaps recorded as gaps. Retention:
operator default 30 days, bounded by a size ceiling per space, oldest first.

Who sees it: the owner sees the space's audit; managers see plugin and member actions;
**every person sees reads of their own writing by marked chains** ("*Claude* for *Ann*
read 3 of your messages in *Chat name* at 14:02"). That last view is what makes consent
trustworthy.

### 1.13 What is stored, and its bound

| What | Where | Bound | Retention |
| --- | --- | --- | --- |
| Accounts, passkeys, sessions, invites, groups, memberships | `auth.db` (today) | as today | as ADR 0059/0060 |
| Space roles | `auth.db` | one per member | until changed |
| Shares and links | per-space authority table | 256 per file (operator) | until removed or the file is purged from Trash |
| Plugin approvals (grant digest, approver, time) | per-space authority table | one per installed plugin | until uninstall |
| Consent grants | per-space authority table | one-run grants: ended with the run; standing: one per (author, file, harness) | until revoked, or the author loses access |
| Asks | the run's work row | participants of the file | gone when the input freezes |
| Picks | per-space authority table (per person rows) | one per (person, slot) | until changed or the person leaves |
| Handles | not stored (MAC) | expiry 1 hour | — |
| Secrets | `crates/secrets` | per space and person (operator) | until removed |
| Audit | per-space audit table | size ceiling | 30 days default |

A visible, read-only Markdown projection of shares, approvals and consent
(`Settings/Access.md`, written by the host) keeps them human-readable (question 3).

### 1.14 Request-path cost summary

| Path | Disk reads (warm) | Flushes |
| --- | --- | --- |
| Any call's check (steps 1-6) | 0 | 0 |
| Read call with audit | 0 | 0 (batched) |
| Write call with audit | 0 for the check | the write's own commit, no extra |
| Marked read of a shared file | 0 (consent set in memory) | 0 |
| Share, consent, approval, pick change | 1 | 1, plus a per-space version bump |
| Cold space wake | 1 (authority snapshot) | 0 |

No installation-wide lock: authority state, version counter, call table and buckets are
per space. A group membership change touches each affected space's version in turn.

---

## 2. What plugins can do and cannot do

**Can:**

- Keep their own kinds of items and change them freely. *Reading List adds and ticks
  off links.*
- Read or change another plugin's kinds, if the owner approved it. *A "weekly review"
  plugin reads your notes.*
- Ask another plugin to do something it offers, without holding that plugin's data
  rights. *A community composer sends messages through Chat.*
- Replace a part of another app where that app offers a slot. *Your own composer in
  Chat, just for you.*
- Find people you share a group with. *A mention picker.*
- Run a program, such as an agent, with what the person may read and others agreed to.
  *Claude summarising a chat where everyone said yes.*
- Use a key you gave it to talk to one approved site. *A mail plugin logging in to
  your mail server.*
- Wake up at a time it chooses or when something changes. *Snooze brings a note back.*
- Stop being picked and still leave your files readable. *Turn off Chat; chats open as
  Markdown.*

**Cannot:**

- Read or change anything the owner did not approve, or the person cannot. *A reader in
  a space cannot change a note through any plugin.*
- Use another plugin to reach something it could not name itself. *A composer cannot
  make Chat post in a chat it was never shown.*
- Show an agent someone else's messages unless that person agreed. *Lee's messages are
  left out when Lee does not answer within a minute.*
- Pretend to be someone. *A message always shows who really wrote it.*
- Edit or delete another person's message. *Even a plugin with write access to chats.*
- Send data anywhere not on its install screen. *A notes theme has no network at all.*
- See a secret's value (except a program it runs). *The mail plugin never holds your
  password.*
- Draw a fake permission prompt or install screen. *Only the host draws those.*
- Fill the disk, run forever, or outlive Stop or uninstall. *A runaway loop is killed.*
- Find people outside your groups. *No directory of everyone.*

---

## 3. Security

### 3.1 How a hostile or broken plugin is contained in this area

| Attack | What stops it | Where it is checked |
| --- | --- | --- |
| Call an operation not in its grant | Step 2 | call table |
| Write where the person cannot | Step 3 and recheck at commit | call table, commit |
| Use B as a deputy on a target it named | Designation rule | call table step 4 |
| Forge a handle | MAC with a per-space key | call table step 4 |
| Read others' writing through a non-outbound plugin while itself having network | Chain marked by any outbound principal | read check |
| Start a run with copied bytes of others' messages in its prompt text | Run inputs that name records are refs; free text is the plugin's own. **Not stopped** if the plugin itself read the bytes (G2) | partly at `host.run.start` |
| Forge or skip consent | Consent set only from the author's session via a host prompt | consent methods |
| Spam consent prompts | One open ask per author per file; asks per hour | consent quota |
| Edit or tombstone others' records | Author-owned records | records methods |
| Change a person's picks to itself | Picks are host-held | authority table |
| Widen an audience by moving or sharing | Host challenge to the person | files and share methods |
| Keep acting after disable or a lost role | Version bump; recheck at commit; Stop to runs | commit, run supervisor |
| Exfiltrate a secret | Value never in plugin memory; response scrubbing | network method |
| Hide in the audit | Audit reserved before effect; refused when full | call table step 7 |
| Remember Ann's data and give it to Bob from a warm worker | **Not stopped** (G1) | — |

### 3.2 Threat model for the whole design

Assumed trusted: the host binary, the operator, the client binaries we ship, the
sandbox (Podman, the JS isolate), and the space owner's choice of installed plugins
*for data the owner could read anyway*. Out of scope: a hostile operator (sees
everything; no end-to-end encryption yet, owner put it on the roadmap), a person
copying text by hand into an agent.

#### A. A hostile plugin (installed by the owner, or updated to bad code)

| # | Tries | What stops it | Holds? |
| --- | --- | --- | --- |
| A1 | Read kinds or folders outside its grant | Basic set + grant check per host method | yes |
| A1b | Claim another person, space or run in its input | The call context is set by the host from the channel and session; input fields never select person, space or machine (rev 14 rule kept) | yes |
| A1c | Reach another space (a ref or UUID from a different space) | A worker's channel is bound to one space; cross-space refs answer `404`; handles carry the space | yes |
| A2 | Act beyond the acting person's rights | Intersection with `rights(person)` | yes |
| A3 | Act with no person (alarm, change) after the person lost access | Rights re-read at fire | yes |
| A4 | Confused deputy through another plugin | Designation rule (1.5) | yes for typed refs; B parsing raw strings is a bug in B |
| A5 | Learn data from B's results | `returns` on the install screen; marked chain | by disclosure |
| A6 | Cache one person's data and show it to another | Drawing reads come from host queries under the viewer, not from plugin code (primitive 2/6); but operation results and snapshots come from plugin code | **no (G1)** |
| A7 | Launder others' writing: a no-network plugin copies Lee's text into a file stamped as Ann's, a network plugin reads it | Per-call taint: a write in a call that read other authors' records carries those authors in its stamp | within a call yes; **across calls no (G2)** |
| A8 | Forge an author, rewrite history | Host stamps; author-owned records | yes |
| A9 | Spoof host UI (prompt, install screen, consent) | Host-only components; sealed frame cannot overlay shell chrome | yes, if the sealed frame holds on every client |
| A10 | Exfiltrate over the network | Declared destinations; public addresses only; DNS and TLS pinning; no private ranges (rev 14 rules, `crates/server/src/netguard`) | yes, to approved destinations only |
| A11 | Exfiltrate through rendered images or links | Images only through the anonymous proxy when declared; links drawn by the host show the destination | **partly**: data can be encoded in a link the person clicks (G4) |
| A12 | Exfiltrate through push text | Push is title plus fixed phrase (owner decided) | yes |
| A13 | Copy data into a folder others can read | Allowed if the person could; audience widening of an existing file is challenged; a new copy is not | **no** for copies, as in any app with read and write (G5) |
| A14 | Read a secret | Injection by the host; scrubbing | yes, except values given to programs |
| A15 | Fill the disk, spin the CPU, flood calls or alarms | Quotas (1.10) | yes |
| A16 | Starve other plugins in the space | Per-plugin buckets; per-space work; no global lock | yes, within a space's capacity |
| A17 | Outlive Stop or uninstall | Host kills workers; machine supervisor stops runs | yes on the host; on a machine see C |
| A18 | Take an official ID or another plugin's namespace | Namespace ownership index; publisher signatures | yes |
| A19 | Update to wider rights silently | Wider grant waits for owner approval | yes |
| A20 | Update to bad code within the same grant | Digest pinning; updates need approval of the new digest? | **open (question 6)** |
| A21 | Fill a per-person slot to spy on its picker | It runs only with the picker's rights; it is the picker's choice | yes |
| A22 | Learn hidden items from a plugin-owned index's ranks and counts | Host filters refs; counts from plugin indexes are not shown | **partly (G3)** |
| A23 | Timing side channels on queries | Not addressed | **no (G6)** |
| A24 | Oversized descriptions to freeze clients | Description size budget (interface lane) | yes |

#### B. A hostile member of a shared space

| # | Tries | What stops it | Holds? |
| --- | --- | --- | --- |
| B1 | Read or write beyond their role | Capability check per call | yes |
| B2 | Edit or delete others' messages or records | Author-owned records | yes (moderation: question 5) |
| B3 | Post as someone else (also by hand-editing a synced file) | Stamps from the session; hand edits of others' records refused | yes |
| B4 | Share another person's file outside | Needs the share right on that file; recipients must be visible to them; widening challenge | yes, within the right they hold |
| B5 | Find people outside their groups | Group-scoped lookup only | yes |
| B6 | Invite strangers into a group | ADR 0059 invites; only into own groups | by design (members may invite) |
| B7 | Feed others' messages to an agent | Per-author consent | yes, until the author consents |
| B8 | Consent fatigue: ask again and again | One open ask per author per file; hourly ask quota | partly |
| B9 | Get "Always" once, then run many agents | Standing grant is per file and harness; visible audit of every read; revocable | by design |
| B10 | Install a hostile plugin | Owner only installs (question 6) | yes |
| B11 | Pick a per-space part | Owner only | yes |
| B12 | Flood others with notifications | Rate per person; mute | yes |
| B13 | Read the audit to learn others' activity | Owner and managers only; each person sees only reads of their writing | yes |
| B14 | Keep access after removal from the group or space | Next request; version bump | yes on the server; **no for copies already downloaded to their device (G7)** |
| B15 | Read a message deleted for everyone in file history | Tombstone must purge the bytes from revisions too (files lane) | **must be specified (G8)** |
| B16 | Hide a file from others by moving it | Narrowing is allowed to those with the right; audited | by design |
| B17 | Exhaust the space's storage | Per-account quota (ADR 0059); space quota | yes |

#### C. A hostile harness (program) on a machine

| # | Tries | What stops it | Holds? |
| --- | --- | --- | --- |
| C1 | Read files outside its mounts | Sandbox mounts only granted folders | yes, if the sandbox holds |
| C2 | Write into other files on the server | Its output is its own file; other writes are calls under the run's context | yes |
| C3 | Claim a person's authorship | Appends stamped with the run identity | yes |
| C4 | Inject output after Stop, or into another run | Per-run credential; Stop epoch; appends after Stop refused | yes |
| C5 | Ask the host for more data (search) | Run context = caller's rights ∩ everyone in the file's audience (owner decided) ∩ consent | yes |
| C6 | Send what it was given anywhere | Approved harness on hosted sandbox: egress allow-list. Unapproved harness on the person's own machine: full network by owner decision | **by design: whatever it receives can leave** |
| C7 | Read the person's provider logins | Allowed by owner decision for their own sandbox | by design |
| C8 | Prompt injection from consented messages | Bounded by the run's grant and the person's rights; risky effects need the host challenge to the person | partly |
| C9 | Ignore Stop or the 24-hour outage limit | Enforced by the machine supervisor, not the harness | yes against the harness; **no against a hostile machine owner (C10)** |
| C10 | The machine's owner tampers with the supervisor | They receive only what their runs were given, which they could read anyway; consented bytes of others then reach a machine the consenting person did not choose | **gap G9**: consent should name where the run executes |
| C11 | Flood output | Output bytes per run quota; sync bounded | yes |
| C12 | Escape the sandbox to the machine | Rootless Podman; the machine owner's risk | outside the host |

#### D. A compromised client (stolen session, or a modified client)

| # | Tries | What stops it | Holds? |
| --- | --- | --- | --- |
| D1 | Act as the person | Session revocation; device list | **no until revoked** |
| D2 | Approve host challenges automatically (installs, widening, "Always" consent) | Challenges protect against plugins, not against the client | **no, unless high-risk challenges need a passkey (question 7)** |
| D3 | Run plugin code outside the sealed frame | Only our clients enforce it | n/a for a modified client: it only gets what the person can read |
| D4 | Replay requests | Request IDs; session binding | yes |
| D5 | Sync forged files | Stamps from the session; others' records refused | yes |
| D6 | Bulk download everything | A feature (offline download, owner decided); audited; rate limited | by design |
| D7 | Leak offline copies after sign-out | Wipe on sign-out; space offline expiry (owner decided) | only for a cooperating client |

#### Gaps, named honestly

- **G1 Warm workers remember.** A plugin worker that served Ann can answer Bob from
  memory. Rev 14 accepted this ("members trust the Owner's installed plugins"). Fix
  options in question 4.
- **G2 Laundering across calls.** A plugin with read on chats and a run or network
  right can read Lee's message in one call and send it in another. The host sees each
  call separately. Fix: treat the whole plugin as marked (it is, if it has network or
  programs: then all its reads are consent-filtered); the remaining hole is two
  colluding plugins, one unmarked reader and one marked sender, communicating through a
  file. The per-call taint stamp catches the file route only within a call.
- **G3 Plugin indexes** leak existence through rank and result counts.
- **G4 Links** can carry data in their URL when a person clicks.
- **G5 Copies** into shared folders are not challenged.
- **G6 Timing** is not addressed.
- **G7 Device copies** survive removal from a space until the device next signs in.
- **G8 Deleted bytes in file history** must be purged with the tombstone (files lane).
- **G9 Consent does not name the machine** a run executes on.
- **G10** Delete for everyone cannot reach an agent's log or a model provider that
  already received the message.

---

## 4. Was and is

| Topic | `dev` today | Revision 14 | This design |
| --- | --- | --- | --- |
| People, groups, invites, roles | `crates/auth` (people, invites, groups, spaces, capability sets) | same, plus six audiences, `group_people_scope` | same as `dev`; one people lookup |
| Sharing | `item_grants` for notes and collections only | widening protection record, guest tickets, graph edges | `item_grants` for any file or folder; widening is a host challenge |
| Call authority | `crates/server/src/authz` policy table per route; plugin calls on a one-call channel | tickets (60 s), execution leases, attempt principals, activation resolve/confirm/dispatch, per-carrier checks | one call table, one `authorize`, handles |
| Plugin to plugin | none | extension contributions intersecting declarations | model D with the designation rule |
| Agents and others' writing | agent access records (`crates/auth/src/agents.rs`), Full access mode | `attempt_disclosure`, discovery classes `creator_only` / `consent_only`, consent modes | outbound-capable marking, per-author consent grant, one rule |
| Authorship | `Actor {account, via}` (`crates/types/src/actor.rs`) | attested prefix/suffix, `item_spans` | origin stamps; author-owned records |
| Secrets | `crates/secrets`: handles, audited use | credential routes with fixed profiles | injection into a declared destination |
| Network | `crates/server/src/netguard` | three install lines, pinning rules | same three lines, as host methods |
| Audit | secrets audit, turn records | audit fields, 10,000 records or 7 days | one record per hop, per space, 30 days default |

**Deleted:** stored tickets; the activation resolve/confirm/dispatch split (kept only as
the host challenge); `attempt_authority_end`; credential route profiles; discovery
classes; consent modes beyond two grant lengths; widening protection records; guest
ticket graph edges; the protected built-ins list and bundled-ID exceptions; the
process-wide item access epoch (replaced by a per-space version).

**Survives (paths on `v2-host-integration`):** `crates/auth/*` (people, groups,
invites, spaces, sessions, machines); `crates/types/src/capability.rs`;
`crates/types/src/actor.rs` (becomes the stamp's person/plugin/run); `crates/auth/src/sharing.*`
(kind check dropped); `crates/secrets`; `crates/server/src/netguard`;
`crates/server/src/authz/mod.rs` (its scope enum and policy table seed the call
table); `crates/plugin-host/src/claims.rs` (namespace ownership); the request-ID
deduplication from slice 1.

---

## 5. Worked examples

### 5.1 Reading List

```ts
export default definePlugin({
  id: "reading-list",
  kinds: { reading: { fields: { url: "url", title: "text", read: "bool" },
                      row: ["title", "site(url)"] } },
  // no grant block: own kind only, no network, no programs
  operations: {
    add:      create("reading", ["url", "title"]),
    markRead: setField("reading", "read", true),
  },
  collections: { Unread: query("reading").where("read", "=", false).newest(),
                 All: query("reading") },
});
```

Install screen (host-written): "Reading List keeps reading-list items in this space. It
cannot read anything else, has no network access and cannot run programs." Calls: the
person's tap on "Mark read" → `reading-list/markRead {item}` → `authorize`: entry allows
persons, Ann holds `content.write`, the item is her own kind → write, audit in the same
commit.

### 5.2 Notes

```ts
export default definePlugin({
  id: "calternal-notes",
  kinds: { notes: { fields: { tags: "text[]", snooze_until: "time" } } },
  grant: { triggers: { times: true } },            // snooze
  publishes: { create: { input: { folder: ref("folder"), title: "text" },
                         needs: "content.write", returns: ["notes"] } },
  operations: {
    snooze: async (ctx, { note, until }) => {
      await ctx.host.files.setField(note, "snooze_until", until);
      await ctx.host.alarms.arm({ at: until, op: "unsnooze", input: { note } });
    },
    unsnooze: (ctx, { note }) => ctx.host.files.setField(note, "snooze_until", null),
  },
});
```

Sharing is not Notes' code: the shell's Share sheet calls `host.share.add`. When the
alarm fires, the context person is whoever snoozed; if they lost access, the alarm is
dropped. Mail's "Save to Notes" calls `calternal-notes/create` under model D: Mail's grant
lists the call; the `folder` designator must be a folder Mail holds a handle for (from
the host's folder picker), so Mail cannot save into a folder Ann never picked.

### 5.3 Chat (people and agents, consent, quotes, delete, restart)

Plugins and grants:

| Plugin | Grant | Outbound-capable? |
| --- | --- | --- |
| `calternal-chat` | own kind `chats` (`records: author`), `people: true`; slots `composer` (viewer), `ai` (others), `notify` (others) | no |
| `calternal-chat-composer` | `calls: [calternal-chat/send, calternal-chat/draft]` | no |
| `calternal-chat-ai` | `calls: [host.run.start]`, fills `ai`; programs via the harness slot | **yes**: all its reads are consent-filtered |
| `calternal-chat-claude-code` | `programs: {image, logins: [~/.claude], where: enrolled or approved}`, network | yes (it is a run) |
| `calternal-chat-notify` | change trigger on `chats` per subscriber; `host.notify` | no |

Send from the composer:

```ts
// calternal-chat-composer, mounted with a handle to the open chat
onSubmit: (ctx, text) => ctx.call("calternal-chat/send", { chat: ctx.slot.target, text }),

// calternal-chat
publishes: { send: { input: { chat: ref("chats"), text: "text", quote: ref("record").optional() },
                     needs: "content.write", returns: [] } },
send: (ctx, i) => ctx.host.records.append(i.chat, { text: i.text, quote: i.quote }),
```

`authorize` for the first hop: the composer's grant lists `send`; Ann may write the
chat; `chat` is a handle the composer received at mount. Chat's append is a second
hop, a host method in Chat's basic set; the stamp is `{person: Ann, plugin:
calternal-chat, via: acme-composer}`.

Calling an agent with a selection:

```ts
// calternal-chat-ai
call: async (ctx, { chat, selection, prompt }) => {
  const run = await ctx.host.run.start({
    harness: ctx.slot("chat.harness"),
    reads: { records: selection, consent: "ask" },   // host asks, waits 60 s, freezes
    prompt: prompt,                                    // Ann's own record, by ref
    replyTo: { chat, as: "run" },
  });
  return { run: run.ref, withheld: run.withheld };   // counts only, e.g. { Lee: 3 }
},
```

Lee sees the host prompt; no answer in a minute → Lee's 3 messages are withheld markers
in the run's input; Ann sees "Lee's messages were withheld". The run's tool search later
sees Lee's records in this chat as withheld too. Every read is in Lee's audit view.

Quotes are refs (`quote: ref("record")`); drawn with the reader's rights; a run reading
a reply that quotes Lee sees the quote withheld if Lee did not consent.

Delete for everyone: `calternal-chat/delete {record}` → `host.records.tombstone(record)`;
the host checks `stamp.person == Ann`; the bytes leave the file and its history (G8);
every quote of it draws as nothing. An agent that already read it keeps what it got (G10).

Restart: consent asks and the frozen input live in the run's work row; the run keeps
going on its machine; its replies are appended later by its run identity, stamped
`{person: Ann, run: R}`; a consent answer sent during the outage is applied when the
server returns (if after the freeze, only "Always" has an effect).

---

## 6. Where the thesis is wrong or incomplete here, and what changed

| Thesis says | Problem | Changed to |
| --- | --- | --- |
| "The callee runs with the intersection of the caller's grant, its own grant and the person's rights." | A composer would need write on chat files: more raw power to the less trusted plugin (1.5) | Model D: call right + person's rights + callee's grant + designation rule (question 1) |
| "A run acting for an agent may not read bytes whose author has not granted that plugin access." | The host does not know agents; a network plugin is an agent too; reads through another plugin escape the rule | Outbound-capable principals and marked chains (1.6) |
| "...for any shared file" | Breaks agent search of team notes in group spaces, which the owner allowed | Scope is a question (question 2) |
| "Every action runs with the intersection of the plugin's grant and the acting person's rights." | Says nothing about memory between calls, person-less triggers or plugin indexes | Contexts (1.9), refs-only plugin answers, gap G1 (question 4) |
| "A plugin holds a grant that the space owner approves at install." | Some grants are per person at run time (consent, personal secrets, enrolled machines, "servers you add") | Grants have install parts (owner) and personal parts (each person) |
| Origin stamps are "the basis for own actions" | A stamp alone does not stop a plugin with write on the kind | Author-owned records are a host rule (1.7) |
| "Userdata is strictly files." | Shares, consent, approvals and picks are not rebuildable, yet a file is writable by sync, editors and plugins | Host-held tables with a read-only Markdown projection (question 3) |
| "Secrets that plugin code can use but not read" | False for programs and harness logins (owner decided) | Stated as an exception |
| Primitives 3 and 5 are separate | The permission check exists only at the call table; people and groups are data the check reads | Merge them into one primitive, **Authority** (seven become six) |

---

## 7. Questions for the owner

Most consequential first.

**1. When one plugin calls another, whose rights apply?**
*What:* a composer you install asks Chat to post your message. Either the composer
must itself be allowed to write chat files (strict), or it is allowed only to *ask
Chat to post*, and Chat does the writing with its own rights, for you, after the host
checks you may post there. *Why now:* every split app (Chat's parts, Mail saving to
Notes) and every grant on every install screen depends on it. *Example:* a community
composer under the strict rule shows "Can change your chats" and could rewrite any
message directly; under the delegated rule it shows "Can send messages through Chat"
and can only post where you are, in a chat it was shown. *Options:* (a) **strict
intersection** (the thesis): no deputy problem, but the smaller plugin gets more raw
power, grants grow to everything a plugin touches through others, and layering stops
being useful. (b) **delegated operation** (the draft, made precise): the caller needs the
right to call, you need the rights the operation uses, the callee acts under its own
grant, and anything the caller names must be something it could reach itself or was
handed by the host; costs one new rule (designation) and one token check per named
item; the residual risk is a callee that builds paths from plain strings, which the SDK
and validator discourage. *Recommendation:* (b): it gives less power to the less trusted
plugin, and the host can check the one rule.

**2. Which writing does "an agent may read another person's writing only with that
person's agreement" cover, and what counts as an agent?**
*What:* the host cannot tell an agent from other code. The proposal: anything that can
send data off the server (a running program, or a plugin with network, secrets or
programs) counts. And the rule must say which files it protects. *Why now:* it decides
whether agents can search a team's notes and whether link previews or translators work
in chats. *Example:* in your "Family" group space, you ask Claude "find the notes about
the lease"; one of them was written by your partner. *Options for scope:* (a) **chats
only** (kinds opt in): team notes stay searchable; protection depends on the kind's plugin
opting in. (b) **every file with another author**: strongest; agents in group spaces see
almost nothing without many prompts. (c) **(b), except in spaces whose owner turned on
"members' agents may read members' writing here", shown to everyone at join**: team
spaces work; files shared one by one and chats between people stay protected. *Options for "agent":* (i)
programs only: simple, but a plugin with network can call a model itself unlabelled;
(ii) **anything outbound-capable**: airtight against plugins, and link-preview or
translation plugins also need consent in chats. *Recommendation:* (c) with (ii).

**3. Is security state an exception to "userdata is strictly files"?**
*What:* who may see a file, which plugins are approved, who agreed to agents reading
their messages, and which composer you picked cannot be rebuilt, so the rule says
files. But a file can be changed by sync, a text editor or a plugin. *Why now:* it
decides where the permission check reads from. *Example:* a plugin with write access to
`Settings/` edits `consent.md` to say Lee agreed. *Options:* (a) **host-held tables**
(as `auth.db` and `item_grants` are today), plus a read-only Markdown copy the host
writes for people to read: safe and fast; not editable by hand. (b) Files the host alone
writes, refused for any other writer: one rule, but sync and backups must special-case
them, and a restore from an old backup could bring back revoked consent. *Recommendation:*
(a).

**4. May a plugin remember one person's data and show it to another?**
*What:* a plugin's process serves everyone in the space. The permission check limits
what it can fetch in each call, not what it remembers. *Why now:* it decides the memory
cost of every plugin (being measured on #103). *Example:* a "chat summary" plugin read
Lee's private note while serving Lee; asked by Ann, it repeats a line from it.
*Options:* (a) **trust installed plugins** (revision 14's rule): no extra cost; the
owner's choice of plugins is the boundary. (b) **one process per person** for plugins
that read kinds of others: airtight; memory grows with active people. (c) (a) for plugins
with no network and no programs, (b) for outbound-capable ones: most plugins cheap, the
risky ones isolated. *Recommendation:* (c), pending the memory numbers.

**5. Can a space manager delete other people's messages?**
*What:* records are author-owned: only the writer can edit or delete. A moderation right
would let some role delete anyone's. *Why now:* it is a host rule (otherwise any plugin
could do it). *Example:* someone posts abuse in the family group's chat. *Options:* (a)
**authors only**: simplest; nobody can clean up. (b) **managers and owner of the space**
may delete (not edit) others' records, shown as "deleted by a manager": a moderation
path, a new capability `space.moderate`. In a chat between people (a file shared with
full rights, no space role), only (a) applies. *Recommendation:* (b).

**6. Who approves plugins, and do updates with the same rights need approval?**
*What:* today's role table lets managers install plugins; you said the space owner
decides. And an update can change behaviour without asking for new rights. *Why now:* it
is the line between a bad member and a bad plugin. *Example:* a community composer
updates itself to send drafts to its own server: blocked (new network line needs you);
but it could start garbling messages with the same rights. *Options:* (a) **owner
approves installs and wider rights; updates within the same rights install
automatically** for signed publishers: little friction; behaviour can change silently.
(b) Owner approves every new digest: safe, a prompt per update. (c) (a) for publishers
you marked trusted, (b) for the rest. *Recommendation:* (c), and drop `plugins.manage`
from the manager role.

**7. Should the riskiest confirmations need your passkey?**
*What:* host prompts stop plugins from acting without you, but a stolen session or a
modified app can press "Allow" itself. *Why now:* it decides the design of the host
prompt. *Example:* a stolen laptop session approves a plugin with "Any website" and grants
"Always" for agents in every chat. *Options:* (a) a tap is enough: no friction, no
protection against a compromised client. (b) **a passkey check for: installing or
widening a plugin, sharing to people outside the space, and "Always" consent**: a few
Face ID prompts a month. *Recommendation:* (b).

---

## Simplification opportunities

- Merge primitives 3 and 5 into one **Authority** primitive: the check only exists at
  the call table.
- Replace the process-wide item access epoch (`crates/types/src/item_access.rs`) with
  one counter per space: fewer cache flushes and no installation-wide fence.
- Drop the `kind IN ('notes','collections')` check in `item_grants`: one share table for
  every file and folder.
- Rev 14's tickets, attempt leases and activation tables collapse into the call context
  plus stateless handles.
- The two discovery classes and consent modes become one per-author rule with two grant
  lengths.
- `crates/server/src/authz/mod.rs`'s `Scope` enum is already the first column of the
  call table; reuse it rather than writing a new policy layer.
