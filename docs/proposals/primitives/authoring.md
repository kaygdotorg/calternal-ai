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
  fields: F;                         // frontmatter, typed: "text" | "long" | "url" | "flag" | "number" | "time" | "person" | "file" | "people" | …
  records?: R;                       // present: the body is a list of records with these fields
  folder?: string;                   // where new files go, e.g. "Notes" or "Chats/{created:date} {title}"
  private?: "person";                // one file per person, readable only by that person
  title?: keyof F;
  row: Show<F>; card?: Show<F>; cell?: Show<F>;  // how it looks small, medium, tiny
  record?: Show<R>;                  // how one record looks (records kinds only)
}): Kind;
interface Files {
  create(kind: string, f: { fields: object; body?: string; name?: string; key?: string }): Promise<FileRef>;
  read(file: FileRef, at?: { revision?: string }): Promise<{ fields: any; body: string; revision: string; origin: Origin }>;
  write(file: FileRef, w: { fields?: object; body?: string; expected: string }): Promise<{ revision: string }>;
  append(file: FileRef, r: { fields?: object; body?: string; replaces?: Anchor; erase?: true; key?: string }): Promise<{ anchor: Anchor; revision: string }>;
  records(file: FileRef, q?: { after?: Anchor; anchors?: Anchor[]; limit?: number }): Promise<{ records: Rec[]; next?: Anchor }>;
  history(file: FileRef): Promise<{ revision: string; at: string; origin: Origin }[]>;
}
// Host operations, called like any other: "host.move", "host.trash", "host.restore", "host.revert".

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
  files: Files; people: People; signal: AbortSignal;
  request: string;              // stable per invocation; replays reuse it
  key(purpose: string): string; // deterministic subeffect key under request
  cause?: { file: FileRef; records: Rec[] } | { work: WorkRef };   // what fired a trigger
}
// A record as read back: the host's origin stamp is part of it and cannot be forged.
type Rec = { anchor: Anchor; fields: any; body: string; origin: Origin; replaces?: Anchor; erased?: true };
type Origin = { person?: PersonRef; plugin?: string; run?: RunRef; at: string };
```

**Counts.** 12 declarations (`plugin`, `kind`, `collection`, `operation`,
`create`, `set`, `on.time`, `on.change`, `slot`, `part`, `grant`, `needs`)
and 25 calls (`create`, `read`, `write`, `append`, `records`, `history`, the
four host file operations, `query`, `me`, `find`, `share`, `consent`,
`later`, `cancel`, `run`, `stop`, `status`, `call`, `fetch`, `secret`,
`notify`, `key`), 37 names in all. Revision 14 had 47 broker methods,
about 60 concepts for a Chat-class plugin and 312 numeric limits.

**What a plain app author learns: 15 concepts.** space, person, file, field,
kind (with row, card and cell), record, collection, query, operation, call,
grant, trigger, run, component, slot and part (one idea: a named place in an
app that a plugin fills).

## 2. Four plugins

Line counts are the non-blank TypeScript lines in each block. "Today" is
the same plugin on revision 14.

### 2.1 Reading List: 11 lines (today 226)

```ts
import { plugin, kind, collection, create, set, on, operation } from "@calternal/sdk";

export default plugin({
  id: "reading-list", name: "Reading List", version: "2.0.0", icon: "book", grant: {},
  kinds: [kind("reading", { fields: { title: "text", url: "url", read: "flag", remind: "time" },
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

### 2.2 Notes: 37 lines (today: a 67-line package plus the host's native notes engine)

```ts
import { plugin, kind, collection, create, set, on, operation, slot } from "@calternal/sdk";

const notes = kind("notes", {
  fields: { title: "text", tags: "text[]", pinned: "flag", snooze: "time" },
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
        const { revision } = await ctx.files.read(file);
        await ctx.files.write(file, { fields: { snooze: null }, expected: revision });
        await ctx.notify([ctx.me.id], { title: "A note is back", open: file });
        return {};
      } }),
    operation("notes.restore", { title: "Restore this version", input: { file: "file", revision: "text" }, effect: "write",
      handler: async ({ file, revision }, ctx) => ctx.call("host.revert", { file, revision }) }),
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

### 2.3 Chat core: 136 lines (today 1,884 lines of source plus a 1,317-line test host)

A chat is a file of records. People are the file's sharing; agents are runs.

```ts
import { plugin, kind, collection, operation, on, slot } from "@calternal/sdk";

const message = { text: "long", reply: "anchor", quote: "ref", mentions: "people", agent: "text", call: "text" };
const chats = kind("chats", {
  fields: { title: "text", people: "people", archived: "flag", running: "text" },
  records: message, folder: "Chats/{created:date} {title}", title: "title",
  row: ["title", "updated", { count: "unread" }], card: ["title", "people", "updated"],
  record: { author: "origin", text: "text", reply: "reply", quote: "quote" },
});
// One private file per person: read positions, drafts, mutes (thesis question 2, option a).
const mine = kind("chat-mine", { fields: { read: "map", draft: "map", muted: "file[]" }, private: "person", row: [] });

const EDIT_HOURS = 48;
async function mineFile(ctx: Ctx) {
  const [row] = (await ctx.query({ kind: "chat-mine", limit: 1 })).rows;
  return row?.file ?? ctx.files.create("chat-mine", { fields: { read: {}, draft: {}, muted: [] }, key: "mine" });
}
async function own(ctx: Ctx, chat: FileRef, anchor: Anchor) {
  const [rec] = (await ctx.files.records(chat, { anchors: [anchor] })).records;
  if (!rec || rec.erased || rec.origin.person !== ctx.me.id) throw new Error("Only the author can change this message.");
  return rec;
}

export default plugin({
  id: "chat", name: "Chat", version: "2.0.0", icon: "chat",
  grant: { call: ["host.trash"] },
  kinds: [chats, mine],
  slots: [
    slot("chat/composer", { pick: "person", recommended: "chat" }),
    slot("chat/harness", { pick: "person", recommended: "chat-claude",
      operation: { input: { chat: "file", call: "anchor", prompt: "long", context: "anchor[]" }, output: { run: "run" } } }),
  ],
  operations: [
    operation("chat.create", { title: "New chat", input: { title: "text", people: "people" }, effect: "share",
      async handler({ title, people }, ctx) {
        const file = await ctx.files.create("chats", { fields: { title, people: [ctx.me.id, ...people] }, key: ctx.key("chat") });
        if (people.length) await ctx.people.share(file, { people, role: "editor" });
        return { created: file };
      } }),
    operation("chat.send", { title: "Send", public: true, input: { chat: "file", text: "long", reply: "anchor?", quote: "ref?" }, effect: "write",
      async handler({ chat, text, reply, quote }, ctx) {
        const mentions = mentioned(text, (await ctx.files.read(chat)).fields.people);
        const { anchor } = await ctx.files.append(chat, { fields: { text, reply, quote, mentions }, key: ctx.request });
        await markRead(ctx, chat, anchor);
        return { anchor };
      } }),
    operation("chat.edit", { title: "Edit", input: { chat: "file", anchor: "anchor", text: "long" }, effect: "write",
      async handler({ chat, anchor, text }, ctx) {
        const rec = await own(ctx, chat, anchor);
        if (Date.now() - Date.parse(rec.origin.at) > EDIT_HOURS * 3600_000) throw new Error("Messages can be edited for 48 hours.");
        return ctx.files.append(chat, { replaces: anchor, fields: { ...rec.fields, text }, key: ctx.request });
      } }),
    operation("chat.delete", { title: "Delete for everyone", input: { chat: "file", anchor: "anchor" }, effect: "write",
      async handler({ chat, anchor }, ctx) {
        await own(ctx, chat, anchor);
        return ctx.files.append(chat, { replaces: anchor, erase: true, key: ctx.request });  // quotes of it now draw "deleted"
      } }),
    operation("chat.read", { title: "Mark read", input: { chat: "file", anchor: "anchor" }, effect: "write",
      handler: async ({ chat, anchor }, ctx) => (await markRead(ctx, chat, anchor), {}) }),
    operation("chat.mute", { title: "Mute", input: { chat: "file", on: "flag" }, effect: "write",
      async handler({ chat, on }, ctx) {
        const file = await mineFile(ctx);
        const { fields, revision } = await ctx.files.read(file);
        const muted = on ? [...new Set([...fields.muted, chat])] : fields.muted.filter((f: FileRef) => f !== chat);
        await ctx.files.write(file, { fields: { muted }, expected: revision });
        return {};
      } }),

    // Calling an agent in: ask the authors of the chosen messages, then start the person's harness.
    operation("chat.call", { title: "Ask an agent", input: { chat: "file", anchors: "anchor[]", prompt: "long", harness: "plugin" }, effect: "outside",
      async handler({ chat, anchors, prompt, harness }, ctx) {
        const { anchor: call } = await ctx.files.append(chat, { fields: { text: prompt, call: "waiting" }, key: ctx.key("call") });
        const { asked } = await ctx.people.consent({ file: chat, anchors, to: harness, why: prompt });
        await ctx.later(asked.length ? in1Minute() : "now", "chat.start", { chat, call, anchors, prompt, harness }, ctx.key("start"));
        return { call };
      } }),
    operation("chat.start", { title: "Start agent", input: { chat: "file", call: "anchor", anchors: "anchor[]", prompt: "long", harness: "plugin" }, effect: "outside",
      async handler({ chat, call, anchors, prompt, harness }, ctx) {
        // The harness reads the context itself; the host filters it by each author's consent.
        const { run } = await ctx.call<{ run: RunRef }>({ slot: "chat/harness", plugin: harness }, { chat, call, prompt, context: anchors });
        const { revision } = await ctx.files.read(chat);
        await ctx.files.write(chat, { fields: { running: run }, expected: revision });
        return {};
      } }),
    operation("chat.reply", { title: "Post an agent reply", public: true, input: { chat: "file", call: "anchor", text: "long", agent: "text" }, effect: "write",
      async handler({ chat, call, text, agent }, ctx) {
        // Called by a harness from its own run's trigger; the host stamps origin.run from that cause.
        if (!ctx.cause || !("file" in ctx.cause)) throw new Error("Replies come from an agent run.");
        return ctx.files.append(chat, { fields: { text, agent, reply: call }, key: ctx.request });
      } }),
    operation("chat.stop", { title: "Stop", input: { chat: "file" }, effect: "write",
      async handler({ chat }, ctx) {
        const { fields, revision } = await ctx.files.read(chat);
        if (fields.running) await ctx.stop(fields.running);
        await ctx.files.write(chat, { fields: { running: null }, expected: revision });
        return {};
      } }),
    operation("chat.notify", { title: "Notify mentions", input: {}, effect: "outside",
      async handler(_, ctx) {
        const { file, records } = ctx.cause as { file: FileRef; records: Rec[] };
        const muted = await mutedBy(ctx, file);
        for (const rec of records) {
          const to = (rec.fields.mentions ?? []).filter((p: PersonRef) => p !== rec.origin.person && !muted.includes(p));
          if (to.length) await ctx.notify(to, { title: "You were mentioned", body: rec.fields.text.slice(0, 120), open: file });
        }
        return {};
      } }),
  ],
  triggers: [on.change({ kind: "chats", records: { mentions: { not: [] } } }, "chat.notify")],
  collections: [
    collection("all", { title: "All", query: { kind: "chats", where: { archived: false }, sort: "-updated" } }),
    collection("pinned", { title: "Pinned", query: { kind: "chats" }, hand: true, small: "strip" }),
    collection("people", { title: "People", query: { kind: "chats", where: { archived: false, "people.count": { gt: 1 } }, sort: "-updated" } }),
    collection("archived", { title: "Archived", query: { kind: "chats", where: { archived: true }, sort: "-updated" } }),
  ],
  parts: [
    { slot: "chat/composer", description: { type: "form", fields: [{ id: "text", input: "multiline", draft: true, mentions: "$file.people" }],
      submit: { call: "chat.send", input: { chat: "$file", text: "$form.text" } } } },
    { slot: "app/main", description: { type: "transcript", file: "$file", read: "chat.read",
      actions: [{ title: "Reply", fill: "chat/composer" }, { title: "Edit", call: "chat.edit", own: true },
        { title: "Delete for everyone", call: "chat.delete", own: true }, { title: "Ask an agent", call: "chat.call", select: "anchors" }],
      footer: { slot: "chat/composer" } } },
  ],
});

// Helpers.
async function markRead(ctx: Ctx, chat: FileRef, anchor: Anchor) {
  const file = await mineFile(ctx);
  const { fields, revision } = await ctx.files.read(file);
  if (fields.read[chat] === anchor) return;
  await ctx.files.write(file, { fields: { read: { ...fields.read, [chat]: anchor } }, expected: revision });
}
async function mutedBy(ctx: Ctx, chat: FileRef): Promise<PersonRef[]> {
  return [];   // AWKWARD (section 3, item 2): another person's private file cannot be read, so mutes cannot be honoured here.
}
function mentioned(text: string, people: PersonRef[]): PersonRef[] {
  return people.filter(person => text.includes(`@${person}`));
}
function in1Minute() {
  return new Date(Date.now() + 60_000).toISOString();
}
```

What this covers: people (sharing), agents (a run through the picked
harness), calling an agent in with consent (the host filters what the
harness can read; Chat waits a minute for answers), quotes as references
(`quote: "ref"`, drawn with the reader's grants), delete for everyone
(`erase`), an agent run that survives a restart (the run keeps appending to
its own file on the machine; the harness's change trigger posts the reply
whenever that file syncs, so nothing in Chat holds a connection), and
mentions. Typing indicators, unread counts in rows and per-person mutes are
not covered properly; see section 3.

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
  kinds: [kind("claude-run", { fields: { chat: "file", call: "anchor" }, records: { kind: "text", text: "long" },
    private: "person", row: ["created"] })],
  parts: [{ slot: "chat/harness", operation: "claude.start", name: "Claude Code", icon: "spark" }],
  operations: [
    operation("claude.start", { title: "Run Claude Code", input: { chat: "file", call: "anchor", prompt: "long", context: "anchor[]" },
      effect: "outside",
      async handler({ chat, call, prompt, context }, ctx) {
        // Consent-filtered: records whose authors did not consent come back erased.
        const { records } = await ctx.files.records(chat, { anchors: context });
        const log = await ctx.files.create("claude-run", { fields: { chat, call }, key: ctx.key("log") });
        const run = await ctx.run({
          image: "ghcr.io/calternal/claude-code@sha256:4f…", key: ctx.key("run"), deadline: "2h", output: log,
          input: { prompt, context: records.filter(r => !r.erased).map(r => ({ by: r.origin.person, text: r.fields.text })) },
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
        return ctx.call("chat.reply", { chat: fields.chat, call: fields.call, text: final.fields.text, agent: "Claude Code" });
      } }),
  ],
  triggers: [on.change({ kind: "claude-run", records: { kind: "final" } }, "claude.deliver")],
});
```

### Size summary

| Plugin | Proposed | Today (revision 14) |
| --- | ---: | --- |
| Reading List | 11 (+4 for the reminder nudge) | 226 |
| Notes | 37 | 67-line package plus a native host engine |
| Chat core | 136 | 1,884 source lines across two packages (1,767 after the SDK), plus a 1,317-line test host |
| Third-party composer | 19 | not possible without forking Chat |
| Harness | 33 | 70 (fixture only) plus a 692-line v1 runtime |

The Chat number hides real costs: it uses a dozen field types, a transcript
component, slots with operation signatures, consent, `erase`, and change
triggers carrying a cause. Each of those is host work that today lives in
Chat or in the revision 14 vocabulary. Section 3 lists where the line count
is bought with something the primitives do not yet have.

## 3. Where the code got awkward

Each item: what I had to write or invent, and which section owns it.

1. **Unread counts in a row need a per-person join.** The row shows
   `{ count: "unread" }`, meaning records in this chat after my read anchor,
   not written by me. The read anchor lives in my private file; the records
   live in the chat. A query cannot join one person's private file into a
   count over another file without a new primitive. Options: the host keeps a
   per-person cursor per record file (a "read position" primitive by another
   name), or Chat writes counts into my private file on every message (a
   write per reader per message). *Files and questions lane.*
2. **A plugin cannot honour another person's mute.** `chat.notify` runs as
   the sender (or as a trigger with no person) and cannot read the
   recipients' private files. Either notifications need a per-recipient
   filter the host applies (the recipient's plugin state consulted by the
   host), or mutes must live in a shared file, which leaks who muted whom.
   *Permissions lane.*
3. **Who is the actor of a trigger?** `on.change` fires when a file changes.
   I assumed the trigger runs as the person who caused the change, with
   `ctx.cause` describing it, and that origin stamps carry the cause (so
   `chat.reply` called from the harness's trigger is stamped
   `run: <run>`). Without that, an agent's reply would be stamped as the
   harness plugin and readers could not tell agent text from plugin text.
   *Running code and permissions lanes.*
4. **Trigger delivery must be durable and at-least-once with a cursor.**
   The restart story depends on the host remembering, per trigger, which
   records it has delivered, and re-delivering after a crash. If the host
   does not keep that cursor, every plugin writes one into a file
   (a convention every app would reinvent). *Running code lane.*
5. **Calling "the part the person picked".** `ctx.call({ slot, plugin })`
   is invented: the composer and the harness are parts of slots, and Chat
   needs to call the picked harness's operation with a signature the slot
   declares. The draft has extension points but no call form for them.
   *Permissions and calls lane.*
6. **Consent waiting is a timer plus guesswork.** `people.consent` asks
   authors, but Chat then waits a fixed minute with `ctx.later`. It would be
   cleaner to trigger on the consent answers (`on.change` over grants), which
   needs grants to be queryable files or a trigger source of their own.
   *Permissions lane.*
7. **Which reads are consent-filtered.** I invented `grant.agent: true`:
   every read of a shared file by such a plugin is filtered by author
   consent. The thesis says "a run acting for an agent"; but the harness
   plugin's handler reads the context and passes it into the run's input,
   so the filter must apply to the plugin, not only the run. *Permissions
   lane.*
8. **Erasing a record from history.** `append({ replaces, erase: true })`
   must remove the bytes from the file, from every earlier revision and from
   the index, or "delete for everyone" is false. Files promises history
   (`files.history`, `host.revert`), so erase needs an explicit host rule:
   which revisions are rewritten, and what `host.revert` does across an
   erase. *Files lane.*
9. **Record fields versus record body.** I put message text in a record
   field because records need structure (reply, quote, mentions). Whether a
   record is "frontmatter-like fields plus a Markdown body" or free bytes
   decides how a chat file reads in a text editor. *Files lane.*
10. **The `running` field is a convention.** Chat stores the current run in
    a frontmatter field so `chat.stop` can find it. A query over runs by
    file (`status` for "the run serving this chat") would remove the field
    and a write per call. *Running code lane.*
11. **Declarative operations are a second way to write an operation.**
    `create` and `set` make Reading List and Notes code-free, but they are a
    small language (`$input`, field defaults) that the host must interpret.
    The alternative is a three-line handler per operation and a worker for
    every app. *Calls lane.*
12. **Field types are a vocabulary.** `"text"`, `"long"`, `"url"`,
    `"people"`, `"anchor"`, `"ref"`, `"map"` cover these four plugins; JSON
    Schema would be more general and much longer. The index and the
    components need to know types like `url.site` and `people.count`.
    *Files and drawing lanes.*
13. **Folder templates.** `"Chats/{created:date} {title}"` replaces
    revision 14's title binding. It is one more mini-language, and rename
    must re-run it. *Files lane.*
14. **Typing and "working" indicators** have no primitive. They are not
    files (they must not be stored), not questions, not runs. The draft
    mentions an "ephemeral appdata channel"; it needs a name and a bound.
    *Running code or drawing lane.*
15. **A private file per person per app needs a find-or-create.**
    `mineFile` queries, then creates with a key. A kind with
    `private: "person"` could be addressed as `"$mine"` directly. *Files
    lane.*
16. **Drawing needs to know about `own`.** The transcript shows Edit and
    Delete only on my own records. That requires the component to compare
    the origin stamp with the viewer, which is a host-side rule the
    catalogue has to include. *Drawing lane.*
17. **No unit of change for Undo.** Chat's "Delete for everyone" and Notes'
    move into a collection touch one file each here, but a future operation
    that writes several files has no way to group them for the generic
    revert (thesis question 5). *Files lane.*

None of the four design sections had been pushed when this was written
(`lane/design-*` branches absent at 17:50 UTC); the owner column above is a
guess at which lane each item belongs to.

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
- Create, read, replace and append to files of kinds it may write, and read
  their history. Example: Notes restores an old version.
- Keep one private file per person. Example: Chat's read positions.
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
- Read another person's private file, or learn about people outside a shared
  group. Example: Chat cannot see who muted a chat.
- Give an agent plugin text whose author has not agreed. Example: the
  harness gets erased records for non-consenting authors.
- Run code in a client, except in a sealed frame. Example: no custom
  JavaScript in the composer.
- Draw outside the catalogue or hide the host's challenge for risky
  effects. Example: sharing a file always shows the host's confirmation.
- Keep running after Stop, or exceed its storage, compute, call and wake-up
  quotas. Example: a looping trigger is cut off.
- Reach the network except declared hosts. Example: the harness image only
  talks to the model provider.

## 6. Questions for the owner

**1. Should the host keep a read position per person per record file?**
*What:* a read position is "the last record I have seen in this file".
*Why now:* it decides whether unread counts, the most visible number in
Chat and Mail, can be a query. *Example:* the sidebar shows "3" next to a
chat; computing it needs my position and the chat's records together.
*Options:* (a) the host keeps one position per person per record file, as
small appdata rebuilt from a per-person file: counts become a query field;
costs one more host concept. (b) Plugins write counts into each reader's
private file: no new concept, but one write per reader per message, and the
plugin cannot write other people's private files at all. *Recommendation:*
(a); without it unread counts cannot be built from the seven primitives.

**2. When a trigger fires, who is acting?**
*What:* a change trigger runs plugin code because a file changed.
*Why now:* origin stamps and permissions both depend on it. *Example:* an
agent's run appends its final answer to its log; the harness's trigger posts
it into the chat. *Options:* (a) the trigger acts as the person whose action
caused the change, and the stamp records the cause (the run): agent replies
are correctly attributed; the host must carry causes across hops. (b) The
trigger acts as the plugin alone: simpler, but every agent reply looks like
plugin text and consent cannot be enforced on it. *Recommendation:* (a).

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
