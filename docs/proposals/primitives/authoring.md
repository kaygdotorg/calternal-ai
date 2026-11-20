# Authoring against the primitives: a paper prototype

Status: a proposal for #107, written on the night of 2026-11-20 against the
thesis (`docs/proposals/primitives-only-thesis-2026-11-21.md` on `dev`), the
contract owner's draft (`docs/proposals/primitives-only-host-2026-11-21.md` on
`lane/v2-host-base`) and the four design sections under
`docs/proposals/primitives/` (first versions, read at 18:00 UTC):
`files-and-questions.md` (`lane/design-files`), `permissions-and-calls.md`
(`lane/design-authority`), `running-code.md` (`lane/design-run`) and
`drawing.md` (`lane/design-interface`). Nothing here is decided. The
TypeScript is not built; it is written to be type-shaped and plausible, so
that what an author writes, and where it gets awkward, can be measured.

Why this matters: on the revision 14 contract, the plugin SDK (#106) removed
Chat's hand-written test host and build script, but Chat's own source only
went from 1,884 to 1,767 lines (about 6 percent). The rest was contract
vocabulary that no SDK can hide. This document shows the same plugins when
the contract is seven primitives.

Where the sections disagree on a name, I use one name and say which section
it comes from; section 3 lists the disagreements.

## 1. The whole author-facing API

One import, `@calternal/sdk`. A plugin is a manifest written in TypeScript;
every handler receives one context, `ctx`. Every call goes through the
host's call table and its one check (the permissions section's
`authorize`); nothing below is a capability by itself.

```ts
// ── The package ──────────────────────────────────────────────────────────
declare function plugin(p: {
  id: string; name: string; version: string; icon?: string;
  depends?: { plugin: string; version: string }[];
  grant?: Grant;                         // what it asks for beyond its own kinds (primitive 3)
  kinds?: Record<string, Kind>;
  operations?: Record<string, Operation>;
  sidebar?: (Collection | Facet)[];      // the app's default arrangement
  watches?: Watch[];                     // change triggers created at install (primitive 4)
  programs?: Record<string, Program>;    // programs it runs in boxes (primitive 4)
  slots?: Record<string, Slot>;          // places others may fill (primitive 6)
  fills?: Record<string, Part>;          // places it fills
  phrases?: Record<string, string>;      // the only texts a push may carry (primitive 7)
}): Plugin;

// ── 1. Files ─────────────────────────────────────────────────────────────
declare function kind<F extends Fields>(k: {
  fields: F;                             // text() url() bool() number() date() datetime() link() person() file() list(x); .index() to query it
  records?: { fields?: Fields; edit: Policy; redact: Policy; position?: string; segment?: "monthly" };
  folder?: string;
  row?: Look; card?: Look; cell?: Look; page?: Page; record?: Look;   // drawing, primitive 6
}): Kind;
type Policy = "author" | "author-or-manager" | "writers";
interface Files {                        // ctx.files (files section 1.2)
  create(kind: string, f: { fields: object; body?: string; folder?: string; name?: string; request: string }): Promise<FileRef>;
  read(file: FileRef, at?: { revision?: string }): Promise<{ fields: any; body: string; revision: string }>;
  replace(file: FileRef, w: { fields?: object; body: string; expected: string; request: string }): Promise<{ revision: string }>;
  set(file: FileRef, fields: object, expected?: string): Promise<{ revision: string }>;
  history(file: FileRef): Promise<{ revision: string; change: string; stamp: Stamp }[]>;
}
interface Records {                      // ctx.records (files section 1.3)
  append(file: FileRef, r: { body: string; fields?: object; request: string }): Promise<{ anchor: Anchor }>;
  edit(file: FileRef, anchor: Anchor, r: { body: string; expected: string }): Promise<{ revision: string }>;
  redact(file: FileRef, anchor: Anchor): Promise<void>;
  read(file: FileRef, q: { anchors?: Anchor[]; after?: Anchor | { revision: string }; limit?: number }): Promise<Rec[]>;
}
interface State { get(key: string): Promise<unknown>; set(key: string, value: unknown): Promise<void> }  // ctx.state, per person and plugin
// Host operations, called like any other: host.files.move, host.files.trash, host.files.restore, host.change.revert.

// ── 2. Questions ─────────────────────────────────────────────────────────
declare function collection(title: string, q: Query & { hand?: true; view?: "list" | "board" | "gallery" | "strip" }): Collection;
declare function facet(title: string, q: { kind: string; by: string }): Facet;
type Query = { kind?: string | string[]; where?: string; sort?: string; linksTo?: FileRef | "this"; text?: string; limit?: number };
interface Questions { query(q: Query): Promise<{ rows: Row[]; count?: number; next?: string }> }   // ctx.query

// ── 3. People and permissions ────────────────────────────────────────────
type Grant = {
  kinds?: Record<string, "read" | "write">; paths?: string[];   // other plugins' kinds, narrowed to folders
  calls?: string[];                                              // other plugins' operations
  people?: true;                                                 // look up people in shared groups
  network?: string[]; secrets?: { name: string; for: string }[]; // primitive 7, declared here
};
interface People {                       // ctx.people
  find(text: string): Promise<Person[]>;
  share(file: FileRef, s: { people?: PersonRef[]; group?: string; role: "reader" | "editor" }): Promise<void>;  // widening draws the host challenge
}

// ── 4. Running code ──────────────────────────────────────────────────────
interface Work {                         // ctx.work (running-code section 1.2)
  start(w: { request: string; op?: string; program?: string; input: object; at?: string; key?: string;
             deadline?: string; place?: "space" | "hosted" | "mine" | "ask";
             reads?: { records: RecordRef[]; consent: "ask" } }): Promise<{ id: WorkRef; state: string }>;
  send(id: WorkRef, record: object, request: string): Promise<{ seq: number }>;
  stop(id: WorkRef, request: string): Promise<void>;
  list(q: { key?: string; state?: string }): Promise<WorkRow[]>;
}
type Watch = { target: Query | { file: FileRef }; op: string };
type Program = { image: string; entry: string; network?: string[]; login?: { name: string; mount: string }; workdir?: "none" | "scratch" | "ask" };
// Inside a box, the program's entry gets a second, smaller API (@calternal/sdk/box):
interface Box {
  input<T>(): T; records(): Promise<{ author: string; body: string; withheld?: true }[]>;   // the frozen, consented set
  inbox(): AsyncIterable<InboxRecord>; log: { append(r: object): void; call(name: string, input: object): void; end(how: string): void };
}

// ── 5. Calls ─────────────────────────────────────────────────────────────
declare function operation<I extends Fields, O>(o: {
  title: string; input: I; returns?: string[];       // kinds its result may contain (shown on callers' install screens)
  public?: true; confirm?: string;                   // callable by other plugins; a host-drawn "are you sure"
  handler: (input: Values<I>, ctx: Ctx) => Promise<O>;
}): Operation;
declare function create(kind: string, o: { inputs: string[] }): Operation;     // declarative, no handler
declare function set(kind: string, fields: object): Operation;                 // declarative, no handler
interface Calls { call<O>(name: string, input: object): Promise<O> }           // ctx.call("calternal-chat/send", …)

// ── 6. Drawing ───────────────────────────────────────────────────────────
// Looks name fields, never values: row { title, detail, leading, trailing }, card { image, title, detail },
// cell { title, leading }, page: form | editor | records(...) | collection | frame, record { body, quote, status }.
// Per-person fields use me.: me.unread (= after_position), me.<state key>.
declare function slot(s: { effect: "viewer" | "others"; recommended: string; gives?: string[]; may?: string[] }): Slot;
type Part = { composer?: object; page?: Page; form?: string; actions?: Record<string, Action> };
type Action = ({ op: string; inputs?: Record<string, string> } | { compose: Record<string, string> })
  & { when?: string; confirm?: string; place?: "menu" | "swipe" | "primary" };

// ── 7. The outside ───────────────────────────────────────────────────────
interface Outside {
  net: { fetch(url: string, i?: { method?: string; headers?: Record<string, string>; body?: string; secret?: { name: string; header: string } }): Promise<Response> };
  notify(to: PersonRef[], n: { item: FileRef; phrase: string }): Promise<{ sent: number }>;
}

// ── The context ──────────────────────────────────────────────────────────
interface Ctx extends Questions, Calls, Outside {
  me: Person; request: string; key(purpose: string): string;
  files: Files; records: Records; state: State; people: People; work: Work;
  change?: { target: FileRef; from: string; to: string };   // set when a watch fired
  scheduled?: string;                                         // set when a time fired
}
type Rec = { anchor: Anchor; body: string; fields: any; stamp: Stamp; revision: string; withheld?: true; redacted?: true };
type Stamp = { person?: PersonRef; plugin: string; run?: WorkRef; carries?: PersonRef[]; at: string };
```

**Counts.** 14 declarations (`plugin`, `kind`, `collection`, `facet`,
`operation`, `create`, `set`, `slot`, part (`fills`), `grant`, `depends`,
`watches`, `programs`, `phrases`) and 26 calls: files `create`, `read`,
`replace`, `set`, `history`; records `append`, `edit`, `redact`, `read`;
`state.get`, `state.set`; the four host file operations; `query`;
`people.find`, `people.share`; `work.start`, `work.send`, `work.stop`,
`work.list`; `call`; `net.fetch`; `notify`; `key`. That is 40 names, plus six
inside a box (`input`, `records`, `inbox`, `log.append`, `log.call`,
`log.end`). Revision 14 had 47 broker methods, about 60 concepts for a
Chat-class plugin and 312 numeric limits.

**What a plain app author learns: 15 concepts.**

1. space, 2. person, 3. file, 4. field, 5. kind (and its looks: row, card,
cell, page), 6. record, 7. per-person state, 8. collection (with facets),
9. query, 10. operation (with its form), 11. call, 12. grant,
13. work (a call, a time or a change starts it), 14. program (a run in a
box, with its inbox and log), 15. slot and part.

A ten-line list app uses six of them: file, field, kind, collection, query,
operation. Chat's three plugins use thirteen; a harness adds program.

## 2. Five plugins, written out

Line counts are non-blank TypeScript lines in each block. "Today" is the same
plugin on revision 14 (Chat at contract 14.43, on `lane/threads-ui`).

### 2.1 Reading List: 10 lines, plus 8 for reminders (today 226)

```ts
import { plugin, kind, text, url, bool, collection, create, set } from "@calternal/sdk";
export default plugin({
  id: "reading-list", name: "Reading List", version: "2.0.0", icon: "book",
  kinds: { reading: kind({ fields: { url: url(), title: text().index(), read: bool(false).index() },
    row: { title: "title", detail: ["site(url)", "created"], trailing: [{ dot: { when: "!read" } }] },
    card: { title: "title", detail: ["site(url)"] } }) },
  operations: { add: create("reading", { inputs: ["url", "title"] }), done: set("reading", { read: true }) },
  sidebar: [collection("Unread", { kind: "reading", where: "!read", sort: "-created" }),
    collection("All", { kind: "reading", sort: "-created" })],
});
```

The reminder needs code, because a time trigger is `work.start` with `at`
(the running section has no "when this field comes due"):

```ts
remind: operation({ title: "Remind me", input: { item: link(), at: datetime() },
  async handler({ item, at }, ctx) {
    await ctx.work.start({ request: ctx.request, op: "reading-list/nudge", input: { item }, at, key: `remind:${item}` });
    return {};
  } }),
nudge: operation({ title: "Nudge", input: { item: link() },
  handler: async ({ item }, ctx) => ctx.notify([ctx.me.id], { item, phrase: "remind" }) }),
// plus phrases: { remind: "Time to read this" }
```

Today's 226 lines declare a second kind for reminders, three operations in
JSON Schema, indexed-field audiences, a Bind per list with window defaults and
the chrome. Here the list app is data and never wakes; the drawing section's
own count for its interface is 7 lines.

### 2.2 Notes: 29 lines (today: a 67-line package plus the host's native notes engine)

```ts
import { plugin, kind, text, list, datetime, link, collection, facet, create, operation } from "@calternal/sdk";
export default plugin({
  id: "notes", name: "Notes", version: "2.0.0", icon: "notes", phrases: { back: "A note is back" },
  kinds: { notes: kind({
    fields: { title: text().index(), tags: list(text()).index(), snooze: datetime().index() },
    folder: "Notes",
    row: { title: "title", detail: ["updated", "excerpt"] }, card: { title: "title", detail: ["excerpt"] },
    page: { stack: [{ editor: "body" }, { panel: collection("Linked here", { linksTo: "this", sort: "-updated" }) },
      { panel: { history: "this", undo: "host.change.revert" } }] },
  }) },
  operations: {
    create: create("notes", { inputs: ["title"] }),
    snooze: operation({ title: "Snooze", input: { note: link(), until: datetime() },
      async handler({ note, until }, ctx) {
        await ctx.files.set(note, { snooze: until });
        await ctx.work.start({ request: ctx.request, op: "notes/wake", input: { note }, at: until, key: `snooze:${note}` });
        return {};
      } }),
    wake: operation({ title: "Wake", input: { note: link() },
      async handler({ note }, ctx) {
        await ctx.files.set(note, { snooze: null });
        await ctx.notify([ctx.me.id], { item: note, phrase: "back" });
        return {};
      } }),
  },
  sidebar: [collection("All notes", { kind: "notes", where: "snooze = null", sort: "-updated" }),
    facet("Tags", { kind: "notes", by: "tags" }),
    collection("Snoozed", { kind: "notes", where: "snooze > now", sort: "snooze" })],
});
```

Collections inside folders, search, rename with links that follow, Trash and
Undo are the host's (files section 1.2 and 1.7). Today the same features are a
manifest plus the host's content engine (`crates/content`, about 11,600 lines
of Rust shared by every kind), which knows what a note is.

### 2.3 Chat in three plugins: 104 lines (today 1,884 lines of source plus a 1,317-line test host)

The owner's split: the chat file (`calternal-chat`), calling agents in
(`calternal-chat-ai`), and notifications (`calternal-chat-notify`, a separate
plugin because a plugin's own writes never fire its own watches). The
composer and the harness are 2.4.

```ts
// calternal-chat: a chat is a record file shared with its people.
import { plugin, kind, text, list, person, bool, link, collection, operation, slot } from "@calternal/sdk";
const EDIT_HOURS = 48;
export default plugin({
  id: "calternal-chat", name: "Chat", version: "2.0.0", icon: "chat", grant: { people: true },
  kinds: { chats: kind({
    fields: { title: text().index(), people: list(person()).index(), archived: bool(false).index() },
    records: { fields: { mentions: list(person()).index() }, edit: "author", redact: "author-or-manager", position: "read", segment: "monthly" },
    folder: "Chats",
    row: { title: "title", leading: { avatar: "people" }, detail: ["last.excerpt"], trailing: ["last.at", { count: "me.unread", quiet: "me.muted" }] },
    page: { records: { composer: "composer", actions: {
      reply: { compose: { quote: "record" }, place: "menu" },                 // fills the composer, sends nothing
      edit: { op: "calternal-chat/edit", inputs: { chat: "this", anchor: "record" }, when: "own" },
      delete: { op: "calternal-chat/delete", inputs: { chat: "this", anchor: "record" }, when: "own", confirm: "Delete for everyone?" },
      ask: { op: "calternal-chat-ai/call", inputs: { chat: "this", range: "records.selection" } },
    } } },
  }) },
  slots: { composer: slot({ effect: "viewer", recommended: "calternal-chat-composer", gives: ["this", "draft", "quote"], may: ["calternal-chat/send"] }) },
  operations: {
    create: operation({ title: "New chat", input: { title: text(), people: list(person()) },
      async handler({ title, people }, ctx) {
        const chat = await ctx.files.create("chats", { fields: { title, people: [ctx.me.id, ...people] }, request: ctx.request });
        if (people.length) await ctx.people.share(chat, { people, role: "editor" });
        return { created: chat };
      } }),
    send: operation({ title: "Send", public: true, input: { chat: link(), text: text(), quote: text().optional() },
      async handler({ chat, text, quote }, ctx) {
        const { fields } = await ctx.files.read(chat);
        const body = quote ? `[↩](<#^${quote}>)\n${text}` : text;       // a quote is a link to the record (files 1.12)
        const mentions = fields.people.filter((p: PersonRef) => text.includes(`@${p}`));
        const { anchor } = await ctx.records.append(chat, { body, fields: { mentions }, request: ctx.request });
        await ctx.state.set(`read.${chat}`, anchor);
        return { anchor };
      } }),
    reply: operation({ title: "Agent reply", public: true, input: { chat: link(), call: text(), text: text() },
      // Called by a run from its log (running section 2.7), once per log record, so ctx.request is unique per reply.
      handler: ({ chat, call, text }, ctx) =>
        ctx.records.append(chat, { body: `[↩](<#^${call}>)\n${text}`, request: ctx.request }) }),
    edit: operation({ title: "Edit", input: { chat: link(), anchor: text(), text: text() },
      async handler({ chat, anchor, text }, ctx) {
        const [rec] = await ctx.records.read(chat, { anchors: [anchor] });
        if (Date.now() - Date.parse(rec.stamp.at) > EDIT_HOURS * 3_600_000) throw new Error("Messages can be edited for 48 hours.");
        return ctx.records.edit(chat, anchor, { body: text, expected: rec.revision });   // the host checks the author
      } }),
    delete: operation({ title: "Delete for everyone", input: { chat: link(), anchor: text() },
      handler: async ({ chat, anchor }, ctx) => (await ctx.records.redact(chat, anchor), {}) }),
    mute: operation({ title: "Mute", input: { chat: link(), on: bool() },
      handler: async ({ chat, on }, ctx) => (await ctx.state.set(`muted.${chat}`, on), {}) }),
  },
  sidebar: [collection("Pinned", { hand: true, view: "strip" }),
    collection("Chats", { kind: "chats", where: "!archived", sort: "-updated" }),
    collection("Archived", { kind: "chats", where: "archived", sort: "-updated" })],
});
```

```ts
// calternal-chat-ai: call an agent into a chat with the authors' consent.
import { plugin, link, text, list, operation, slot } from "@calternal/sdk";
export default plugin({
  id: "calternal-chat-ai", name: "Chat agents", version: "2.0.0", icon: "spark",
  depends: [{ plugin: "calternal-chat", version: "^2" }],
  grant: { kinds: { chats: "read" }, calls: ["calternal-chat/send"] },
  slots: { harness: slot({ effect: "viewer", recommended: "calternal-chat-claude-code" }) },
  operations: {
    call: operation({ title: "Ask an agent", input: { chat: link(), range: list(text()), prompt: text(), harness: text() },
      async handler({ chat, range, prompt, harness }, ctx) {
        const { anchor: call } = await ctx.call<{ anchor: string }>("calternal-chat/send", { chat, text: prompt });
        // The host asks each other author, waits up to a minute, then freezes the input (permissions 1.6).
        const run = await ctx.work.start({
          request: ctx.request, program: `${harness}/agent`, place: "ask", key: `chat:${chat}`,
          reads: { records: range.map(anchor => ({ file: chat, anchor })), consent: "ask" },
          input: { chat, call, prompt },
        });
        return { run: run.id };
      } }),
    stop: operation({ title: "Stop the agent", input: { chat: link() },
      async handler({ chat }, ctx) {
        for (const row of await ctx.work.list({ key: `chat:${chat}`, state: "running" })) await ctx.work.stop(row.id, ctx.key(row.id));
        return {};
      } }),
    steer: operation({ title: "Tell the agent", input: { chat: link(), text: text() },
      async handler({ chat, text }, ctx) {
        const [row] = await ctx.work.list({ key: `chat:${chat}`, state: "running" });
        if (!row) throw new Error("No agent is working in this chat.");
        return ctx.work.send(row.id, { type: "input", text }, ctx.request);
      } }),
  },
});
```

```ts
// calternal-chat-notify: push mentions to the people mentioned.
import { plugin, operation } from "@calternal/sdk";
export default plugin({
  id: "calternal-chat-notify", name: "Chat notifications", version: "2.0.0", icon: "bell",
  depends: [{ plugin: "calternal-chat", version: "^2" }],
  grant: { kinds: { chats: "read" } },
  phrases: { mention: "You were mentioned" },
  watches: [{ target: { kind: "chats" }, op: "calternal-chat-notify/changed" }],   // delivered per subscribing person
  operations: {
    changed: operation({ title: "Notify", input: {},
      async handler(_, ctx) {
        const { target, from } = ctx.change!;
        const fresh = await ctx.records.read(target, { after: { revision: from } });
        const mentioned = fresh.some(r => !r.withheld && r.fields.mentions?.includes(ctx.me.id) && r.stamp.person !== ctx.me.id);
        if (mentioned && !(await ctx.state.get(`muted.${target}`))) await ctx.notify([ctx.me.id], { item: target, phrase: "mention" });
        return {};
      } }),
  },
});
```

How the requirements are met:

| Requirement | Where |
| --- | --- |
| A chat is a file of records | `records` on the kind; the host writes each record's header and stamp (files 1.3) |
| People | the file's sharing (`people.share`); authors and "own" come from the stamp |
| Agents | a run of the harness's program; its replies are `calternal-chat/reply` calls from its log, stamped with the run |
| Calling an agent in with consent | `reads: { consent: "ask" }` on `work.start`; the host draws the asks, waits a minute, withholds the rest |
| Quotes as references | a link to the record's anchor; the records component draws it with the reader's rights |
| Delete for everyone | `records.redact`: scrubbed from history and search; quotes draw nothing |
| A run that survives a restart | the run's inbox and log are synced files; its reply call waits in the log and runs once after reconnect (running section 2.1 and 6.3) |
| Unread and mute | the kind's `position` key gives `me.unread`; mute is per-person state, honoured by the notify plugin running as the recipient |

### 2.4 A third-party composer and a harness

**Composer: 14 lines (today: impossible without forking Chat; its composer is 12 lines
inside Chat's 86-line description).**

```ts
import { plugin, link, text, operation } from "@calternal/sdk";
const SNIPPETS: Record<string, string> = { "/shrug": "¯\\_(ツ)_/¯", "/lgtm": "Looks good to me." };
export default plugin({
  id: "acme-composer", name: "Snippet composer", version: "1.0.0", icon: "edit",
  depends: [{ plugin: "calternal-chat", version: "^2" }],
  grant: { calls: ["calternal-chat/send"] },
  operations: {
    send: operation({ title: "Send", input: { chat: link(), text: text(), quote: text().optional() },
      handler: ({ chat, text, quote }, ctx) =>
        ctx.call("calternal-chat/send", { chat, quote, text: text.replace(/\/\w+/g, word => SNIPPETS[word] ?? word) }) }),
  },
  fills: { "calternal-chat/composer": { composer: { send: "acme-composer/send", draft: true, mention: "this.people",
    insert: Object.keys(SNIPPETS) } } },
});
```

The composer never reads the chat: it gets a host handle for the open chat
when mounted in the slot (permissions 1.5, designation rule), names it to
`calternal-chat/send`, and Chat writes under its own grant and the person's
rights.

**Harness: 23 lines (today: 70 lines for the fixture harness, which cannot run
anything, plus 692 lines of the real v1 Claude Code runtime).**

```ts
// The package: a program and nothing else.
import { plugin } from "@calternal/sdk";
export default plugin({
  id: "calternal-chat-claude-code", name: "Claude Code", version: "1.0.0", icon: "spark",
  depends: [{ plugin: "calternal-chat-ai", version: "^2" }],
  grant: { calls: ["calternal-chat/reply"] },
  fills: { "calternal-chat-ai/harness": {} },
  programs: { agent: { image: "registry.example/claude-code@sha256:4be1…", entry: "dist/entry.js",
    network: ["api.anthropic.com"], login: { name: "claude", mount: "/home/agent/.claude" }, workdir: "ask" } },
});
```

```ts
// src/entry.ts runs in the box next to `claude`, with the box API only.
import { inbox, log, input, records } from "@calternal/sdk/box";
import { spawnClaude } from "./claude.js";
const { chat, call, prompt } = input<{ chat: string; call: string; prompt: string }>();
const context = (await records()).filter(r => !r.withheld).map(r => `${r.author}: ${r.body}`).join("\n");
const claude = spawnClaude({ prompt: `${context}\n\n${prompt}` });
void (async () => { for await (const rec of inbox()) if (rec.type === "input") claude.stdin.write(rec.text); })();
for await (const event of claude.events()) {
  log.append({ type: "event", event });                                    // the working log, a synced record file
  if (event.type === "assistant_message")
    log.call("calternal-chat/reply", { chat, call, text: event.text });    // executed once, after any reconnect
}
log.end("completed");
```

`spawnClaude` (starting the program and reading its event stream) is not
counted: it is the program adapter, the same in any design.

### Size summary

| Plugin | Proposed | Today (revision 14) |
| --- | ---: | --- |
| Reading List | 10, plus 8 for reminders | 226 |
| Notes | 29 | 67-line package plus a native host engine |
| Chat: core | 53 | |
| Chat: agents (consent, Stop, steer) | 32 | |
| Chat: mention notifications | 19 | |
| Chat, all three | 104 | 1,884 source lines across two packages (1,767 after the SDK), plus a 1,317-line test host |
| Third-party composer | 14 | not possible without forking Chat |
| Harness: package plus box entry | 10 + 13 | 70 (fixture only) plus a 692-line v1 runtime |

The Chat number is bought with host work: the records component, the
per-asker unread count, `redact` with history scrubbing, consent inside
`work.start`, run files that sync, the designation rule for the composer, and
watches delivered per person. Each was Chat code, or revision 14 vocabulary,
before. Section 3 lists where even this is not enough.

## 3. Where the code got awkward

Each item: what I had to write or invent, and which section should own it.

1. **The run's input could carry other people's words before consent is
   known.** `chat-ai/call` passes the selected anchors as `reads`, the host
   freezes the consented set, and the entry gets it with `records()`. The
   permissions section has the input as refs (good); the running section's
   example passes `prompt: selection.text` (copied words), and its box API has
   no `records()`. The box needs exactly one call that returns the frozen,
   consented set, or the consent rule can be bypassed by putting text in
   `input`. *Running code and permissions.*
2. **Who picks the harness: the person per call, or a slot?** The person
   chooses an agent in the composer for each call ("Claude" or "Codex"), so
   `harness` is an input; but the permissions section makes AI a slot with
   effect `others`, picked per space. I declared a `viewer` slot and still
   pass the harness as input, which means the host must check that the input
   names a plugin filling that slot. Neither section has that check.
   *Permissions and drawing.* (Question 2.)
3. **Calling a program by name is a convention.** `program:
   \`${harness}/agent\`` assumes every harness names its program `agent`. A
   slot should be able to declare "a part here provides a program", so the
   host resolves the program from the pick. *Running code.*
4. **A plugin's own writes never fire its own watches**, so mention pushes
   need a whole extra plugin (`calternal-chat-notify`, with its own install
   grant) for what is one function. *Running code.* (Question 3.)
5. **A watch hands over revisions, not records.** The notify plugin reads
   `records.read(target, { after: { revision } })`, a read form the files
   section does not define (it has anchor and sequence ranges). *Files.*
6. **Segments.** The files section caps a record file (proposed 4 MiB) and
   has Chat roll over monthly. Every Chat call above takes one `chat` link;
   append, position, watches, sharing and the row all need "the chat", not
   this month's file. I declared `segment: "monthly"` and assumed the host
   presents the folder as one logical record file; the files section's table
   says the plugin rolls over, its example says the kind declares it.
   Anchors are unique across the space, so a quote into last month still
   resolves; appends, positions, watches and sharing still need the rule.
   *Files.* (Question 1.)
7. **Names differ between sections.** Edit and delete are `record.edit` and
   `record.redact` (files), `host.records.replace` and
   `host.records.tombstone` (permissions), and "appends a tombstone"
   (drawing). Redact scrubbing history (files) and a tombstone record
   (drawing) are not the same thing. One vocabulary is needed before the SDK.
   *All four; the integrated design.*
8. **Declarative operations cannot schedule.** `create` and `set` make the
   list app code-free, but a reminder or a snooze is a field plus
   `work.start` with `at`, so they need a handler and a worker. A time field
   declared as a trigger (`datetime().wakes("notes/wake")`) would keep them
   data. The running section rejects recurrence in the host but does not
   discuss field-driven times. *Running code.* (Question 4.)
9. **Cancelling a pending wake is a key convention.** Unsnoozing must stop
   the pending wake: `work.list({ key: \`snooze:${note}\` })` then
   `work.stop`. Every plugin that schedules invents a key scheme, and Notes
   above does not even handle unsnooze. *Running code.*
10. **Mentions are parsed from text.** `send` looks for `@<id>` in the text,
    but the composer shows names; the composer's `mention` binding has to
    write a token the core can parse, or send the mentioned people as an
    input. The drawing section says `mention: "chat.people"` and no more.
    *Drawing.*
11. **The agent's reply has no author name of its own.** The stamp says
    `run`, for Ana; the records component must show "Claude (for Ana)". That
    needs the run's program name in the stamp or a lookup from the run to the
    harness plugin's name. *Files and drawing.*
12. **Field types are a vocabulary.** `text()`, `list(person())`, `link()`
    with `.index()` cover these plugins, and the files section bounds a kind
    to 8 indexed fields. Operation inputs reuse the same builders, so authors
    learn one vocabulary; JSON Schema users lose patterns and lengths, and
    `.optional()` is invented. *Files and calls.*
13. **"Is typing" has no primitive.** The drawing section shows a working
    line from the work row (good), but typing must not be stored, so it is
    not a file, a question or work. *Running code or drawing.* (Question 6.)
14. **The box API is a second environment.** `input`, `records`, `inbox`,
    `log.append`, `log.call` and `log.end` run in the box, not in the space;
    the harness author learns them as well. It is small, but the SDK ships two
    entry points and the fake host has to fake both. *Running code.*
15. **Request IDs on every write.** Writes take a `request` for idempotence;
    the running section has the SDK derive request IDs from the work ID on a
    retried time trigger. I wrote `request: ctx.request` or `ctx.key(…)` on
    every write; the host should derive it unless given. *Calls.* (Question 5.)
16. **The chat stays open to the run only through its calls.** The harness
    holds `calls: ["calternal-chat/reply"]`, so any run of it may reply into
    any chat its person can write. The designation rule needs the run to hold
    a handle for this chat (issued at `work.start`), or a hostile harness
    posts into the person's other chats. The permissions section issues
    handles for slots, queries and returns, not for work inputs. *Permissions.*

Answered by the sections since my first draft: unread counts (the kind's
`position` key and `after_position`, files 1.8); per-person state
(`state.get` and `state.set`, files 1.10); delete for everyone with history
scrubbing (files 1.11); which reads are consent-filtered ("marked" chains,
permissions 1.6); consent waiting (inside `work.start`, permissions 1.6); who
writes an agent's reply (the run calls an operation from its log, running
2.7); finding the run for Stop (`work.list` by key, running 1.2); "own" and
quotes in the transcript (the records component, drawing 1.8); recipients'
mutes (watches delivered per subscribing person, permissions 1.9).

## 4. The developer loop

| Step | Command | What the host must provide |
| --- | --- | --- |
| Create | `calternal new my-app` writes the 10-line list app | nothing |
| Run with reload | `calternal dev` starts a local single-space host, installs the directory in development trust, opens the shell, and reinstalls on every save | a host binary that runs without a gallery or signature; a reinstall that keeps the space's files, state and work rows across package versions; a reload signal to open clients; a local machine runner for programs |
| Test | `calternal test` runs tests against an in-process fake host | the same call table and `authorize` function, in-memory files, records with stamps, index with `after_position`, state, work rows with a fake clock, scripted people, consent answers and box programs |
| Validate | `calternal check` runs the installer's validator and prints the install screen the owner will see | the installer's validator as a command (the Rust validator as a native binary per platform, since Wasm is ruled out) and the host-written install sentences |
| Publish | `calternal publish` signs, packs and opens a gallery submission with screenshots taken from fixtures | a publisher identity, one package digest, the gallery listing check |

The fake host built in #106 shows the shape: about 1,400 lines covered every
revision 14 exchange Chat used, and running Chat's tests on it found five
host-rule bugs. Against the seven primitives it should be smaller (records,
consent and work replace spans, attempts and contribution frames), but it has
to fake the box as well.

## 5. What a plugin can and cannot do

**Can:**

- Declare kinds of files with fields, and how each looks as a row, card,
  cell, page and record. Example: Reading List's `reading` kind.
- Create files, replace them, set fields, append records, and edit or redact
  records under the kind's policy. Example: Chat's messages.
- Keep small private state for each person. Example: read positions, drafts,
  mutes.
- Ask questions over any files it may read and ship collections and facets as
  the app's default sidebar. Example: Notes' "Tags".
- Share a file with people in a shared group; the host asks before widening.
  Example: a new group chat.
- Run on a call, at a time, or when files change, and run a program in a box
  on a person's machine or a hosted sandbox. Example: the Claude harness.
- Let an agent read other people's messages, once each author has agreed.
  Example: calling Claude into a group chat.
- Publish operations and call other plugins' operations its grant lists.
  Example: the composer calls `calternal-chat/send`.
- Offer slots and fill other plugins' slots. Example: a replacement composer.
- Reach declared hosts with secrets it never sees, and push one of its
  declared phrases to people who may read the item. Example: "You were
  mentioned".

**Cannot:**

- Read or write outside its grant and the person's rights, whoever asks.
  Example: the composer cannot read the chat it sends into.
- Name a file it could not reach itself to another plugin. Example: a
  composer cannot post into a chat it was never shown.
- Forge who wrote something. Example: no plugin can post as Lee.
- Read another person's private state, or learn about people outside a
  shared group. Example: Chat cannot see who muted a chat.
- Hand another person's writing to anything that can send it out without
  that person's consent. Example: Lee's messages reach Claude only if Lee
  said yes, and Claude's reply carries Lee as an author, so a later agent
  reading that reply needs Lee's consent too.
- Run code in a client except in a sealed frame, draw outside the
  catalogue, or draw or skip the host's challenge. Example: sharing always
  shows the host's sheet.
- Push arbitrary text. Example: a notification says "You were mentioned",
  never the message.
- Keep running after Stop or exceed its quotas for storage, calls, wakes and
  runs. Example: a looping watch stops at depth four.
- Reach the network except declared hosts. Example: the harness talks only
  to the model provider.

## 6. Questions for the owner

**1. Does the host hide the monthly files of a long chat?**
*What:* the files section caps one record file at a few megabytes and starts
a new file each month, so a long chat is a folder of files. *Why now:* every
record app (Chat, Mail, logs) is written against one answer. *Example:* Lee
scrolls up a two-year-old group chat and quotes last year's message; the
unread count spans this month and last. *Options:* (a) the kind declares
`segment: "monthly"` and the host presents the folder as one chat: appends go
to the current month; reads, unread counts, watches and sharing span all
months. One more host rule; plugins never see months. (b) Plugins see the
months: no host rule, but every record app writes rollover, cross-file reads
and folder sharing, slightly differently. *Recommendation:* (a).

**2. When the person picks an agent for one message, who checks the pick?**
*What:* in Chat you choose "Claude" or "Codex" in the composer for each call;
the permissions section treats AI as a part picked once per space. *Why
now:* it decides whether agents are a per-call input or a space setting.
*Example:* in the family space, Ana asks Claude, then Codex, in the same
chat. *Options:* (a) a slot lists the eligible harnesses (the space decides
which are allowed) and the person picks among them per call; the host checks
that the input names an allowed filler. Flexible; one more host check.
(b) One harness per space; the person cannot choose. Simple; no per-call
choice. *Recommendation:* (a).

**3. May a plugin's own writes wake its own watches?**
*What:* the running section never wakes a plugin for its own writes, to stop
loops. *Why now:* it decides how many plugins a feature needs. *Example:*
Chat's mention push must live in a separate notify plugin with its own
install screen. *Options:* (a) a watch may opt in to its own writes, bounded
by the existing chain depth of four: Chat stays one plugin for that feature.
(b) Keep the rule: no loops by construction, but small features become
separate plugins, each with its own install screen. *Recommendation:* (a);
the depth limit already stops loops.

**4. Do operations without code exist, and may a time field wake one?**
*What:* `create` and `set` let an app declare "add a file of this kind" or
"set this field" with no worker. *Why now:* it decides whether a small app
needs code at all. *Example:* Reading List's Add and Mark read are data; its
reminder is not, because waking at a time needs `work.start`. *Options:*
(a) two declarative forms, plus a time field that may wake an operation
(`datetime().wakes(...)`, cancelled when the field is cleared): small apps
have no code to run or review; the host interprets two forms and one wake
rule. (b) Every operation has a handler: one way to do things; every app ships
a worker. *Recommendation:* (a), limited to those forms.

**5. Does every write need an explicit request ID?**
*What:* every host write takes a request ID so that a retry does not write
twice. *Why now:* it appears on almost every line of every plugin.
*Example:* `ctx.records.append(chat, { body, request: ctx.request })`.
*Options:* (a) the host derives it from the invocation's request and a
per-invocation counter unless the author passes one: shorter code; a handler
that changes its call order on retry would write twice (the SDK's fake host
can catch that in tests). (b) Always explicit: exact, but noisy and easy to
get wrong. *Recommendation:* (a), with `ctx.key(purpose)` where the order can
change.

**6. Where does "is typing" live?**
*What:* short-lived state that must not be stored. *Why now:* without a
place, plugins will write it to files and fill history. *Example:* "Lee is
typing" in a chat. *Options:* (a) a per-file ephemeral channel the host
forwards to current readers, with a small size and rate bound: one more
concept. (b) Leave it out of the first version; Chat shows no typing.
*Recommendation:* (b) now, (a) when Chat needs it.

## Simplification opportunities

- The declarative `create` and the drawing section's derived forms overlap: a
  form derived from a create operation is the whole "New" flow.
- `facet` could be a collection with `group`, as the files section's query
  already has `group`.
- If writes derive request IDs (question 5), `ctx.key` remains only for
  out-of-order calls and `ctx.request` disappears from author code.
- The box API's `records()` and the space API's `records.read` should be one
  call with one filtering rule, so that harness authors learn one thing.
