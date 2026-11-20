# Authoring against the primitives: a paper prototype

Status: a proposal for #107, written on 2026-11-20 against the thesis
(`docs/proposals/primitives-only-thesis-2026-11-21.md` on `dev`) and the
contract owner's draft (`docs/proposals/primitives-only-host-2026-11-21.md` on
`lane/v2-host-base`). Nothing here is decided. The TypeScript is not built; it
is written to be type-shaped and plausible, so that what an author has to
write, and where it gets awkward, can be measured.

Why this matters: on the revision 14 contract, the plugin SDK (#106) removed
the hand-written test host and the build script, but Chat's own source only
went from 1,884 to 1,767 lines (about 6 percent). The rest was contract
vocabulary that no SDK can hide. This document shows what the same plugins
look like when the contract is seven primitives.

The API below uses the thesis's seven primitives. The draft's twelve map onto
them: records are part of files; index and collections are questions; people,
grants and packages are permissions; work, sandboxes and sync are running
code.

## 1. The whole author-facing API

One import, `@calternal/sdk`. Every handler receives one context `ctx`. The
host checks every call against the plugin's grant and the acting person's
rights; nothing below is a capability by itself.

```ts
// ── Package ──────────────────────────────────────────────────────────────
declare function plugin(p: {
  id: string; name: string; version: string; icon?: string;
  needs?: { plugin: string; version: string; optional?: true }[];   // dependencies
  grant: Grant;                                                     // what it asks for at install (3)
  kinds?: Kind[]; collections?: Collection[]; operations?: Operation[];
  triggers?: Trigger[]; slots?: Slot[]; parts?: Part[];
}): Plugin;

// ── 1. Files ─────────────────────────────────────────────────────────────
declare function kind<F extends Fields, R extends Fields = {}>(id: string, k: {
  fields: F;                         // frontmatter: "text" | "number" | "bool" | "date" | "datetime" | "url" | "link" | "person", "[]" for lists; "!" marks indexed
  records?: { fields?: R; edit: Policy; redact: Policy; segment?: "monthly" };  // the body is a list of host-headed records
  position?: string;                 // a state key holding the asker's last read anchor; rows gain after_position
  folder?: string;                   // where new files go
  title?: keyof F;
  row: Show<F>; card?: Show<F>; cell?: Show<F>;  // how it looks small, medium, tiny
}): Kind;
type Policy = "author" | "author-or-manager" | "writers";
interface Files {
  create(kind: string, f: { fields: object; body?: string; folder?: string; name?: string; key: string }): Promise<FileRef>;
  read(file: FileRef, at?: { revision?: string }): Promise<{ fields: any; body: string; revision: string }>;
  replace(file: FileRef, w: { fields?: object; body: string; expected: string; key: string }): Promise<{ revision: string }>;
  set(file: FileRef, fields: object, expected?: string): Promise<{ revision: string }>;
  append(file: FileRef, r: { body: string; fields?: object; key: string }): Promise<{ anchor: Anchor }>;
  edit(file: FileRef, anchor: Anchor, r: { body: string; expected: string }): Promise<{ revision: string }>;
  redact(file: FileRef, anchor: Anchor): Promise<void>;          // delete for everyone: scrubbed from history and search
  records(file: FileRef, q?: { anchors?: Anchor[]; after?: Anchor; limit?: number }): Promise<Rec[]>;
  history(file: FileRef): Promise<{ revision: string; change: string; stamp: Stamp }[]>;
}
interface State { get(key: string): Promise<unknown>; set(key: string, value: unknown): Promise<void> }  // per person and plugin
// Host operations, called like any other: "host.move", "host.trash", "host.restore", "host.revert" (one change).

// ── 2. Questions ─────────────────────────────────────────────────────────
declare function collection(id: string, c: { title: string; query: Query; hand?: true; big?: View; small?: View }): Collection;
type Query = {
  kind?: string | string[]; where?: Where; text?: string; linksTo?: FileRef | "$file";
  sort?: string; group?: string; limit?: number; count?: true;
};
interface Questions { query(q: Query): Promise<{ rows: Row[]; count?: number; next?: string }> }

// ── 3. People and permissions ────────────────────────────────────────────
type Grant = {
  read?: string[]; write?: string[];         // kinds, own kinds are implied
  call?: string[];                           // other plugins' operations
  agent?: true;                              // its reads of shared files are filtered by author consent
  outside?: { hosts?: string[]; secrets?: string[]; notify?: true };  // primitive 7
};
interface People {
  find(text: string): Promise<Person[]>;                         // only people in a shared group
  share(file: FileRef, s: { people?: PersonRef[]; group?: string; role: "reader" | "editor" }): Promise<void>;
  consent(c: { file: FileRef; anchors: Anchor[]; to: string; why: string }): Promise<{ granted: PersonRef[]; asked: PersonRef[] }>;
}

// ── 4. Running code ──────────────────────────────────────────────────────
declare const on: {
  time(kind: string, field: string, operation: string): Trigger;            // when a time field comes due
  change(q: Query & { records?: Where }, operation: string): Trigger;       // when a query's result or a file's records change
};
interface Running {
  later(at: string, operation: string, input: object, key?: string): Promise<WorkRef>;
  cancel(work: WorkRef): Promise<void>;
  run(r: { image: string; input: object; mounts?: Mount[]; output: FileRef; deadline: string; key: string }): Promise<RunRef>;
  stop(run: RunRef | WorkRef): Promise<void>;
  status(ref: RunRef | WorkRef): Promise<{ state: "waiting" | "due" | "running" | "held" | "ended"; ended?: "ok" | "failed" | "stopped" }>;
}

// ── 5. Calls ─────────────────────────────────────────────────────────────
declare function operation<I extends Schema, O extends Schema>(id: string, o: {
  title: string; input: I; output?: O;
  effect: "read" | "write" | "share" | "outside";   // drives the host's challenge, never a grant
  public?: true;                                    // callable by other plugins that declare it in grant.call
  handler: (input: SchemaType<I>, ctx: Ctx) => Promise<SchemaType<O>>;
}): Operation;
declare function create(id: string, kind: string, fields: string[]): Operation;   // declarative, no handler
declare function set(id: string, kind: string, fields: object): Operation;        // declarative, no handler
interface Calls { call<O>(operation: string | SlotRef, input: object): Promise<O> }

// ── 6. Drawing ───────────────────────────────────────────────────────────
declare function slot(id: string, s: { pick: "person" | "space"; operation?: Schema; recommended: string }): Slot;
declare function part(slot: string, p: Description | { operation: string; name: string; icon?: string }): Part;
// Description = the closed component catalogue: list, transcript, form, field, button, menu, editor,
// card, cell, history, choice, frame (sealed). Bindings: $file, $record, $form.<field>, $me.

// ── 7. The outside ───────────────────────────────────────────────────────
interface Outside {
  fetch(url: string, init?: { method?: string; headers?: Record<string, string | SecretRef>; body?: string }): Promise<Response>;
  secret(name: string): SecretRef;                                   // usable in fetch headers, never readable
  notify(to: PersonRef[], n: { title: string; body?: string; open?: FileRef }): Promise<void>;
}

// ── The context ──────────────────────────────────────────────────────────
interface Ctx extends Questions, Running, Calls, Outside {
  me: Person;                   // the acting person
  files: Files; state: State; people: People; signal: AbortSignal;
  request: string;              // stable per invocation; replays reuse it
  key(purpose: string): string; // deterministic subeffect key under request
  cause?: { file: FileRef; records: Rec[] } | { work: WorkRef };   // what fired a trigger
}
// A record as read back. The stamp lives in the host's ledger, never in the file, and cannot be forged.
type Rec = { anchor: Anchor; fields: any; body: string; stamp: Stamp; revision: string; withheld?: true; redacted?: true };
type Stamp = { person?: PersonRef; plugin: string; run?: RunRef; at: string };
```

**Counts.** 12 declarations (`plugin`, `kind`, `collection`, `operation`,
`create`, `set`, `on.time`, `on.change`, `slot`, `part`, `grant`, `needs`)
and 29 calls (files: `create`, `read`, `replace`, `set`, `append`, `edit`,
`redact`, `records`, `history`; four host file operations; `state.get`,
`state.set`; `query`; `me`, `find`, `share`, `consent`; `later`, `cancel`,
`run`, `stop`, `status`; `call`; `fetch`, `secret`, `notify`; `key`), 41
names in all. The file calls follow the files section
(`lane/design-files`, `files-and-questions.md`). Revision 14 had 47 broker methods,
about 60 concepts for a Chat-class plugin and 312 numeric limits.

**What a plain app author learns: 15 concepts.** space, person, file, field,
kind (with row, card and cell), record, per-person state, collection, query,
operation, call, grant, trigger, run, component, and slot and part (one idea:
a named place in an app that a plugin fills). A ten-line app needs six of
them: file, field, kind, collection, query, operation.

## 2. Four plugins

Line counts are the non-blank TypeScript lines in each block. "Today" is
the same plugin on revision 14.

### 2.1 Reading List: 11 lines (today 226)

```ts
import { plugin, kind, collection, create, set, on, operation } from "@calternal/sdk";

export default plugin({
  id: "reading-list", name: "Reading List", version: "2.0.0", icon: "book", grant: {},
  kinds: [kind("reading", { fields: { title: "text!", url: "url", read: "bool!", remind: "datetime!" },
    folder: "Reading", row: ["title", "url.site"], card: ["url.image", "title", "url.site"] })],
  operations: [create("reading.add", "reading", ["url", "title"]), set("reading.done", "reading", { read: true }),
    set("reading.remind", "reading", { remind: "$input" })],
  collections: [collection("unread", { title: "Unread", query: { kind: "reading", where: { read: false }, sort: "-created" } }),
    collection("all", { title: "All", query: { kind: "reading", sort: "-created" } })],
  triggers: [on.time("reading", "remind", "reading.nudge")],
}); // plus reading.nudge: one operation that calls ctx.notify (4 lines)
```

Today's 226 lines declare two kinds (a separate reminder kind), three
operations with JSON Schema, indexed-field audiences, a Bind per list with
window defaults, and the chrome. Here the reminder is a time field, and the
app is data: no worker.

### 2.2 Notes: 36 lines (today: a 67-line package plus the host's native notes engine)

```ts
import { plugin, kind, collection, create, set, on, operation, slot } from "@calternal/sdk";

const notes = kind("notes", {
  fields: { title: "text!", tags: "text[]!", pinned: "bool!", snooze: "datetime!" },
  folder: "Notes", title: "title",
  row: ["title", "excerpt"], card: ["title", "excerpt", "tags"], cell: ["title"],
});

export default plugin({
  id: "notes", name: "Notes", version: "2.0.0", icon: "notes", grant: { read: ["*"] },
  kinds: [notes],
  operations: [
    create("notes.new", "notes", ["title"]),
    set("notes.pin", "notes", { pinned: true }), set("notes.unpin", "notes", { pinned: false }),
    set("notes.snooze", "notes", { snooze: "$input" }),
    operation("notes.wake", { title: "Wake", input: { file: "file" }, effect: "write",
      async handler({ file }, ctx) {
        await ctx.files.set(file, { snooze: null });
        await ctx.notify([ctx.me.id], { title: "A note is back", open: file });
        return {};
      } }),
    operation("notes.restore", { title: "Undo this change", input: { change: "text" }, effect: "write",
      handler: async ({ change }, ctx) => ctx.call("host.revert", { change }) }),
  ],
  collections: [
    collection("all", { title: "All notes", query: { kind: "notes", where: { snooze: null }, sort: "-pinned,-updated" } }),
    collection("tags", { title: "Tags", query: { kind: "notes", group: "tags", sort: "title" }, small: "groups" }),
    collection("snoozed", { title: "Snoozed", query: { kind: "notes", where: { snooze: { gt: "now" } }, sort: "snooze" } }),
    collection("backlinks", { title: "Linked here", query: { linksTo: "$file", sort: "-updated" }, small: "list" }),
    collection("pins", { title: "Pinned", query: { kind: "notes", where: { pinned: true } }, hand: true, small: "strip" }),
  ],
  triggers: [on.time("notes", "snooze", "notes.wake")],
  slots: [slot("notes/editor", { pick: "person", recommended: "notes" })],
  parts: [
    { slot: "notes/editor", description: { type: "editor", file: "$file",
      side: [{ type: "list", collection: "backlinks" }, { type: "history", file: "$file", restore: "notes.restore" }] } },
  ],
});
```

Collections in folders, search and moves are the host's (`host.move`,
`query.text`); links are found by the index. Today the same features are a
manifest plus the host's content engine (`crates/content`, about 11,600 lines
of Rust shared by every kind), which knows what a note is.

### 2.3 Chat core: 104 lines (today 1,884 lines of source plus a 1,317-line test host)

A chat is a file of records. People are the file's sharing; agents are runs.

```ts
import { plugin, kind, collection, operation, on, slot } from "@calternal/sdk";

const chats = kind("chats", {
  fields: { title: "text!", people: "person[]!", archived: "bool!", running: "text" },
  records: { fields: { mentions: "person[]!", agent: "text", call: "text" }, edit: "author", redact: "author-or-manager", segment: "monthly" },
  position: "read", folder: "Chats", title: "title",
  row: ["title", "updated", { count: "after_position" }], card: ["title", "people", "updated"],
});
const EDIT_HOURS = 48;

export default plugin({
  id: "chat", name: "Chat", version: "2.0.0", icon: "chat", grant: {},
  kinds: [chats],
  slots: [
    slot("chat/composer", { pick: "person", recommended: "chat" }),
    slot("chat/harness", { pick: "person", recommended: "chat-claude",
      operation: { input: { chat: "link", call: "text", prompt: "text", context: "text[]" }, output: { run: "text" } } }),
  ],
  operations: [
    operation("chat.create", { title: "New chat", input: { title: "text", people: "person[]" }, effect: "share",
      async handler({ title, people }, ctx) {
        const file = await ctx.files.create("chats", { fields: { title, people: [ctx.me.id, ...people] }, key: ctx.key("chat") });
        if (people.length) await ctx.people.share(file, { people, role: "editor" });
        return { created: file };
      } }),
    operation("chat.send", { title: "Send", public: true, input: { chat: "link", text: "text", quote: "text?" }, effect: "write",
      async handler({ chat, text, quote }, ctx) {
        const { fields } = await ctx.files.read(chat);
        const body = quote ? `[↩](<#^${quote}>)\n${text}` : text;   // a quote is a link; the client draws it with the reader's rights
        const { anchor } = await ctx.files.append(chat, { body, fields: { mentions: mentioned(text, fields.people) }, key: ctx.request });
        await ctx.state.set(`read.${chat}`, anchor);
        return { anchor };
      } }),
    operation("chat.edit", { title: "Edit", input: { chat: "link", anchor: "text", text: "text" }, effect: "write",
      async handler({ chat, anchor, text }, ctx) {
        const [rec] = await ctx.files.records(chat, { anchors: [anchor] });
        if (Date.now() - Date.parse(rec.stamp.at) > EDIT_HOURS * 3600_000) throw new Error("Messages can be edited for 48 hours.");
        return ctx.files.edit(chat, anchor, { body: text, expected: rec.revision });   // the host checks the author
      } }),
    operation("chat.delete", { title: "Delete for everyone", input: { chat: "link", anchor: "text" }, effect: "write",
      handler: async ({ chat, anchor }, ctx) => (await ctx.files.redact(chat, anchor), {}) }),
    operation("chat.read", { title: "Mark read", input: { chat: "link", anchor: "text" }, effect: "write",
      handler: async ({ chat, anchor }, ctx) => (await ctx.state.set(`read.${chat}`, anchor), {}) }),
    operation("chat.mute", { title: "Mute", input: { chat: "link", on: "bool" }, effect: "write",
      handler: async ({ chat, on }, ctx) => (await ctx.state.set(`mute.${chat}`, on), {}) }),

    // Calling an agent in: ask the authors of the chosen messages, then start the harness the person picked.
    operation("chat.call", { title: "Ask an agent", input: { chat: "link", anchors: "text[]", prompt: "text", harness: "text" }, effect: "outside",
      async handler({ chat, anchors, prompt, harness }, ctx) {
        const { anchor: call } = await ctx.files.append(chat, { body: prompt, fields: { call: "waiting" }, key: ctx.key("call") });
        const { asked } = await ctx.people.consent({ file: chat, anchors, to: harness, why: prompt });
        await ctx.later(asked.length ? inOneMinute() : "now", "chat.start", { chat, call, anchors, prompt, harness }, ctx.key("start"));
        return { call };
      } }),
    operation("chat.start", { title: "Start agent", input: { chat: "link", call: "text", anchors: "text[]", prompt: "text", harness: "text" }, effect: "outside",
      async handler({ chat, call, anchors, prompt, harness }, ctx) {
        // The harness reads the context itself; the host withholds records whose authors did not consent.
        const { run } = await ctx.call<{ run: RunRef }>({ slot: "chat/harness", plugin: harness }, { chat, call, prompt, context: anchors });
        await ctx.files.set(chat, { running: run });
        return {};
      } }),
    operation("chat.reply", { title: "Post an agent reply", public: true, input: { chat: "link", call: "text", text: "text", agent: "text" }, effect: "write",
      async handler({ chat, call, text, agent }, ctx) {
        // Called from the harness's trigger on its run's log; the stamp carries that run (section 3, item 3).
        const { anchor } = await ctx.files.append(chat, { body: `[↩](<#^${call}>)\n${text}`, fields: { agent }, key: ctx.request });
        await ctx.files.set(chat, { running: null });
        return { anchor };
      } }),
    operation("chat.stop", { title: "Stop", input: { chat: "link" }, effect: "write",
      async handler({ chat }, ctx) {
        const { fields } = await ctx.files.read(chat);
        if (fields.running) await ctx.stop(fields.running);
        await ctx.files.set(chat, { running: null });
        return {};
      } }),
    operation("chat.notify", { title: "Notify mentions", input: {}, effect: "outside",
      async handler(_, ctx) {
        const { file, records } = ctx.cause as { file: FileRef; records: Rec[] };
        for (const rec of records) {
          // AWKWARD (section 3, item 2): recipients' mutes are in their private state, which this run cannot read.
          const to = (rec.fields.mentions ?? []).filter((p: PersonRef) => p !== rec.stamp.person);
          if (to.length) await ctx.notify(to, { title: "You were mentioned", body: rec.body.slice(0, 120), open: file });
        }
        return {};
      } }),
  ],
  triggers: [on.change({ kind: "chats", records: { mentions: { exists: true } } }, "chat.notify")],
  collections: [
    collection("all", { title: "All", query: { kind: "chats", where: { archived: false }, sort: "-updated" } }),
    collection("pinned", { title: "Pinned", query: { kind: "chats" }, hand: true, small: "strip" }),
    collection("archived", { title: "Archived", query: { kind: "chats", where: { archived: true }, sort: "-updated" } }),
  ],
  parts: [
    { slot: "chat/composer", description: { type: "form", fields: [{ id: "text", input: "multiline", draft: "draft.$file", mentions: "$file.people" }],
      submit: { call: "chat.send", input: { chat: "$file", text: "$form.text" } } } },
    { slot: "app/main", description: { type: "transcript", file: "$file", read: "chat.read",
      actions: [{ title: "Reply", fill: "chat/composer" }, { title: "Edit", call: "chat.edit", own: true },
        { title: "Delete for everyone", call: "chat.delete", own: true }, { title: "Ask an agent", call: "chat.call", select: "anchors" }],
      footer: { slot: "chat/composer" } } },
  ],
});

function mentioned(text: string, people: PersonRef[]): PersonRef[] {
  return people.filter(person => text.includes(`@${person}`));
}
function inOneMinute() {
  return new Date(Date.now() + 60_000).toISOString();
}
```

What this covers: people (sharing), agents (a run through the picked
harness), calling an agent in with consent (the host filters what the
harness can read; Chat waits a minute for answers), quotes as references
(a link to the record's anchor, drawn with the reader's rights), delete for
everyone (`redact`, scrubbed from history and search), an agent run that survives a restart (the run keeps appending to
its own file on the machine; the harness's change trigger posts the reply
whenever that file syncs, so nothing in Chat holds a connection), and
mentions, and unread counts (`position: "read"` gives each row an
`after_position` count for the asker). Typing indicators and honouring other
people's mutes are not covered; see section 3.

### 2.4 A third-party composer and a harness

**Composer: 19 lines (today: impossible without forking Chat; its composer
is 12 lines inside Chat's 86-line description).**

```ts
import { plugin, operation } from "@calternal/sdk";

const SNIPPETS: Record<string, string> = { "/shrug": "¯\\_(ツ)_/¯", "/lgtm": "Looks good to me." };

export default plugin({
  id: "acme-composer", name: "Snippet composer", version: "1.0.0", icon: "edit",
  needs: [{ plugin: "chat", version: "^2" }],
  grant: { call: ["chat.send"] },
  operations: [
    operation("acme.send", { title: "Send", input: { chat: "file", text: "long" }, effect: "write",
      handler: ({ chat, text }, ctx) =>
        ctx.call("chat.send", { chat, text: text.replace(/\/\w+/g, word => SNIPPETS[word] ?? word) }) }),
  ],
  parts: [{
    slot: "chat/composer",
    description: { type: "form",
      fields: [{ id: "text", input: "multiline", draft: true, mentions: "$file.people" }],
      menu: Object.keys(SNIPPETS).map(word => ({ title: word, insert: { field: "text", text: word } })),
      submit: { call: "acme.send", input: { chat: "$file", text: "$form.text" } } },
  }],
});
```

**Harness: 33 lines (today: 70 lines for the fixture harness, which cannot
run anything, plus 692 lines of the real v1 Claude Code runtime).**

```ts
import { plugin, kind, operation, on } from "@calternal/sdk";

export default plugin({
  id: "chat-claude", name: "Claude Code", version: "1.0.0", icon: "spark",
  needs: [{ plugin: "chat", version: "^2" }],
  grant: { read: ["chats"], call: ["chat.reply"], agent: true },
  kinds: [kind("claude-run", { fields: { chat: "link", call: "text" }, folder: "$me/Claude runs", row: ["created"],
    records: { fields: { kind: "text!" }, edit: "writers", redact: "author-or-manager" } })],
  parts: [{ slot: "chat/harness", operation: "claude.start", name: "Claude Code", icon: "spark" }],
  operations: [
    operation("claude.start", { title: "Run Claude Code", input: { chat: "link", call: "text", prompt: "text", context: "text[]" },
      effect: "outside",
      async handler({ chat, call, prompt, context }, ctx) {
        // Consent-filtered: records whose authors did not consent come back withheld.
        const records = await ctx.files.records(chat, { anchors: context });
        const log = await ctx.files.create("claude-run", { fields: { chat, call }, key: ctx.key("log") });
        const run = await ctx.run({
          image: "ghcr.io/calternal/claude-code@sha256:4f…", key: ctx.key("run"), deadline: "2h", output: log,
          input: { prompt, context: records.filter(r => !r.withheld).map(r => ({ by: r.stamp.person, text: r.body })) },
          mounts: [{ picked: "login", as: "/home/agent/.claude" }],
        });
        return { run };
      } }),
    operation("claude.deliver", { title: "Post the reply", input: {}, effect: "write",
      async handler(_, ctx) {
        const { file, records } = ctx.cause as { file: FileRef; records: Rec[] };
        const final = records.find(r => r.fields.kind === "final");
        if (!final) return {};
        const { fields } = await ctx.files.read(file);
        return ctx.call("chat.reply", { chat: fields.chat, call: fields.call, text: final.body, agent: "Claude Code" });
      } }),
  ],
  triggers: [on.change({ kind: "claude-run", records: { kind: "final" } }, "claude.deliver")],
});
```

### Size summary

| Plugin | Proposed | Today (revision 14) |
| --- | ---: | --- |
| Reading List | 11 (+4 for the reminder nudge) | 226 |
| Notes | 36 | 67-line package plus a native host engine |
| Chat core | 104 | 1,884 source lines across two packages (1,767 after the SDK), plus a 1,317-line test host |
| Third-party composer | 19 | not possible without forking Chat |
| Harness | 33 | 70 (fixture only) plus a 692-line v1 runtime |

The Chat number hides real costs: it uses a dozen field types, a transcript
component, slots with operation signatures, consent, `redact`, the
per-asker position count, and change triggers carrying a cause. Each of those is host work that today lives in
Chat or in the revision 14 vocabulary. Section 3 lists where the line count
is bought with something the primitives do not yet have.

## 3. Where the code got awkward

Each item: what I had to write or invent, and which section owns it. The
files section (`lane/design-files`, first version, read at 18:00 UTC) already
answers four items from my first draft; they are listed at the end.

1. **A segmented chat is several files.** The files section caps a record
   file (proposed 4 MiB) and has Chat roll over monthly
   (`Chats/<chat>/2026-12.md`). Every Chat operation above takes one `chat`
   link: `append`, `position`, `on.change`, sharing and the row all need "the
   chat", not "this month's file". Either the host presents a segmented kind
   as one logical file (append goes to the current segment, records and
   positions span segments, sharing is on the folder), or every record app
   re-implements rollover and cross-segment reads. I wrote the first; the
   files section does not yet say which. *Files lane.*
2. **A plugin cannot honour another person's mute.** `chat.notify` runs for
   the sender (or for no person) and cannot read the recipients' private
   state. Either the host applies a per-recipient filter (a notification is
   dropped when the recipient's state for this plugin says `mute.<file>`),
   or mutes go into a shared file, which tells everyone who muted whom.
   *Permissions lane (notifications are primitive 7).*
3. **Who acts in a trigger, and does the stamp carry the run?** `on.change`
   fires when the harness's run log changes. I assumed the trigger acts for
   the person whose action caused the change and that the stamp of anything
   it writes carries the cause (the run), so `chat.reply` called from the
   harness's trigger is stamped `run: <run>`. The files section instead has
   the run append the reply itself (`record.append` by the run). That needs
   the run, a program in a sandbox, to call the host API from inside the
   sandbox with a grant on the chat. Either is possible; they are different
   security shapes, and the running-code section has to pick one. *Running
   code and permissions lanes.*
4. **Trigger delivery must be durable, at least once, with a host cursor.**
   The restart story depends on the host remembering which log records a
   trigger has delivered and re-delivering after a crash. If the host does
   not keep that cursor, every plugin keeps one in its state (a convention
   every app would reinvent). Request keys make the redelivery harmless.
   *Running code lane.*
5. **Calling "the part the person picked".** `ctx.call({ slot, plugin })` is
   invented: Chat calls the picked harness's operation with a signature the
   slot declares. The draft has extension points but no call form for them.
   *Permissions and calls lane.*
6. **Consent waiting is a timer plus guesswork.** `people.consent` asks the
   authors; Chat then waits a fixed minute with `ctx.later`. A trigger on
   the answers (`on.change` over grants) would be exact, which needs grants
   to be queryable or a trigger source of their own. *Permissions lane.*
7. **Which reads are consent-filtered.** I invented `grant.agent: true`:
   every read of a shared file by such a plugin is filtered by author
   consent. The thesis says "a run acting for an agent", but the harness
   plugin's handler reads the context and passes it into the run's input, so
   the filter must apply to the plugin's handler too. *Permissions lane.*
8. **A private run log has no folder.** The harness wants its log readable
   only by the person who called it (host draft question 4b). Per-person
   state is a hidden file per plugin, not a folder of record files, so I
   invented `folder: "$me/Claude runs"`. Files need a per-person folder (or
   a "private to creator" share at create). *Files and permissions lanes.*
9. **The `running` field is a convention.** Chat stores the current run in a
   field so `chat.stop` can find it, and must clear it on reply and on Stop.
   A query over runs by target file would remove the field and two writes
   per call. *Running code lane.*
10. **Quotes are a link convention the drawing must recognise.** Chat writes
    `[↩](<#^anchor>)` at the top of a reply. The transcript draws it as a
    quote card with the reader's rights only if the catalogue knows that a
    leading anchor link means "quote". *Drawing lane.*
11. **Descriptions bind to state and stamps.** The composer binds its draft
    to the state key `draft.$file`; the transcript shows Edit and Delete only
    on records whose stamp names the viewer (`own: true`). Both need the
    drawing catalogue to read per-person state and stamps on the host side.
    *Drawing lane.*
12. **Declarative operations are a second way to write an operation.**
    `create` and `set` make Reading List and Notes code-free, but they are a
    tiny language (`$input`, field lists) that the host interprets. The files
    section's Reading List example uses the same idea in YAML. *Calls lane.*
13. **Field types are a vocabulary.** `"text!"`, `"person[]"`, `"link"` cover
    these plugins, with `!` for indexed fields (the files section allows 8 per
    kind). Operation inputs reuse the same strings, so authors learn one
    vocabulary, but JSON Schema users lose expressiveness (patterns, lengths).
    *Files and calls lanes.*
14. **Typing and "working" indicators have no primitive.** They must not be
    stored, so they are not files, questions or runs. *Running code or
    drawing lane.*
15. **Mentions are parsed by the plugin.** `mentioned()` matches `@<id>` in
    the text, but the composer shows names. The form's `mentions` binding
    must write a parseable token (or record fields directly). *Drawing lane.*

Answered by the files section since my first draft: unread counts (a
kind's `position` key gives rows `after_position`, the engine's only join);
per-person state (`state.get` and `state.set`, buffered, one file per person
and plugin); delete for everyone (`record.redact` scrubs history and
search); record structure (host-written headers, Markdown bodies, optional
indexed record fields, edit and redact policies on the kind).

## 4. The developer loop

| Step | Command | What the host must provide |
| --- | --- | --- |
| Create | `calternal new my-app` writes a 10-line plugin with one kind, one collection and one declarative operation | nothing |
| Run with reload | `calternal dev` starts a local single-space host, installs the directory in development trust (no signature), opens the shell, and on every save rebuilds and reinstalls | a host binary that runs locally without a gallery, a development-trust install, a reinstall that keeps the space's files and work table across package versions, and a reload signal to the shell |
| Test | `calternal test` runs tests against an in-process fake host | the same validator and grant check as the server, in-memory files, records, index and work table, scripted people, consent answers, runs and clock |
| Validate | `calternal check` runs exactly the installer's validator | the installer's validator shipped as a command (the Rust validator as a native binary per platform, since Wasm is ruled out) |
| Publish | `calternal publish` signs, packs and opens a gallery submission with screenshots taken from fixtures | a signing identity, a package format with one digest, and the gallery listing check |

The fake host built in #106 shows the shape: about 1,400 lines covered every
revision 14 exchange Chat used. Against the seven primitives it should be
smaller, because records, consent and runs replace spans, attempts and
contribution frames.

## 5. What a plugin can and cannot do

**Can:**

- Declare kinds of files with typed fields, and say how each looks as a row,
  card and cell. Example: Reading List's `reading` kind.
- Create, replace, set fields on and append records to files of kinds it may
  write, edit or redact records under the kind's policy, and read history.
  Example: Notes undoes a change.
- Keep small private state per person. Example: Chat's read positions and
  drafts.
- Ask questions over any files it may read, and offer them as collections
  with big and small views. Example: Notes' "Linked here".
- Share a file with people in a shared group. Example: a new group chat.
- Ask authors to let an agent plugin read their messages. Example: calling
  Claude into a chat.
- Run on a call, at a time, or when files change, and run long programs on a
  machine whose output lands in a file. Example: the harness.
- Publish operations for other plugins, and call theirs if the install grant
  allows. Example: the composer calls `chat.send`.
- Define slots others can fill, and fill other plugins' slots. Example: the
  composer.
- Reach declared hosts with secrets it cannot read, and notify people.
  Example: a mail plugin fetching from a mail server.

**Cannot:**

- Read or write any file outside its grant, even when another plugin asks it
  to. Example: the composer cannot read chats.
- Forge who wrote something: the host stamps every write. Example: a plugin
  cannot post "as Alice".
- Read another person's private state, or learn about people outside a
  shared group. Example: Chat cannot see who muted a chat.
- Give an agent plugin text whose author has not agreed. Example: the
  harness gets withheld records for non-consenting authors.
- Run code in a client, except in a sealed frame. Example: no custom
  JavaScript in the composer.
- Draw outside the catalogue or hide the host's challenge for risky
  effects. Example: sharing a file always shows the host's confirmation.
- Keep running after Stop, or exceed its storage, compute, call and wake-up
  quotas. Example: a looping trigger is cut off.
- Reach the network except declared hosts. Example: the harness image only
  talks to the model provider.

## 6. Questions for the owner

**1. Does the host hide segments of a long record file?**
*What:* the files section caps a record file (proposed 4 MiB) and starts a
new file each month, so a long chat is a folder of files. *Why now:* every
record app (Chat, Mail, logs) is written against one answer. *Example:* Lee
opens a two-year-old group chat, scrolls up, and quotes a message from last
year; the unread count spans this month and last. *Options:* (a) the kind
declares `segment: "monthly"` and the host presents the folder as one
logical record file: append goes to the current segment, reads, positions,
triggers and sharing span all of them; the host carries one more rule, and
plugins never see segments. (b) Plugins see the segments: no host rule, but
every record app writes rollover, cross-file reads, cross-file positions and
folder sharing, and gets them subtly different. *Recommendation:* (a).

**2. When an agent's reply is written, whose write is it?**
*What:* an agent run produces an answer; something must append it to the
chat with a stamp that says "Claude, for Lee". *Why now:* consent, the
author shown on the message, and what the run is allowed to touch all depend
on it. *Example:* Lee calls Claude into a group chat; its reply appears
under Lee's message. *Options:* (a) the run appends to the chat itself
through the host, with a grant on that one chat for the run's lifetime: the
stamp is exact, but a sandboxed program now holds a write grant on a shared
file. (b) The run only writes its own log; the harness plugin's change
trigger posts the reply, and the host carries the cause so the stamp still
names the run: the program touches nothing shared, but triggers must carry
causes across calls. *Recommendation:* (b); it keeps every sandbox writing
only its own files, which is the simpler thing to contain.

**3. Do operations without code exist?**
*What:* `create` and `set` let an app declare "add a file of this kind" or
"set this field" with no worker. *Why now:* it decides whether the ten-line
app needs a sandbox at all. *Example:* Reading List's Add and Mark read.
*Options:* (a) yes, two declarative forms with `$input` substitution: small
apps have no code to run or review; the host interprets a tiny language.
(b) No, every operation is a handler: one way to do things; every app ships
a worker. *Recommendation:* (a), limited to those two forms.

**4. How does an app call the part a person picked?**
*What:* a slot is a named place another plugin can fill, such as Chat's
harness. *Why now:* agents, composers and notifications all depend on it.
*Example:* Chat starts whichever harness Alice picked. *Options:* (a) a slot
declares an operation signature and `call({ slot, plugin })` reaches the
filling plugin's operation with the caller's and the person's rights: one
mechanism for every pluggable behaviour. (b) Slots are drawing-only, and
behaviour goes through public operations named by convention: fewer host
rules, but every app invents its own naming. *Recommendation:* (a).

**5. Which plugins are "agents" for the consent rule?**
*What:* the rule is that agents read other people's writing only with
consent. *Why now:* the host must know where to apply the filter.
*Example:* the Claude harness's handler reads chat messages and passes them
to a run. *Options:* (a) a plugin declares `agent: true` in its grant, and
every read it makes of a shared file is consent-filtered: simple and hard to
bypass, but a plugin could avoid the flag (the installer must require it for
anything that starts runs with model access). (b) Only runs are filtered:
precise, but a plugin can read in its handler and pass the text into the run.
*Recommendation:* (a), with the installer requiring the flag for any plugin
that declares a run image.

**6. Where do typing and "working" indicators live?**
*What:* short-lived state that must not be stored. *Why now:* without a
place, plugins will write it to files, which fills history. *Example:* "Lee
is typing" in a chat. *Options:* (a) a per-file ephemeral channel the host
forwards to current readers, with a small size and rate bound: one more
concept. (b) Leave it out of the first version: simpler, Chat shows no
typing. *Recommendation:* (b) for the first version, (a) when Chat needs it.

## Simplification opportunities

- Records could be the only write besides replace: Notes' history and
  Chat's edits both read as records, which would make `history` a records
  query.
- `on.time` over a field and `later` overlap; if the host indexes time
  fields, `later` may only be needed for computed times.
- The `running` field and `status` overlap (item 10 above).
